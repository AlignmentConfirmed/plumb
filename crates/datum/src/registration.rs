//! P2 — live registration: a running court binds a fresh key without
//! a restart or a re-run of genesis.
//!
//! ```text
//! REGISTER_TAG   the request: holder name ‖ scheme ‖ public key
//! attestation    a self-signed proof of possession over the
//!                session's own freshness challenge — the same rule
//!                that keeps a replayed admission dead, applied
//!                before the bind exists
//! REGISTER_TAG   the court's ack: the granted range and bind window
//! ```
//!
//! Before this module, the only way a holder got bound was genesis
//! time — hand-edit a `bind = holder:seedhex` line and re-run genesis.
//! That is not a live network, it is a fixture with a restart button.
//! A stranger who shows up after the network is already running has
//! nowhere to go. This closes that: a court that opts in
//! (`SessionRules::register`) accepts a register request from an
//! unbound key, proves the requester actually holds it (never the
//! seed — only a signature over a challenge this session minted), and
//! appends `Act::Issue` + `Act::Bind` to its own live ledger.
//!
//! What this module does not decide: whether appending is safe against
//! a flood of holder names (that is [`crate::plumbd`]'s admission
//! wall, a separate concern) or who may connect at all (TCP accept is
//! wide open; this is the one check standing behind it for a new
//! identity).

use isthmus::deed::{Act, Deed, Ledger};
use isthmus::layout::Tag;

/// The tag a register request — and the court's ack — travel under:
/// beside claims (80–82), attestation (83), witness (84), query (85).
pub const REGISTER_TAG: Tag = 86;

/// How wide a deed a freshly registered holder receives — the same
/// width `plumbd genesis`'s `grant =` lines give everyone else. A
/// live-registered holder is not a second class of party.
pub const REGISTER_WIDTH: u128 = 16;

/// A stranger's request to become a named, bound party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRequest {
    /// What they want the chain to call them. Not yet a claim on
    /// anything — `bind_live` is where a collision refuses.
    pub holder: String,
    /// Which scheme the accompanying key is. Unknown schemes are a
    /// named refusal at `verify_possession`, never a guess.
    pub scheme: u8,
    /// The public key this request is for — never a seed. Proof that
    /// the requester holds the matching private half is the
    /// accompanying attestation, checked separately.
    pub key: [u8; 32],
}

/// Why a register request did not decode, prove, or bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterRefused {
    /// The request or ack bytes are not the right shape.
    Malformed,
    /// A scheme this build does not speak.
    UnknownScheme(u8),
    /// The attestation signs a different key than the one being
    /// registered — proof of possession failed before it started.
    NotYourKey,
    /// The attestation does not verify over this session's own
    /// challenge: forged, or answering a different session entirely.
    Forged,
    /// That holder name already has a live deed.
    HolderTaken,
    /// That public key is already bound to someone.
    KeyAlreadyBound,
    /// No open run large enough remains in the tag space.
    NoRoom,
}

impl RegisterRequest {
    /// `scheme(1) ‖ key(32) ‖ LE16(len) ‖ holder`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + 32 + 2 + self.holder.len());
        out.push(self.scheme);
        out.extend_from_slice(&self.key);
        let holder = self.holder.as_bytes();
        let len = u16::try_from(holder.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(holder.get(..usize::from(len)).unwrap_or(holder));
        out
    }

    /// Read a request back. Refuses the wrong shape rather than
    /// guessing at trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, RegisterRefused> {
        let scheme = *bytes.first().ok_or(RegisterRefused::Malformed)?;
        let mut key = [0u8; 32];
        key.copy_from_slice(bytes.get(1..33).ok_or(RegisterRefused::Malformed)?);
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(bytes.get(33..35).ok_or(RegisterRefused::Malformed)?);
        let len = usize::from(u16::from_le_bytes(len_bytes));
        let end = 35usize.saturating_add(len);
        let holder_bytes = bytes.get(35..end).ok_or(RegisterRefused::Malformed)?;
        let holder =
            String::from_utf8(holder_bytes.to_vec()).map_err(|_| RegisterRefused::Malformed)?;
        if end != bytes.len() {
            return Err(RegisterRefused::Malformed);
        }
        Ok(Self { holder, scheme, key })
    }
}

