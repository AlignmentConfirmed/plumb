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
SEED1="0101010101010101010101010101010101010101010101010101010101010101"
SEED2="0202020202020202020202020202020202020202020202020202020202020202"
NODES="court-a court-b court-c carrier-1 client-1 client-2"

write_configs() {
  cat > "$HOME_DIR/genesis.conf" <<EOF
role    = genesis
holder  = court-a
grant   = court-b
grant   = court-c
grant   = carrier-1
grant   = client-1
grant   = client-2
bind    = client-1:$SEED1
bind    = client-2:$SEED2
declare = court-a
out     = $HOME_DIR/chain.tlv
EOF
  cat > "$HOME_DIR/court-a.conf" <<EOF
role = court
holder = court-a
chain = $HOME_DIR/chain.tlv
listen = 127.0.0.1:9501
require_signatures = true
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
seed = $SEED1
every = 5
start_n = 3
step = 2
EOF
  cat > "$HOME_DIR/client-2.conf" <<EOF
role = client
holder = client-2
chain = $HOME_DIR/chain.tlv
peer = 127.0.0.1:9502
seed = $SEED2
every = 7
start_n = 4
step = 2
EOF
}

start() {
  mkdir -p "$HOME_DIR"
  (cd "$ROOT" && cargo build -q --bin plumbd) || { echo "build failed"; exit 1; }
  write_configs
  if [ ! -f "$HOME_DIR/chain.tlv" ]; then
    "$BIN" "$HOME_DIR/genesis.conf" || exit 1
  fi
  for node in $NODES; do
    if [ -f "$HOME_DIR/$node.pid" ] && kill -0 "$(cat "$HOME_DIR/$node.pid")" 2>/dev/null; then
      echo "$node: already running"
      continue
    fi
    nohup "$BIN" "$HOME_DIR/$node.conf" >> "$HOME_DIR/$node.log" 2>&1 &
    echo $! > "$HOME_DIR/$node.pid"
    echo "$node: started (pid $!)"
    sleep 0.3
  done
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
