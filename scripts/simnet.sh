#!/usr/bin/env bash
# simnet — a standing distributed network on localhost, until the
# alpha/beta goes global. Ports live in the 9xxx range:
#
#   court-a   session 9501   federation 9601   (enforces signatures)
#   court-b   session 9502   federation 9602
#   court-c   session 9503   federation 9603
#   carrier-1 session 9701 → court-a           (forwards unread)
#   client-1  → carrier-1   fresh odd-cycle work, signed
#   client-2  → court-b     fresh even-cycle work, signed
#
# Federation ring: A→B→C→A. Every court converges on every act; replay
# refuses everywhere by work identity.
#
#   simnet.sh start | stop | status | logs [name] | reset

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOME_DIR="${PLUMB_SIMNET_DIR:-$HOME/.plumb/simnet}"
BIN="$ROOT/target/debug/plumbd"
IDENT="$HOME_DIR/ident"
# Every signing party in this network gets a REAL identity — drawn
# from OS entropy by `plumbd keygen`, never a repeated-digit fixture
# seed hand-typed into a config. Only the parties that actually sign
# something need one.
SIGNING_NODES="client-1 client-2 court-a solver-1 witness-1"
NODES="court-a court-b court-c carrier-1 client-1 client-2 gateway-1"

ensure_identities() {
  mkdir -p "$IDENT"
  for node in $SIGNING_NODES; do
    if [ ! -f "$IDENT/$node.seed" ]; then
      "$BIN" keygen "$IDENT/$node.seed" > /dev/null || { echo "keygen failed for $node"; exit 1; }
      echo "$node: identity generated ($IDENT/$node.seed)"
    fi
  done
}

seed_hex_of() {
  # genesis binding needs the RAW seed (it derives the public key
  # itself) — read it back from the real file keygen wrote, never
  # from a literal in this script.
  tail -n 1 "$IDENT/$1.seed" | tr -d '[:space:]'
}

write_configs() {
  cat > "$HOME_DIR/genesis.conf" <<EOF
role    = genesis
holder  = court-a
grant   = court-b
grant   = court-c
grant   = carrier-1
grant   = client-1
grant   = client-2
grant   = solver-1
grant   = witness-1
bind    = client-1:$(seed_hex_of client-1)
bind    = client-2:$(seed_hex_of client-2)
bind    = court-a:$(seed_hex_of court-a)
bind    = solver-1:$(seed_hex_of solver-1)
bind    = witness-1:$(seed_hex_of witness-1)
declare = court-a
out     = $HOME_DIR/chain.tlv
EOF
  cat > "$HOME_DIR/court-a.conf" <<EOF
role = court
holder = court-a
chain = $HOME_DIR/chain.tlv
listen = 127.0.0.1:9501
require_signatures = true
market = theta
seed_file = $IDENT/court-a.seed
snapshot = $HOME_DIR/court-a.xdct
snapshot_secs = 2
fed_listen = 127.0.0.1:9601
fed_peer = 127.0.0.1:9602
fed_secs = 3
EOF
  cat > "$HOME_DIR/court-b.conf" <<EOF
role = court
holder = court-b
chain = $HOME_DIR/chain.tlv
listen = 127.0.0.1:9502
require_signatures = true
snapshot = $HOME_DIR/court-b.xdct
snapshot_secs = 2
fed_listen = 127.0.0.1:9602
fed_peer = 127.0.0.1:9603
fed_secs = 3
EOF
  cat > "$HOME_DIR/court-c.conf" <<EOF
role = court
holder = court-c
chain = $HOME_DIR/chain.tlv
listen = 127.0.0.1:9503
require_signatures = true
snapshot = $HOME_DIR/court-c.xdct
snapshot_secs = 2
fed_listen = 127.0.0.1:9603
fed_peer = 127.0.0.1:9601
fed_secs = 3
EOF
  cat > "$HOME_DIR/carrier-1.conf" <<EOF
role = carrier
holder = carrier-1
chain = $HOME_DIR/chain.tlv
listen = 127.0.0.1:9701
upstream = 127.0.0.1:9501
EOF
  cat > "$HOME_DIR/client-1.conf" <<EOF
role = client
holder = client-1
chain = $HOME_DIR/chain.tlv
peer = 127.0.0.1:9701
seed_file = $IDENT/client-1.seed
every = 5
start_n = 3
step = 2
EOF
  cat > "$HOME_DIR/gateway-1.conf" <<EOF
listen = 127.0.0.1:9801
chain = $HOME_DIR/chain.tlv
court = court-a
seed_file = $IDENT/court-a.seed
facilitator = 0xBaseFacilitatorTBD
EOF
  cat > "$HOME_DIR/solver-1.conf" <<EOF
role = solver
holder = solver-1
chain = $HOME_DIR/chain.tlv
peer = 127.0.0.1:9501
seed_file = $IDENT/solver-1.seed
EOF
  cat > "$HOME_DIR/witness-1.conf" <<EOF
role = witness
holder = witness-1
chain = $HOME_DIR/chain.tlv
peer = 127.0.0.1:9502
seed_file = $IDENT/witness-1.seed
demo = hexagon
EOF
  cat > "$HOME_DIR/client-2.conf" <<EOF
role = client
holder = client-2
chain = $HOME_DIR/chain.tlv
peer = 127.0.0.1:9502
seed_file = $IDENT/client-2.seed
every = 7
start_n = 4
step = 2
EOF
}