/// The court's ack: the deed it granted, and the bind window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterOutcome {
    /// First tag of the granted deed, inclusive.
    pub low: Tag,
    /// Last tag of the granted deed, inclusive.
    pub high: Tag,
    /// The bind's epoch window.
    pub from_epoch: u64,
    /// The bind's epoch window.
    pub until_epoch: u64,
}

impl RegisterOutcome {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32);
        out.extend_from_slice(&self.low.to_le_bytes());
        out.extend_from_slice(&self.high.to_le_bytes());
        out.extend_from_slice(&self.from_epoch.to_le_bytes());
        out.extend_from_slice(&self.until_epoch.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RegisterRefused> {
        if bytes.len() != 32 {
            return Err(RegisterRefused::Malformed);
        }
        let take = |at: usize| -> u64 {
            let mut b = [0u8; 8];
            b.copy_from_slice(bytes.get(at..at + 8).unwrap_or(&[0u8; 8]));
            u64::from_le_bytes(b)
        };
        Ok(Self {
            low: take(0),
            high: take(8),
            from_epoch: take(16),
            until_epoch: take(24),
        })
    }
}

/// Verify the self-signed proof of possession: the attestation's
/// signer IS the key being registered, and it verifies over this
/// session's own freshness challenge. A captured request cannot be
/// replayed against a different court or a different session — the
/// same freshness rule S4 already applies to a bound key, applied
/// here before the bind exists.
pub fn verify_possession(
    request: &RegisterRequest,
    challenge_frame: &[u8],
    attestation_value: &[u8],
) -> Result<(), RegisterRefused> {
    let attestation =
        sig::Attestation::decode(attestation_value).map_err(|_| RegisterRefused::Malformed)?;
    if attestation.scheme != request.scheme {
        return Err(RegisterRefused::UnknownScheme(attestation.scheme));
    }
    if attestation.signer != request.key {
        return Err(RegisterRefused::NotYourKey);
    }
    attestation
        .verify(challenge_frame)
        .map_err(|_| RegisterRefused::Forged)
}

/// Bind a freshly proven key to a chain that had never heard of it —
/// live, no restart. Proof of possession is the caller's job
/// ([`verify_possession`], before this runs); this enforces only the
/// ledger-level rules: the name is not already held, the key is not
/// already bound to someone else, and there is room to issue it a
/// deed.
pub fn bind_live(
    ledger: &mut Ledger,
    request: &RegisterRequest,
    epoch: u64,
) -> Result<Deed, RegisterRefused> {
    if ledger.binding_of(&request.holder).is_some()
        || ledger
            .deeds()
            .into_iter()
            .any(|d| d.live && d.holder == request.holder)
    {
        return Err(RegisterRefused::HolderTaken);
    }
    if crate::admission::holder_of_key(ledger, request.scheme, &request.key).is_some() {
        return Err(RegisterRefused::KeyAlreadyBound);
    }
    let deed = ledger
        .issue(&request.holder, REGISTER_WIDTH)
        .map_err(|_| RegisterRefused::NoRoom)?;
    ledger.record(Act::Bind {
        holder: request.holder.clone(),
        scheme: request.scheme,
        key: request.key.to_vec(),
        from_epoch: epoch,
        until_epoch: u64::MAX,
    });
    Ok(deed)
}

