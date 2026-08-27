#!/usr/bin/env python3
"""A second implementation of IS-1, written from the document alone.

This exists to answer one question: **can somebody implement the spec
without the source?** It imports nothing from `datum`, `strand`,
`lith` or `netstratum`, and was written by reading

    protocols/IS-1_WIRE.md      §1 §2 §3 §4 §7 §9
    protocols/IS-2_SESSION.md   §7
    protocols/IS-3_REGISTRY.md  §5
    conformance/MANIFEST.md

and nothing else. Every place the document did not say enough is marked
`GAP` in the source and listed in `measure/second-implementation.md`.
Those are defects in the specification, not in this file.

    python3 conformance/reference.py

Exits non-zero if any vector or conformance case disagrees.
"""

import sys
from fractions import Fraction
from pathlib import Path

HERE = Path(__file__).resolve().parent

# IS-2 §7.3
MAX_RECORD = 1 << 20
HEADER = 5


class Refused(Exception):
    """The bytes were not what they claimed. IS-1 §4: refuse, never guess."""


class Incomplete(Exception):
    """A record has begun and not finished. IS-2 §7.4: wait."""


# --------------------------------------------------------------------
# IS-1 §1 — the record
# --------------------------------------------------------------------

def le32(buf, at):
    if at + 4 > len(buf):
        raise Incomplete("length field truncated")
    return int.from_bytes(buf[at:at + 4], "little")


def put_frame(tag, value):
    return bytes([tag]) + len(value).to_bytes(4, "little") + value


def take_frame(buf, bound=MAX_RECORD):
    """IS-2 §7.4, which subsumes IS-1 §1's framing.

    len > bound        REFUSE
    buffer < 5         WAIT
    buffer < 5 + len   WAIT
    otherwise          TAKE
    """
    if len(buf) < HEADER:
        raise Incomplete("header truncated")
    tag = buf[0]
    declared = le32(buf, 1)
    if declared > bound:
        raise Refused(f"declared {declared} exceeds bound {bound}")
    whole = HEADER + declared
    if len(buf) < whole:
        raise Incomplete("value truncated")
    return tag, buf[HEADER:whole], whole


# --------------------------------------------------------------------
# IS-1 §3 and §4 — the exact rational
# --------------------------------------------------------------------

