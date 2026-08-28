#!/usr/bin/env python3
# FALSIFICATION — the audit stronger than greps: deliberately break a
# law in the source, run the narrowest suite, DEMAND failure, revert.
# A law whose breakage no test notices was a costume.
#
# Anchors are point-in-time source text: ANCHOR MISS means the code
# moved, not that the law fell — re-anchor and re-run. Run from the
# workspace root, on a CLEAN tree (this script reverts via git).
import subprocess, sys

MUTATIONS = [
    # (name, file, old, new, test_cmd)
    ("kill ∂c=0 closure law",
     "crates/assay/src/complex.rs",
     "        if let Some((cell, _)) = flux.iter().find(|(_, v)| !v.is_zero()) {\n            return Err(ComplexBroken::OpenBoundary { cell: *cell });\n        }\n        Ok(fuel.spent())\n    }\n\n    /// SQ1",
     "        let _ = &flux;\n        Ok(fuel.spent())\n    }\n\n    /// SQ1",
     ["cargo","test","-q","-p","plumb-assay","--test","engine_laws"]),
    ("kill signature verification in admission",
     "crates/datum/src/admission.rs",
     "    match attestation.verify(envelope) {\n        Ok(()) => {}\n        Err(sig::SigRefused::UnknownScheme(s)) => {\n            return Err(AdmissionRefused::UnknownScheme(s))\n        }\n        Err(_) => return Err(AdmissionRefused::Forged),\n    }",
     "    let _ = &envelope;",
     ["cargo","test","-q","-p","plumb-datum","--test","wire"]),
    ("kill the replay law (T2)",
     "crates/datum/src/reward.rs",
     "        if self.seen.contains(&work_id) {\n            return Err(RewardRefused::Replay { work_id });\n        }",
     "        let _ = &work_id;",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
    ("kill session freshness (always live)",
     "crates/datum/src/plumbd.rs",
     "    let mut session_live = !enforce;",
     "    let mut session_live = true;",
     ["cargo","test","-q","-p","plumb-datum","--test","wire"]),
    ("kill the refinement threshold",
     "crates/datum/src/bounty.rs",
     "    if lhs > rhs {\n        return Err(RefineRefused::NotAnImprovement {",
     "    if false {\n        return Err(RefineRefused::NotAnImprovement {",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
    ("license illegal inferences (compile both directions)",
     "crates/assay/src/rewrite.rs",
     "                    steps.push(Step { from, to, rule, at });",
     "                    steps.push(Step { from, to, rule, at });\n                    steps.push(Step { from: to, to: from, rule, at });",
     ["cargo","test","-q","-p","plumb-assay","--test","engine_laws"]),
    ("kill NotThatCourt on receipts",
     "crates/datum/src/receipt.rs",
     "    if holder != signed.receipt.court {\n        return Err(ReceiptRefused::NotThatCourt);\n    }",
     "    let _ = &holder;",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
    ("kill the poser's-universe gate (rebate free money)",
     "crates/datum/src/bounty.rs",
     "        if claim.complex.encode() != query.statement {\n            return Err(AnswerRefused::NotThePosersUniverse);\n        }",
     "        let _ = &claim;",
     ["cargo","test","-q","-p","plumb-datum","--test","market"]),
]

results = []
for name, path, old, new, cmd in MUTATIONS:
    text = open(path).read()
    if text.count(old) != 1:
        results.append((name, "ANCHOR MISS"))
        continue
    open(path, "w").write(text.replace(old, new))
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    subprocess.run(["git", "checkout", "--", path], check=True)
    if proc.returncode != 0:
        results.append((name, "CAUGHT (tests failed as they must)"))
    else:
        results.append((name, "!!! SURVIVED — the law is NOT constrained by tests"))

print("\n=== MUTATION AUDIT ===")
for name, verdict in results:
    print(f"  {verdict:55s} {name}")
sys.exit(0)
