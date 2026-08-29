//! P4 — transport confidentiality (`IS-6/6`).
//!
//! ```text
//! plaintext TCP   +---------------------------+   claim / receipt / corpus
//!                 |   readable to anyone       |   sat in the clear
//!                 |   who can see the wire     |
//!                 +---------------------------+
//! ```
//!
//! Ed25519 attestations authenticate content — who signed a claim.
//! Nothing before this module authenticated the channel: deployed on
//! a public IP, every claim, receipt, and corpus body a peer ever
//! sent crossed the wire in the clear.
//!
//! TLS buys confidentiality and integrity of the bytes. It buys
//! **nothing about identity** — a plumb court has no DNS name and
//! answers to no CA, so the usual "does this certificate chain to a
//! trust anchor" question has no honest answer here. The one thing a
//! connecting peer can check a certificate against is a fact the
//! chain already vouches for: `Act::Certify` records that a named
//! holder's certificate hashes to a specific fingerprint. Verifying a
//! TLS session then means exactly one thing — "the certificate this
//! socket just presented is the one this chain says this holder
//! uses" — never "some CA vouches for this," which nothing here has.
//!
//! What TLS still does for real, and what [`FingerprintVerifier`]
//! does not skip: the handshake signature is checked against the
//! certificate's embedded key with the same cryptographic primitives
//! any TLS stack uses ([`webpki::EndEntityCert::verify_signature`]).
//! Skipping chain-of-trust is a deliberate, narrow substitution of
//! trust anchor; skipping signature verification would leave the
//! channel unauthenticated.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

/// BLAKE3 of a certificate's exact DER bytes — what [`isthmus::deed::Act::Certify`]
/// records and what a connecting peer checks a presented certificate
/// against.
#[must_use]
pub fn fingerprint(der: &[u8]) -> [u8; 32] {
    sig::envelope_hash(der)
}

/// Why generating or loading a transport identity, or building a
/// config from one, failed.
#[derive(Debug)]
pub enum TlsBroken {
    /// Certificate or key generation refused.
    Generate(String),
    /// Loading a config from generated material refused.
    Configure(String),
}

impl std::fmt::Display for TlsBroken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsBroken::Generate(msg) | TlsBroken::Configure(msg) => write!(f, "{msg}"),
        }
    }
}

/// A generated transport identity: a self-signed certificate and its
/// matching private key, both DER.
#[derive(Debug, Clone)]
pub struct Identity {
    /// The certificate, DER-encoded.
    pub cert_der: Vec<u8>,
    /// The private key, PKCS8 DER-encoded.
    pub key_der: Vec<u8>,
}

impl Identity {
    /// The fingerprint a peer must see on-chain to accept this
    /// identity's certificate.
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.cert_der)
    }
}

/// A fresh self-signed Ed25519 certificate for `holder`.
///
/// TLS's own notion of "subject" is cosmetic here: the certificate's
/// SAN is never checked by [`FingerprintVerifier`] — the chain's
/// `Certify` act is the only fact that matters. Ed25519 is chosen
/// deliberately, not left to rcgen's default (`ECDSA_P256`): it is
/// the one scheme [`FingerprintVerifier`] speaks, which is what makes
/// its supported-scheme list a single arm instead of a table nobody
/// can audit.
pub fn generate_identity(holder: &str) -> Result<Identity, TlsBroken> {
    let key_pair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ED25519)
        .map_err(|e| TlsBroken::Generate(e.to_string()))?;
    let params = rcgen::CertificateParams::new(vec![holder.to_owned()])
        .map_err(|e| TlsBroken::Generate(e.to_string()))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| TlsBroken::Generate(e.to_string()))?;
    Ok(Identity {
        cert_der: cert.der().to_vec(),
        key_der: key_pair.serialize_der(),
    })
}