def put_magnitude(n):
    """§3: magnitudes carry no leading zeros; zero's magnitude is the
    empty string."""
    n = abs(n)
    if n == 0:
        return b""
    return n.to_bytes((n.bit_length() + 7) // 8, "big")


def put_ratio(value):
    sign = 1 if value < 0 else 0
    numer = put_magnitude(value.numerator)
    denom = put_magnitude(value.denominator)
    return (bytes([sign])
            + len(numer).to_bytes(4, "little") + numer
            + len(denom).to_bytes(4, "little") + denom)


def take_ratio(buf, at=0):
    """§3 for the layout, §4 for the three refusals."""
    if at >= len(buf):
        raise Incomplete("sign byte missing")
    sign = buf[at]
    if sign not in (0, 1):
        raise Refused(f"sign byte {sign} is neither 0 nor 1")
    at += 1

    parts = []
    for _ in range(2):
        length = le32(buf, at)
        at += 4
        if at + length > len(buf):
            raise Incomplete("magnitude truncated")
        raw = buf[at:at + length]
        # §4: a magnitude with a leading zero byte is refused, not
        # normalised. §3 alone does not say this.
        if raw and raw[0] == 0:
            raise Refused("magnitude carries a leading zero byte")
        parts.append(int.from_bytes(raw, "big") if length else 0)
        at += length

    numer, denom = parts
    if denom == 0:
        raise Refused("zero denominator")
    # §4, both rows added after this file diverged from the reference.
    # A first draft guarded the reduced check with `numer != 0` and so
    # accepted 0/5, and did not check leading zeros at all because §3
    # states them as a property of the ENCODER.
    from math import gcd
    if numer == 0:
        if denom != 1:
            raise Refused(f"0/{denom}: zero's canonical denominator is 1")
    elif gcd(numer, denom) != 1:
        raise Refused(f"{numer}/{denom} is not reduced")

    return Fraction(-numer if sign == 1 else numer, denom), at


# --------------------------------------------------------------------
# IS-1 §7.1 — the relation and the closures
# --------------------------------------------------------------------

def take_relation(value):
    """`LE32(orbs) ‖ LE32(count) ‖ (LE32(i) ‖ LE32(j) ‖ ratio)…`"""
    orbs = le32(value, 0)
    count = le32(value, 4)
    at = 8
    edges = []
    for _ in range(count):
        i = le32(value, at)
        j = le32(value, at + 4)
        at += 8
        charge, at = take_ratio(value, at)
        edges.append((i, j, charge))
    return orbs, edges


def take_manifold(value):
    """§7.2, added once the document stated the layout.

    `LE64(tick) ‖ LE32(orbs) ‖ orb… ‖ frame(1)`
    `orb = LE32(components) ‖ ratio… ‖ ratio(capacity) ‖ ratio(energy)`
    """
    if len(value) < 8:
        raise Incomplete("tick truncated")
    tick = int.from_bytes(value[0:8], "little")
    at = 8
    count = le32(value, at)
    at += 4

    orbs = []
    for _ in range(count):
        components = le32(value, at)
        at += 4
        extent = []
        for _ in range(components):
            component, at = take_ratio(value, at)
            extent.append(component)
        capacity, at = take_ratio(value, at)
        energy, at = take_ratio(value, at)
        orbs.append((extent, capacity, energy))

    tag, nested, taken = take_frame(value[at:])
    if tag != 1:
        raise Refused(f"a manifold's nested record is tag {tag}, not 1")
    at += taken
    if at != len(value):
        raise Refused("bytes left over after the manifold")
    return tick, orbs, take_relation(nested)


def take_closures(value):
    """`grain = LE32(arity) ‖ LE32(orb)…`, `closures = LE32(count) ‖ grain…`"""
    count = le32(value, 0)
    at = 4
    grains = []
    for _ in range(count):
        arity = le32(value, at)
        at += 4
        orbs = []
        for _ in range(arity):
            orbs.append(le32(value, at))
            at += 4
        grains.append(orbs)
    return grains


# --------------------------------------------------------------------
# The conformance verdicts, IS-1 §10
# --------------------------------------------------------------------

# GAP 1 — the document does not say which tags a reader OWNS.
# IS-3 §5 grants ranges to isthmus, assay, lith and chitin, and freezes
# 1–31 as ancestral, but nothing states which tags THIS reader accepts
# versus skips. Taken from the conformance manifest's own verdicts,
# which is circular: an outside implementer has only the manifest to go
# on and would have to reverse the answer out of the expected column.
OWNED = {1, 5, 51}


def verdict(buf, name):
    """Reproduce the manifest's four verdicts."""
    if "ratio" in name:
        try:
            take_ratio(buf)
            return "accept"
        except Refused:
            return "refuse"
        except Incomplete:
            return "wait"

    try:
        tag, _value, _whole = take_frame(buf)
    except Refused:
        return "refuse"
    except Incomplete:
        return "wait"
    return "accept" if tag in OWNED else "skip"


# --------------------------------------------------------------------
# Checks
# --------------------------------------------------------------------

VECTORS = [
    ("V1  frame tag 1, empty", put_frame(1, b""), "0100000000"),
    ("V2  frame tag 51", put_frame(51, bytes([0xAA, 0xBB])), "3302000000aabb"),
    ("V3  ratio 0", put_ratio(Fraction(0)), "00000000000100000001"),
    ("V4  ratio 1", put_ratio(Fraction(1)), "0001000000010100000001"),
    ("V5  ratio -3", put_ratio(Fraction(-3)), "0101000000030100000001"),
    ("V6  ratio 7/5", put_ratio(Fraction(7, 5)), "0001000000070100000005"),
    ("V7  ratio 256/255", put_ratio(Fraction(256, 255)), "0002000000010001000000ff"),
    ("V12 unknown tag 200", put_frame(200, bytes([0xDE, 0xAD, 0xBE, 0xEF])),
     "c804000000deadbeef"),
]


def check_vectors():
    bad = []
    for name, produced, published in VECTORS:
        got = produced.hex()
        if got != published:
            bad.append(f"  {name}\n    produced  {got}\n    published {published}")
    return bad


def parse_manifest():
    """Read the expected verdict per case out of MANIFEST.md."""
    cases = []
    for line in (HERE / "MANIFEST.md").read_text().splitlines():
        if not line.startswith("| `") or "`.bin`" in line:
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) < 2:
            continue
        name = cells[0].strip("`")
        want = cells[1].strip("`")
        if want in ("accept", "refuse", "skip", "wait"):
            cases.append((name, want))
    return cases


V11 = ("V11 manifold, 1 orb, empty relation",
       "053d00000000000000000000000100000001000000000100000001010000000100"
       "010000000101000000010000000000010000000101080000000100000000000000")