start() {
  mkdir -p "$HOME_DIR"
  (cd "$ROOT" && cargo build -q --bin plumbd) || { echo "build failed"; exit 1; }
  ensure_identities
  write_configs
  if [ ! -f "$HOME_DIR/chain.tlv" ]; then
    "$BIN" "$HOME_DIR/genesis.conf" || exit 1
  fi
  GATEWAY_BIN="$ROOT/target/debug/gateway"
  (cd "$ROOT" && cargo build -q --bin gateway) || { echo "gateway build failed"; exit 1; }
  for node in $NODES; do
    if [ -f "$HOME_DIR/$node.pid" ] && kill -0 "$(cat "$HOME_DIR/$node.pid")" 2>/dev/null; then
      echo "$node: already running"
      continue
    fi
    RUN="$BIN"
    [ "$node" = "gateway-1" ] && RUN="$GATEWAY_BIN"
    nohup "$RUN" "$HOME_DIR/$node.conf" >> "$HOME_DIR/$node.log" 2>&1 &
    echo $! > "$HOME_DIR/$node.pid"
    echo "$node: started (pid $!)"
    sleep 0.3
  done
  # One-shots: the native solver answers the posted market; the
  # witness puts an attestation on the record. Both are the composed
  # economy exercising itself at boot. Bounded: on a RESTART the
  # solver's answer is a replay, the court refuses by silence, and
  # that must not hang the boot.
  sleep 1
  timeout 20 "$BIN" "$HOME_DIR/solver-1.conf" >> "$HOME_DIR/solver-1.log" 2>&1 \
    && echo "solver-1: solved the native market (one-shot)" \
    || echo "solver-1: no receipt (already solved on a prior boot, or see solver-1.log)"
  timeout 20 "$BIN" "$HOME_DIR/witness-1.conf" >> "$HOME_DIR/witness-1.log" 2>&1 \
    && echo "witness-1: on the record (one-shot)" \
    || echo "witness-1: FAILED (see witness-1.log)"
  echo "simnet up — status: $0 status"
}

stop() {
  for node in $NODES; do
    if [ -f "$HOME_DIR/$node.pid" ]; then
      kill "$(cat "$HOME_DIR/$node.pid")" 2>/dev/null && echo "$node: stopped"
      rm -f "$HOME_DIR/$node.pid"
    fi
  done
}

status() {
  echo "simnet @ $HOME_DIR"
  for node in $NODES; do
    if [ -f "$HOME_DIR/$node.pid" ] && kill -0 "$(cat "$HOME_DIR/$node.pid")" 2>/dev/null; then
      state="up  (pid $(cat "$HOME_DIR/$node.pid"))"
    else
      state="DOWN"
    fi
    printf '  %-10s %s\n' "$node" "$state"
  done
  echo "  --- courts: LAST 20 sessions (the audit's lesson: lifetime counts hide a dead economy) ---"
  for court in court-a court-b court-c; do
    recent=$(grep 'session closed' "$HOME_DIR/$court.log" 2>/dev/null | tail -20)
    ok=$(printf '%s' "$recent" | grep -c 'credited 1'); ok=${ok:-0}
    dead=$(printf '%s' "$recent" | grep -c 'credited 0, refused 0, skipped 0'); dead=${dead:-0}
    failed=$(grep -c 'session failed' "$HOME_DIR/$court.log" 2>/dev/null); failed=${failed:-0}
    snap=$(stat -c %s "$HOME_DIR/$court.xdct" 2>/dev/null); snap=${snap:-0}
    health="HEALTHY"
    [ "$ok" -eq 0 ] && health="STALLED (no recent credits)"
    printf '  %-10s recent credited: %-3s empty: %-3s failed-ever: %-4s snapshot: %-9s %s\n' \
      "$court" "$ok" "$dead" "$failed" "$snap" "$health"
  done
  carried=$(grep 'carried session' "$HOME_DIR/carrier-1.log" 2>/dev/null | tail -20 | grep -c 'forwarded'); carried=${carried:-0}
  echo "  carrier-1  recent carried sessions: $carried (all forwarded unread)"
  solved=$(grep -c 'solved natively' "$HOME_DIR/solver-1.log" 2>/dev/null); solved=${solved:-0}
  witnessed=$(grep -c 'on the record' "$HOME_DIR/witness-1.log" 2>/dev/null); witnessed=${witnessed:-0}
  gw402=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9801/query 2>/dev/null || echo down)
  echo "  solver-1   native market solutions: $solved   witness-1 records: $witnessed   gateway /query: HTTP $gw402"
}

logs() {
  node="${2:-court-a}"
  tail -n 20 "$HOME_DIR/$node.log"
}

reset() {
  stop
  rm -rf "$HOME_DIR"
  echo "simnet state cleared"
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  logs) logs "$@" ;;
  reset) reset ;;
  *) echo "usage: $0 start | stop | status | logs [node] | reset"; exit 2 ;;
esac