/// Flush the ledger's whole act log to `path`, atomically (temp file,
/// then rename) — the same technique `court_service::snapshot_atomic`
/// uses for the reward book. A restart that replays this file sees
/// every live registration that landed before it.
pub fn persist_chain_atomic(path: &std::path::Path, ledger: &Ledger) -> std::io::Result<()> {
    let bytes = isthmus::deed::chain::encode(ledger.acts());
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use isthmus::layout::Layout;

    fn fresh_ledger() -> Ledger {
        let mut ledger = Ledger::new(Layout::founding());
        ledger.encumber(1, 31, "ancestral", "founding registries");
        ledger
    }

    #[test]
    fn a_genuine_proof_of_possession_binds_and_a_forged_one_refuses() {
        let key = sig::Keypair::from_seed([9u8; 32]);
        let request = RegisterRequest {
            holder: "newcomer".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: key.public(),
        };
        let challenge = b"session-challenge-frame-bytes".to_vec();
        let genuine = key.attest(&challenge);
        verify_possession(&request, &challenge, &genuine.encode()).expect("proves possession");

        // Signed over a different session's challenge: not this one.
        let stale = key.attest(b"a different session's challenge");
        assert_eq!(
            verify_possession(&request, &challenge, &stale.encode()),
            Err(RegisterRefused::Forged)
        );

        // Signed by a key other than the one being registered.
        let impostor = sig::Keypair::from_seed([2u8; 32]);
        let wrong_signer = impostor.attest(&challenge);
        assert_eq!(
            verify_possession(&request, &challenge, &wrong_signer.encode()),
            Err(RegisterRefused::NotYourKey)
        );
    }

    #[test]
    fn bind_live_issues_a_real_deed_and_the_bound_key_resolves() {
        let mut ledger = fresh_ledger();
        let key = sig::Keypair::from_seed([3u8; 32]);
        let request = RegisterRequest {
            holder: "newcomer".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: key.public(),
        };
        let deed = bind_live(&mut ledger, &request, 5).expect("binds");
        assert_eq!(deed.high() - deed.low() + 1, REGISTER_WIDTH as u64);
        assert_eq!(
            crate::admission::holder_of_key(&ledger, request.scheme, &request.key),
            Some("newcomer".into())
        );
        let binding = ledger.binding_of("newcomer").expect("bound");
        assert_eq!(binding.from_epoch, 5);
        assert_eq!(binding.until_epoch, u64::MAX);
    }

    #[test]
    fn bind_live_refuses_a_taken_name_or_an_already_bound_key() {
        let mut ledger = fresh_ledger();
        let first = sig::Keypair::from_seed([4u8; 32]);
        let request = RegisterRequest {
            holder: "newcomer".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: first.public(),
        };
        bind_live(&mut ledger, &request, 0).expect("first bind");

        let second = sig::Keypair::from_seed([5u8; 32]);
        let same_name = RegisterRequest {
            holder: "newcomer".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: second.public(),
        };
        assert_eq!(
            bind_live(&mut ledger, &same_name, 0),
            Err(RegisterRefused::HolderTaken)
        );

        let same_key = RegisterRequest {
            holder: "someone-else".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: first.public(),
        };
        assert_eq!(
            bind_live(&mut ledger, &same_key, 0),
            Err(RegisterRefused::KeyAlreadyBound)
        );
    }

    #[test]
    fn request_and_outcome_round_trip_through_their_wire_encoding() {
        let request = RegisterRequest {
            holder: "a-fairly-long-holder-name".into(),
            scheme: sig::SCHEME_ED25519_BLAKE3,
            key: [7u8; 32],
        };
        assert_eq!(RegisterRequest::decode(&request.encode()), Ok(request));

        let outcome = RegisterOutcome {
            low: 64,
            high: 79,
            from_epoch: 3,
            until_epoch: u64::MAX,
        };
        assert_eq!(RegisterOutcome::decode(&outcome.encode()), Ok(outcome));
        assert_eq!(RegisterOutcome::decode(&[0u8; 31]), Err(RegisterRefused::Malformed));
    }
}
