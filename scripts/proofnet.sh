#!/usr/bin/env bash
# B0 — the local proofnet: the whole economy on one machine.
#
#   genesis chain (grants + key binds + a registered domain)
#     → enforcing court A (snapshots, federates to B)
#     → court B (accepts federation)
#     → signed producer sends a DECLARED-domain claim (+ its replay)
#     → kill -9 court A → restart → resend → still replay
#
# Exits 0 with PASS only if every check holds. Everything runs under
# a scratch directory and is torn down on exit.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${1:-$(mktemp -d /tmp/plumb-proofnet.XXXXXX)}"
BIN="$ROOT/target/debug/plumbd"
PASS=0; FAIL=0
A_PORT=9451; B_FED_PORT=9452

say()  { printf '\n== %s\n' "$*"; }
ok()   { printf '   ok: %s\n' "$*"; PASS=$((PASS+1)); }
bad()  { printf '   FAIL: %s\n' "$*"; FAIL=$((FAIL+1)); }

cleanup() {
  [ -n "${A_PID:-}" ] && kill "$A_PID" 2>/dev/null
  [ -n "${B_PID:-}" ] && kill "$B_PID" 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT

say "build"
(cd "$ROOT" && cargo build -q --bin plumbd) || { bad "build"; exit 1; }
ok "plumbd builds"

SOLVER_SEED="0707070707070707070707070707070707070707070707070707070707070707"

say "genesis — grants, key bind, registered domain, on the record"
cat > "$WORK/genesis.conf" <<EOF
role    = genesis
holder  = court-a
grant   = court-b
grant   = solver-a
bind    = solver-a:$SOLVER_SEED
declare = court-a
out     = $WORK/chain.tlv
EOF
"$BIN" "$WORK/genesis.conf" | tee "$WORK/genesis.log"
grep -q '1 bind(s), 1 declaration(s)' "$WORK/genesis.log" \
  && ok "genesis carries the grant flow: issue + bind + declare" \
  || bad "genesis incomplete"

say "courts — A enforces signatures and federates to B"
cat > "$WORK/court-a.conf" <<EOF
role = court
holder = court-a
chain = $WORK/chain.tlv
listen = 127.0.0.1:$A_PORT
require_signatures = true
snapshot = $WORK/court-a.xdct
snapshot_secs = 1
fed_peer = 127.0.0.1:$B_FED_PORT
fed_secs = 1
EOF
cat > "$WORK/court-b.conf" <<EOF
role = court
holder = court-b
chain = $WORK/chain.tlv
listen = 127.0.0.1:9453
snapshot = $WORK/court-b.xdct
snapshot_secs = 1
fed_listen = 127.0.0.1:$B_FED_PORT
EOF
"$BIN" "$WORK/court-b.conf" > "$WORK/court-b.log" 2>&1 & B_PID=$!
"$BIN" "$WORK/court-a.conf" > "$WORK/court-a.log" 2>&1 & A_PID=$!
sleep 1
grep -q 'signature enforcement ON' "$WORK/court-a.log" \
  && ok "court A enforces (S4)" || bad "court A not enforcing"
grep -q 'registered domain(s) resolved from chain state' "$WORK/court-a.log" \
  && ok "court A learned a discipline from the chain alone (UC4)" \
  || bad "no registered domain resolved"

say "producer — signed, declared-domain claim, sent twice"
cat > "$WORK/producer.conf" <<EOF
role = producer
holder = solver-a
chain = $WORK/chain.tlv
demo = hexagon
seed = $SOLVER_SEED
peer = 127.0.0.1:$A_PORT
EOF
"$BIN" "$WORK/producer.conf" && "$BIN" "$WORK/producer.conf"
sleep 2
grep -q 'credited 1, refused 0' "$WORK/court-a.log" \
  && ok "the signed declared claim credited (S1-S7 + UC1-UC3)" \
  || bad "claim did not credit"
grep -q 'credited 0, refused 1' "$WORK/court-a.log" \
  && ok "its copy refused as replay (work_id)" || bad "replay not refused"

say "federation — court B carries the act, once"
sleep 2
B_ACTS=$(ls -l "$WORK/court-b.xdct" 2>/dev/null | awk '{print $5}')
[ -n "$B_ACTS" ] && [ "$B_ACTS" -gt 10 ] \
  && ok "court B snapshotted a federated book" || bad "no federated snapshot at B"

say "kill -9 court A — a power cut, not a goodbye"
kill -9 "$A_PID" 2>/dev/null; wait "$A_PID" 2>/dev/null; A_PID=
sleep 1
"$BIN" "$WORK/court-a.conf" > "$WORK/court-a2.log" 2>&1 & A_PID=$!
sleep 1
grep -q 'resumed 1 act(s) from snapshot' "$WORK/court-a2.log" \
  && ok "court A resumed from its snapshot" || bad "no resume"
"$BIN" "$WORK/producer.conf"
sleep 1
grep -q 'credited 0, refused 1' "$WORK/court-a2.log" \
  && ok "and still refuses the replay: the record survived the kill" \
  || bad "resumed court re-credited a replay"

say "verdict"
echo "   passed $PASS, failed $FAIL  (work dir: $WORK)"
if [ "$FAIL" -eq 0 ]; then echo "   PROOFNET: PASS"; exit 0; fi
echo "   PROOFNET: FAIL"; exit 1
