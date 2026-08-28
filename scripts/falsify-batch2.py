#!/usr/bin/env python3
# FALSIFICATION — the audit stronger than greps: deliberately break a
# law in the source, run the narrowest suite, DEMAND failure, revert.
# A law whose breakage no test notices was a costume.
#
# Anchors are point-in-time source text: ANCHOR MISS means the code
# moved, not that the law fell — re-anchor and re-run. Run from the
# workspace root, on a CLEAN tree (this script reverts via git).
import subprocess, sys

M = [
    ("flip the divergence orientation (gauge/flux law)",
     "crates/assay/src/flux.rs",
     "        match self.orientation {\n            Orientation::High => self.flux.clone(),\n            Orientation::Low => -self.flux.clone(),\n        }",
     "        match self.orientation {\n            Orientation::High => self.flux.clone(),\n            Orientation::Low => self.flux.clone(),\n        }",
     ["cargo","test","-q","-p","plumb-assay","--test","engine_laws"]),
    ("work_id keeps the transport (content-address law)",
     "crates/assay/src/complex.rs",
     "    pub fn work_id(&self) -> crate::work::WorkId {\n        crate::work::WorkId::from_bytes(self.encode_with_transport(0))\n    }\n\n    /// Multi-axial credit: the breadth of the universe the closure was",
     "    pub fn work_id(&self) -> crate::work::WorkId {\n        crate::work::WorkId::from_bytes(self.encode_with_transport(self.transport))\n    }\n\n    /// Multi-axial credit: the breadth of the universe the closure was",
     ["cargo","test","-q","-p","plumb-assay","--test","complex_laws","-p","plumb-datum","--test","market"]),
    ("skip the citation check (SQ2 lemma law)",
     "crates/datum/src/reward.rs",
     "        if let WorkBody::Proof(p) = &work {\n            for dep in &p.deps {\n                let cited = WorkId::from_bytes(dep.clone());\n                if !self.seen.contains(&cited) {\n                    return Err(RewardRefused::UnsettledDependency { work_id: cited });\n                }\n            }\n        }",
     "        let _ = &work;",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
    ("constant session token (freshness entropy law)",
     "crates/sig/src/lib.rs",
     "    let mut token = [0u8; 8];\n    getrandom::getrandom(&mut token).map_err(|_| KeyBroken::NoEntropy)?;\n    Ok(token)",
     "    Ok([7u8; 8])",
     ["cargo","test","-q","-p","plumb-datum","--test","wire"]),
    ("guess the witness arm (IS-4 budget law)",
     "crates/isthmus/src/witness.rs",
     "        let arm = match take(&mut at, 1)?.first().copied() {\n            Some(0) => Arm::Succinct,\n            Some(1) => Arm::Replay,\n            _ => return Err(Malformed::TrailingBytes { left: 1 }),\n        };",
     "        let arm = match take(&mut at, 1)?.first().copied() {\n            Some(1) => Arm::Replay,\n            _ => Arm::Succinct,\n        };",
     ["cargo","test","-q","-p","plumb-isthmus","--test","wire_suite"]),
    ("a definition outlives its grant (lapse law)",
     "crates/isthmus/src/deed.rs",
     "        let holder = self.holder_of(tag).filter(|d| d.live)?.holder;",
     "        let holder = self.holder_of(tag)?.holder;",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
    ("equivalence names unsettled work (O3 guard)",
     "crates/datum/src/reward.rs",
     "        if !self.seen.contains(&old) {\n            return Err(RewardRefused::UnsettledDependency { work_id: old });\n        }\n        if !self.seen.contains(&new) {\n            return Err(RewardRefused::UnsettledDependency { work_id: new });\n        }",
     "        let _ = (&old, &new);",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
    ("covers always true (T5 solvency law)",
     "crates/datum/src/extent.rs",
     "        self.axes.len() == outer.axes.len()\n            && self\n                .axes\n                .iter()\n                .zip(outer.axes.iter())\n                .all(|(mine, theirs)| mine <= theirs)",
     "        let _ = outer;\n        true",
     ["cargo","test","-q","-p","plumb-datum","--test","court_laws"]),
    ("carrier drops attestations (payload-blind carriage)",
     "crates/datum/src/plumbd.rs",
     "    let mut forwarded = 0usize;\n    while let Some((_tag, frame)) = read_record(&mut client, &mut client_buf, layout, bound)? {\n        court.write_all(&frame)?;\n        forwarded += 1;\n    }",
     "    let mut forwarded = 0usize;\n    while let Some((_tag, frame)) = read_record(&mut client, &mut client_buf, layout, bound)? {\n        if _tag != crate::admission::ATTESTATION_TAG { court.write_all(&frame)?; }\n        forwarded += 1;\n    }",
     ["cargo","test","-q","-p","plumb-datum","--test","wire"]),
    ("wrong EIP-3009 selector (standard's shape)",
     "crates/datum/src/bin/gateway.rs",
     "    out.extend_from_slice(&[0xe3, 0xee, 0x16, 0x0e]);",
     "    out.extend_from_slice(&[0xe3, 0xee, 0x16, 0x0f]);",
     ["cargo","test","-q","-p","plumb-datum","--bin","gateway"]),
]

for name, path, old, new, cmd in M:
    text = open(path).read()
    if text.count(old) != 1:
        print(f"  ANCHOR MISS                                             {name}")
        continue
    open(path, "w").write(text.replace(old, new))
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    subprocess.run(["git", "checkout", "--", path], check=True)
    verdict = ("CAUGHT (tests failed as they must)" if proc.returncode != 0
               else "!!! SURVIVED — the law is NOT constrained by tests")
    print(f"  {verdict:55s} {name}")