fn install_crypto_provider() {
    // Safe to call repeatedly: only the first call in a process
    // installs anything, and every later one is a harmless no-op —
    // exactly what a court and everything it dials out to both doing
    // this independently needs.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// A court's own server config: presents `identity`'s certificate,
/// signs the handshake with its matching key. No client-certificate
/// requirement — a connecting peer's identity is the Ed25519
/// attestation it sends inside the encrypted channel, never a TLS
/// client cert.
pub fn server_config(identity: &Identity) -> Result<rustls::ServerConfig, TlsBroken> {
    install_crypto_provider();
    let cert_chain = vec![CertificateDer::from(identity.cert_der.clone())];
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identity.key_der.clone()));
    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|e| TlsBroken::Configure(e.to_string()))
}

/// A client's config for dialing a specific holder: trusts exactly
/// one certificate, the one whose fingerprint matches what the chain
/// says that holder certified. No CA, no chain-of-trust list.
#[must_use]
pub fn client_config(expected_fingerprint: [u8; 32]) -> rustls::ClientConfig {
    install_crypto_provider();
    rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(FingerprintVerifier {
            expected: expected_fingerprint,
        }))
        .with_no_client_auth()
}

/// A verifier that trusts exactly one fact: the certificate presented
/// hashes to the fingerprint a specific holder recorded on-chain. No
/// CA, no hostname match, no chain-of-trust — the chain state already
/// loaded IS the trust anchor.
#[derive(Debug)]
struct FingerprintVerifier {
    expected: [u8; 32],
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity.as_ref()) == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "presented certificate does not match the chain-pinned fingerprint".into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_ed25519_handshake_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_ed25519_handshake_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // The one scheme generate_identity ever issues. A wider table
        // here would be a promise this verifier does not keep.
        vec![SignatureScheme::ED25519]
    }
}

/// The signature check chain-of-trust skipping does not skip: does
/// this handshake message's signature actually verify against the
/// certificate's own embedded public key? A verifier that answered
/// [`ServerCertVerifier::verify_server_cert`] honestly but rubber-
/// stamped this would let anyone who ever saw the (public, unsecret)
/// certificate bytes replay them without holding the private key.
fn verify_ed25519_handshake_signature(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
) -> Result<HandshakeSignatureValid, rustls::Error> {
    if dss.scheme != SignatureScheme::ED25519 {
        return Err(rustls::Error::General(format!(
            "unsupported handshake signature scheme {:?} — only Ed25519 certificates are issued here",
            dss.scheme
        )));
    }
    let end_entity =
        webpki::EndEntityCert::try_from(cert).map_err(|e| rustls::Error::General(format!("{e:?}")))?;
    end_entity
        .verify_signature(webpki::ring::ED25519, message, dss.signature())
        .map(|()| HandshakeSignatureValid::assertion())
        .map_err(|e| rustls::Error::General(format!("{e:?}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn generate_identity_draws_real_entropy_and_the_fingerprint_matches_the_cert() {
        let a = generate_identity("court-a").expect("generates");
        let b = generate_identity("court-a").expect("generates");
        assert_ne!(
            a.cert_der, b.cert_der,
            "two generated identities must not collide — this is entropy, not a fixture"
        );
        assert_eq!(a.fingerprint(), fingerprint(&a.cert_der));
    }

    #[test]
    fn a_fingerprint_mismatch_refuses_and_a_match_verifies() {
        let identity = generate_identity("court-b").expect("generates");
        let cert = CertificateDer::from(identity.cert_der.clone());

        let right = FingerprintVerifier {
            expected: identity.fingerprint(),
        };
        // A real TLS handshake message would come with a matching
        // signature; verify_server_cert alone does not check one,
        // which is exactly why verify_tls12/13_signature exist as
        // separate, mandatory methods on the trait.
        assert!(right
            .verify_server_cert(
                &cert,
                &[],
                &ServerName::try_from("court-b").expect("name"),
                &[],
                UnixTime::now(),
            )
            .is_ok());

        let wrong = FingerprintVerifier { expected: [0u8; 32] };
        assert!(wrong
            .verify_server_cert(
                &cert,
                &[],
                &ServerName::try_from("court-b").expect("name"),
                &[],
                UnixTime::now(),
            )
            .is_err());
    }

    #[test]
    fn server_and_client_configs_build_from_a_generated_identity() {
        let identity = generate_identity("court-c").expect("generates");
        server_config(&identity).expect("a generated identity always yields a server config");
        let _ = client_config(identity.fingerprint());
    }
}