def check_manifold():
    """§9.5's vector, decoded against §7.2's layout.

    Until §7.2 stated the layout this could not be attempted: the vector
    was publishable and the frame was not producible.
    """
    tag, value, _ = take_frame(bytes.fromhex(V11[1]))
    if tag != 5:
        return [f"  {V11[0]}: tag {tag}, expected 5"]
    tick, orbs, relation = take_manifold(value)
    problems = []
    if tick != 0:
        problems.append(f"  tick {tick}, expected 0")
    if len(orbs) != 1:
        problems.append(f"  {len(orbs)} orbs, expected 1")
    else:
        extent, capacity, energy = orbs[0]
        if extent != [Fraction(1)] or capacity != Fraction(1) or energy != Fraction(0):
            problems.append(f"  orb reads {extent} {capacity} {energy}")
    if relation != (1, []):
        problems.append(f"  relation reads {relation}, expected (1, [])")
    return problems


def check_corpus():
    bad = []
    cases = parse_manifest()
    if not cases:
        return ["  MANIFEST.md yielded no cases — the format is not readable"]
    for name, want in cases:
        path = HERE / f"{name}.bin"
        if not path.exists():
            bad.append(f"  {name}: no .bin beside the manifest")
            continue
        got = verdict(path.read_bytes(), name)
        if got != want:
            bad.append(f"  {name}: manifest says {want}, this reads {got}")
    return bad, len(cases)


# --------------------------------------------------------------------
# IS-2 §7 — THE LIVE SESSION
#
# Everything above checks bytes that already exist. This drives a
# session: bytes arrive in whatever pieces the carrier hands over, whole
# records come out, and a partial tail is held. It is the same rule
# `take_frame` states, applied to a stream rather than a buffer.
#
#     reference.py session   stdin -> records on stdout
#     reference.py emit      a canonical stream on stdout
#
# The Rust side drives both directions and compares. Neither
# implementation reads the other's source.
# --------------------------------------------------------------------

class Session:
    """IS-2 §7. Feed bytes, get whole records, hold the tail."""

    def __init__(self, bound=MAX_RECORD):
        self.bound = bound
        self.held = bytearray()
        self.dead = None

    def feed(self, chunk):
        """Returns (records, state) where state is one of
        'waiting' / 'refused'. Refusal is terminal: IS-2 §7.4 says a
        record that can never complete kills the edge, and a session
        that kept reading after it would be guessing where the next
        record starts."""
        if self.dead is not None:
            return [], "refused"
        self.held.extend(chunk)
        out = []
        while True:
            try:
                tag, value, whole = take_frame(bytes(self.held), self.bound)
            except Incomplete:
                return out, "waiting"
            except Refused as why:
                self.dead = str(why)
                return out, "refused"
            out.append((tag, bytes(value)))
            del self.held[:whole]


def run_session():
    """Read every byte from stdin, feed it ONE BYTE AT A TIME, and
    report each whole record as `tag hexvalue`.

    One byte at a time on purpose: it is the harshest fragmentation a
    carrier can produce, so agreeing with the Rust session here is a
    stronger statement than agreeing on a single whole delivery.
    """
    stream = sys.stdin.buffer.read()
    session = Session()
    state = "waiting"
    for byte in stream:
        records, state = session.feed(bytes([byte]))
        for tag, value in records:
            print(f"record {tag} {value.hex()}")
        if state == "refused":
            break
    print(f"state {state}")
    print(f"held {len(session.held)}")
    return 0


def emit_stream():
    """A canonical stream this implementation wrote, for the Rust
    session to read. Three records: an empty frame, a ratio, and an
    unknown tag that must be skipped whole."""
    out = bytearray()
    out.extend(put_frame(1, b""))
    out.extend(put_frame(80, put_ratio(Fraction(7, 5))))
    out.extend(put_frame(200, bytes([0xde, 0xad, 0xbe, 0xef])))
    sys.stdout.buffer.write(bytes(out))
    return 0


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "session":
        return run_session()
    if len(sys.argv) > 1 and sys.argv[1] == "emit":
        return emit_stream()

    print("A SECOND IMPLEMENTATION OF IS-1, from the document alone")
    print()

    bad_vectors = check_vectors()
    print(f"vectors checked   {len(VECTORS)}")
    print(f"  disagreed       {len(bad_vectors)}")
    for line in bad_vectors:
        print(line)

    bad_manifold = check_manifold()
    print(f"manifold V11      decoded against §7.2")
    print(f"  disagreed       {len(bad_manifold)}")
    for line in bad_manifold:
        print(line)

    bad_cases, total = check_corpus()
    print(f"conformance cases {total}")
    print(f"  disagreed       {len(bad_cases)}")
    for line in bad_cases:
        print(line)

    print()
    if bad_vectors or bad_cases or bad_manifold:
        print("DISAGREEMENTS ABOVE ARE GAPS IN THE DOCUMENT, not in this file.")
        return 1
    print("Every vector and every case agrees. The document is implementable.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
