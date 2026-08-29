//! Local admission scheduling — the load physics, kept strictly apart
//! from the settlement physics.
//!
//! The court verifies correctness by exact algebra and credits by
//! content address; NONE of that may depend on how busy a machine is,
//! because a federation cannot agree on load (`bounty.rs`: "never cpu
//! cycles, never memory — machine facts are unverifiable by a
//! federation"). So this module governs exactly one thing — **who is
//! served, when, in what order** — and touches nothing that reaches a
//! `RewardAct`. A test in this module holds that boundary by reading
//! its own source, the way `assay` guards against floating point.
//!
//! Three pieces:
//! - [`Governor`] — decides admit / busy / reject from queue load. The
//!   default [`StaticGovernor`] sizes the worker pool from the
//!   machine's own core count (leaving headroom, so a court never pegs
//!   the CPU), and sheds load with a named **busy** answer rather than
//!   a silent drop.
//! - [`FairQueue`] — a bounded, **per-key round-robin** queue, so one
//!   flooding party cannot starve a quiet one out of the worker pool.
//! - [`BUSY_TAG`] — the wire tag a swamped court answers with, so a
//!   producer *waits* cooperatively instead of stalling on a timeout.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::{Condvar, Mutex};

/// The record tag a court sends when it is shedding load: a producer
/// that reads this as its first frame backs off for the carried
/// `retry_after_secs` (LE u32) instead of proceeding into a handshake
/// that will not be serviced. Canonical home is `sdk::submit` (a leaf
/// both the court and a datum-free producer share); re-exported here so
/// `crate::sched::BUSY_TAG` still resolves.
pub use sdk::submit::BUSY_TAG;

/// What the governor decided for one inbound connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Serve it — enqueue for the worker pool.
    Admit,
    /// Shed it, politely: tell the peer to retry after this many
    /// seconds. Not a drop — the peer learns to wait.
    Busy {
        /// How long the peer should back off.
        retry_after_secs: u32,
    },
    /// Refuse outright (reserved for a future hard-reject policy; the
    /// default governor never returns this).
    Reject,
}

/// The queue load a [`Governor`] decides from. Deliberately minimal —
/// depth only, never a machine fact that could leak into consensus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Load {
    /// Total items queued across all keys.
    pub queued: usize,
    /// Items queued for this connection's own key (its IP).
    pub queued_for_key: usize,
}

/// The admission policy. A trait so the machine-sizing default can be
/// swapped for a load-reading one without the court knowing.
pub trait Governor: Send + Sync {
    /// Admit, shed (busy), or reject one connection at this load.
    fn admit(&self, load: &Load) -> Admission;
    /// How many sessions may run concurrently — the worker-pool size,
    /// and the hard ceiling on how much CPU the court can occupy.
    fn worker_target(&self) -> usize;
}

/// The default policy: size the pool to the machine, shed past a
/// bounded queue with a named busy answer.
#[derive(Debug, Clone, Copy)]
pub struct StaticGovernor {
    workers: usize,
    queue_bound: usize,
    per_key_bound: usize,
    retry_after_secs: u32,
}

impl StaticGovernor {
    /// Tuned from the host's core count: workers = cores − 1 (at least
    /// one), so the accept loop and the rest of the machine always
    /// keep a core. The queue absorbs short bursts; past it, peers are
    /// told to wait, never dropped.
    #[must_use]
    pub fn tuned() -> Self {
        let cores = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        let workers = cores.saturating_sub(1).max(1);
        Self {
            workers,
            queue_bound: workers.saturating_mul(8).max(8),
            per_key_bound: workers.saturating_mul(2).max(2),
            retry_after_secs: 1,
        }
    }

    /// Explicit bounds — for tests and for a deployment that wants to
    /// pin them rather than derive from cores.
    #[must_use]
    pub fn with(workers: usize, queue_bound: usize, per_key_bound: usize) -> Self {
        Self {
            workers: workers.max(1),
            queue_bound: queue_bound.max(1),
            per_key_bound: per_key_bound.max(1),
            retry_after_secs: 1,
        }
    }
}

impl Governor for StaticGovernor {
    fn admit(&self, load: &Load) -> Admission {
        // Per-key first: a single flooding IP is shed before it can
        // fill the shared queue and crowd everyone else out.
        if load.queued_for_key >= self.per_key_bound || load.queued >= self.queue_bound {
            return Admission::Busy {
                retry_after_secs: self.retry_after_secs,
            };
        }
        Admission::Admit
    }

    fn worker_target(&self) -> usize {
        self.workers
    }
}

/// A load-reading governor (Linux): the static policy, tightened when
/// the OS load average says the machine is already saturated. Opt-in;
/// the court defaults to [`StaticGovernor`]. Load is read as an
/// integer (the part before the decimal) to stay float-free and
/// coarse — this is a throttle, not a measurement instrument.
#[derive(Debug, Clone, Copy)]
pub struct ResourceGovernor {
    base: StaticGovernor,
}

impl ResourceGovernor {
    /// Wrap the tuned static policy with load-awareness.
    #[must_use]
    pub fn tuned() -> Self {
        Self {
            base: StaticGovernor::tuned(),
        }
    }

    /// The integer part of the 1-minute load average, or 0 if it
    /// cannot be read (a non-Linux host, or no `/proc`): absence of a
    /// signal never tightens the wall.
    fn load_integer() -> usize {
        let Ok(text) = std::fs::read_to_string("/proc/loadavg") else {
            return 0;
        };
        text.split_whitespace()
            .next()
            .and_then(|tok| tok.split('.').next())
            .and_then(|whole| whole.parse::<usize>().ok())
            .unwrap_or(0)
    }
}

impl Governor for ResourceGovernor {
    fn admit(&self, load: &Load) -> Admission {
        // When the machine is already at or past one busy core per
        // worker, halve the effective queue depth before shedding —
        // the court yields the machine rather than fighting for it.
        let saturated = Self::load_integer() >= self.base.workers;
        let effective = if saturated {
            StaticGovernor::with(
                self.base.workers,
                self.base.queue_bound / 2,
                self.base.per_key_bound,
            )
        } else {
            self.base
        };
        effective.admit(load)
    }

    fn worker_target(&self) -> usize {
        self.base.worker_target()
    }
}

/// How much scheduling priority a posted escrow buys (Phase 5, #52).
///
/// `weight(escrow) = base + k·√escrow`, hard-capped at `ceiling`. Three
/// properties, each load-bearing:
/// - **Concave** (`√`, via `u128::isqrt`): doubling a stake buys *less*
///   than double the priority — diminishing returns, so a whale cannot
///   dominate the line in proportion to its capital.
/// - **Floored** (`base ≥ 1`): a zero-escrow holder still has positive
///   weight, so the weighted queue never *starves* the unstaked — it
///   serves them less often, never not at all.
/// - **Capped** (`ceiling`): priority is bounded above, so no stake, however
///   large, buys unbounded precedence.
///
/// It takes the escrow amount as a plain integer and returns a plain
/// integer. It reads no [`reward`](crate::reward) book and names no
/// settlement type on purpose: the *amount* may be a consensus fact, but
/// the *weight a court derives from it* is local scheduling policy and
/// must never re-enter consensus — the same boundary the module's
/// source-scan test holds. The caller reads `escrow_of(holder)` and
/// hands the number in; this type only does the arithmetic.
///
/// Float-free throughout (`assay`'s no-float discipline in spirit,
/// though only `assay` is scanned for it): `u128::isqrt` is exact and
/// stable since Rust 1.84.
#[derive(Debug, Clone, Copy)]
pub struct EscrowWeight {
    base: u64,
    k: u64,
    ceiling: u64,
}

impl EscrowWeight {
    /// A default schedule: floor of 1 (nobody starves), unit slope, and
    /// a ceiling of 64 (the largest stake is served at most 64× as often
    /// as an unstaked holder — bounded plutocracy).
    #[must_use]
    pub fn tuned() -> Self {
        Self {
            base: 1,
            k: 1,
            ceiling: 64,
        }
    }

    /// A weight schedule with explicit parameters. `ceiling` is the hard
    /// cap on the returned weight; `base` is the floor at zero escrow.
    #[must_use]
    pub fn with(base: u64, k: u64, ceiling: u64) -> Self {
        Self { base, k, ceiling }
    }

    /// The scheduling weight for a holder currently staking `escrow`.
    /// Saturating and capped: never panics, never exceeds `ceiling`.
    #[must_use]
    pub fn of(&self, escrow: u128) -> u64 {
        let bonus = u128::from(self.k).saturating_mul(escrow.isqrt());
        let raw = u128::from(self.base).saturating_add(bonus);
        let capped = raw.min(u128::from(self.ceiling));
        // capped ≤ ceiling ≤ u64::MAX, so this conversion cannot fail;
        // fall back to the ceiling rather than expect/panic.
        u64::try_from(capped).unwrap_or(self.ceiling)
    }
}

/// The transport physics of the scheduling fibre (Phase 5, #55).
///
/// The scheduler is a vector bundle π: E → M. The base `M` is the
/// ledger's exact incidence geometry; the fibre `E_x` is a solver's
/// ephemeral transit state. This type computes the three integer
/// quantities that move a claim across the fibre, and nothing else —
/// it reads no settlement book and names no settlement type, the same
/// boundary the module's source-scan test holds.
///
/// - **Quantum** `Q(escrow, torsion) = EscrowWeight::of(escrow)·(1+torsion)`,
///   capped. The escrow term is the static base point on `M` (concave,
///   floored, bounded — see [`EscrowWeight`]); the `(1+torsion)` factor is
///   the torsion of the *open boundary currently in the fibre* (#58,
///   Directive 1). Because the lift attaches to the active claim and not
///   the actor, a solver working a **flat domain** (`torsion = 0`) has the
///   quantum collapse to its bare escrow floor — priority follows the
///   geometry a solver is transporting right now, never accumulated
///   history. This is what closes incumbent-farming while preserving
///   "not a name."
/// - **Curvature** `Γ = |supp(C) ∩ contending|` (#59, Directive 2): the
///   cardinality of the shared algebraic support between a claim and the
///   generators of the concurrently active domains. A monotone integer
///   upper bound on the homological intersection (disjoint supports force
///   `Γ = 0` exactly), computed as an O(N) set intersection rather than a
///   matrix reduction on the hot path.
/// - **Cost** `1 + Γ` (Directive 1, flat unit cost): every serve is one
///   unit of discrete transit time, dilated only by the curvature it
///   meets. Chain magnitude does *not* enter — that is a spatial property
///   already priced at consensus; charging it here would double-charge
///   spatial complexity and drag the velocity vector that escrow and
///   torsion alone are meant to set.
///
/// Curvature is a **read-only kinematic variable**: it reorders transit,
/// it never modulates yield. A solver that pivots to a novel generator
/// meets `Γ = 0` and is handed maximum un-dilated velocity — the reward
/// for orthogonality is throughput, paid by the physics of the fibre, not
/// a number minted on the manifold (which would violate the invariant).
#[derive(Debug, Clone, Copy)]
pub struct Transport {
    weight: EscrowWeight,
    ceiling: u64,
}

impl Transport {
    /// A tuned transport: the tuned escrow weight, quantum capped at 4096
    /// (the escrow ceiling of 64 lifted by a torsion factor of up to 64),
    /// so no boundary, however knotted, buys unbounded velocity.
    #[must_use]
    pub fn tuned() -> Self {
        Self {
            weight: EscrowWeight::tuned(),
            ceiling: 4096,
        }
    }

    /// Explicit parameters — for tests and pinned deployments.
    #[must_use]
    pub fn with(weight: EscrowWeight, ceiling: u64) -> Self {
        Self {
            weight,
            ceiling: ceiling.max(1),
        }
    }

    /// The per-round serving quantum for a holder staking `escrow`,
    /// transporting a boundary of torsion `torsion`. Saturating and
    /// capped: never panics, never exceeds the ceiling. A flat boundary
    /// (`torsion = 0`) lifts by ×1, collapsing the quantum to the bare
    /// escrow weight.
    #[must_use]
    pub fn quantum(&self, escrow: u128, torsion: u64) -> u64 {
        let base = self.weight.of(escrow);
        let lift = torsion.saturating_add(1); // flat domain → ×1
        base.saturating_mul(lift).min(self.ceiling)
    }

    /// The curvature `Γ` a claim meets in a fibre already carrying the
    /// `contending` generators: `Γ = |supp(C) ∩ contending|`. `support`
    /// is the claim's non-zero basis vectors (declared axioms, sublet
    /// indices, vocabulary tags), assumed deduplicated. O(N) in the
    /// support size; the `contending` set is a [`BTreeSet`] so membership
    /// is a log-time probe, and iteration order never enters the count.
    #[must_use]
    pub fn curvature(support: &[u32], contending: &BTreeSet<u32>) -> u64 {
        let hits = support.iter().filter(|g| contending.contains(g)).count();
        u64::try_from(hits).unwrap_or(u64::MAX)
    }

    /// The discrete transport cost of one serve at curvature `gamma`:
    /// exactly `1 + Γ`. Orthogonal transit (`Γ = 0`) costs the
    /// mathematical floor of 1; overlap with concurrently active domains
    /// dilates the cost — and so the transit time — linearly in the
    /// interference. Saturating, so pathological curvature cannot wrap.
    #[must_use]
    pub fn cost(gamma: u64) -> u64 {
        gamma.saturating_add(1)
    }
}

/// The kinematic fibre `E_x` over one holder: its deficit, carried across
/// rounds. This is the **entire** persistent scheduler state for a holder.
/// The base coordinate — escrow — is projected from the ledger at
/// section-time via `escrow_of`, never shadowed here (#57): no holder
/// identity, no escrow amount, no settlement of any kind lives in this
/// cell, only ephemeral integer transit credit.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fibre {
    deficit: u64,
}

impl Fibre {
    /// A fresh fibre with zero deficit.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Grant this fibre its quantum at the start of a turn: `V += Q`.
    /// Saturating, so an unbounded run of turns cannot overflow the cell.
    pub fn grant(&mut self, quantum: u64) {
        self.deficit = self.deficit.saturating_add(quantum);
    }

    /// Attempt one serve at transport `cost`. If the carried deficit
    /// covers it, spend it and return `true`; otherwise leave the deficit
    /// intact — it carries into the next round — and return `false`.
    ///
    /// Carrying the remainder forward is the anti-starvation guarantee:
    /// the quantum is floored at ≥ 1, so after enough rounds the deficit
    /// clears any finite `1 + Γ`. Even a maximally congested solver is
    /// served — later, never *never*. (Contrast a hard priority cap, which
    /// would wall the congested solver out entirely.)
    #[must_use]
    pub fn try_serve(&mut self, cost: u64) -> bool {
        if self.deficit >= cost {
            self.deficit -= cost;
            true
        } else {
            false
        }
    }

    /// The current carried deficit — the fibre's transit credit.
    #[must_use]
    pub fn deficit(&self) -> u64 {
        self.deficit
    }
}

struct Inner<K, T> {
    queues: HashMap<K, VecDeque<T>>,
    order: VecDeque<K>,
    total: usize,
}

/// A bounded, per-key round-robin work queue. One producer (the accept
/// loop) offers; many workers take. Round-robin over keys is the
/// anti-starvation guarantee: a key with a hundred items queued does
/// not delay a key with one.
pub struct FairQueue<K: Eq + std::hash::Hash + Clone, T> {
    inner: Mutex<Inner<K, T>>,
    available: Condvar,
}

impl<K: Eq + std::hash::Hash + Clone, T> Default for FairQueue<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + std::hash::Hash + Clone, T> FairQueue<K, T> {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                queues: HashMap::new(),
                order: VecDeque::new(),
                total: usize::from(false), // 0, without a bare literal
            }),
            available: Condvar::new(),
        }
    }

    /// The current load for `key` — what the governor decides from.
    /// A poisoned lock reports "full" so the court sheds rather than
    /// guesses.
    #[must_use]
    pub fn load(&self, key: &K) -> Load {
        match self.inner.lock() {
            Ok(inner) => Load {
                queued: inner.total,
                queued_for_key: inner.queues.get(key).map_or(0, VecDeque::len),
            },
            Err(_) => Load {
                queued: usize::MAX,
                queued_for_key: usize::MAX,
            },
        }
    }

    /// Enqueue an item under `key`. The accept loop is the only
    /// caller, and it has already consulted the governor, so this does
    /// not re-check bounds — it records and wakes one worker.
    pub fn offer(&self, key: K, item: T) {
        if let Ok(mut inner) = self.inner.lock() {
            let empty_before = inner.queues.get(&key).is_none_or(VecDeque::is_empty);
            inner.queues.entry(key.clone()).or_default().push_back(item);
            inner.total = inner.total.saturating_add(1);
            if empty_before {
                inner.order.push_back(key);
            }
        }
        self.available.notify_one();
    }

    fn pop_locked(inner: &mut Inner<K, T>) -> Option<(K, T)> {
        let key = inner.order.pop_front()?;
        let item = inner.queues.get_mut(&key).and_then(VecDeque::pop_front)?;
        inner.total = inner.total.saturating_sub(1);
        let has_more = inner.queues.get(&key).is_some_and(|q| !q.is_empty());
        if has_more {
            inner.order.push_back(key.clone());
        } else {
            inner.queues.remove(&key);
        }
        Some((key, item))
    }

    /// Non-blocking take — the next item by round-robin, or `None` if
    /// empty. Used by tests; workers call [`FairQueue::take_blocking`].
    #[must_use]
    pub fn take(&self) -> Option<(K, T)> {
        let mut inner = self.inner.lock().ok()?;
        Self::pop_locked(&mut inner)
    }

    /// Block until an item is available, then take it by round-robin.
    /// `None` only if the lock is poisoned.
    #[must_use]
    pub fn take_blocking(&self) -> Option<(K, T)> {
        let mut inner = self.inner.lock().ok()?;
        loop {
            if let Some(next) = Self::pop_locked(&mut inner) {
                return Some(next);
            }
            inner = self.available.wait(inner).ok()?;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn a_flooding_key_cannot_starve_a_quiet_one() {
        let q: FairQueue<&str, u32> = FairQueue::new();
        // A floods three, B offers one after.
        q.offer("A", 1);
        q.offer("A", 2);
        q.offer("A", 3);
        q.offer("B", 9);
        // Round-robin: B is served SECOND, not stuck behind all of A.
        let order: Vec<(&str, u32)> = std::iter::from_fn(|| q.take()).collect();
        assert_eq!(order, vec![("A", 1), ("B", 9), ("A", 2), ("A", 3)]);
    }

    #[test]
    fn escrow_weight_is_concave_diminishing_returns_not_merely_sub_additive() {
        let w = EscrowWeight::with(1, 1, 1_000_000);
        // True midpoint concavity: weight(a) + weight(b) < 2·weight(mid)
        // for the arithmetic midpoint. A LINEAR schedule (base + k·e)
        // satisfies this only as EQUALITY, so a strict `<` fails the
        // moment the √ is removed — this is what makes the test
        // load-bearing, where a plain "doubling buys less than double"
        // would pass even for a linear function because of the base
        // floor. Perfect squares keep integer isqrt exact at the ends.
        for &(a, mid, b) in &[
            (100_u128, 500, 900),
            (10_000, 50_000, 90_000),
            (250_000, 625_000, 1_000_000),
        ] {
            let ends = u128::from(w.of(a)) + u128::from(w.of(b));
            let twice_mid = 2 * u128::from(w.of(mid));
            assert!(
                ends < twice_mid,
                "weight({a})+weight({b}) = {ends} is not < 2·weight({mid}) = {twice_mid}: \
                 the schedule is not strictly concave"
            );
        }
    }

    #[test]
    fn escrow_weight_saturates_at_the_ceiling_no_stake_buys_unbounded_priority() {
        let w = EscrowWeight::with(1, 1, 64);
        // A stake large enough to blow past the cap, and the largest
        // representable stake, both clamp to exactly the ceiling.
        assert_eq!(w.of(10_000), 64, "10_000 → 1 + 100 = 101, must cap at 64");
        assert_eq!(w.of(u128::MAX), 64, "no stake buys more than the cap");
    }

    #[test]
    fn escrow_weight_floors_at_base_so_the_unstaked_are_never_starved() {
        let w = EscrowWeight::tuned();
        // Zero escrow still yields positive weight: the weighted queue
        // serves the unstaked less often, never not at all.
        assert_eq!(w.of(0), 1);
        assert!(w.of(0) > 0);
    }

    /// A single solver's throughput: run `rounds` deficit round-robin
    /// turns, granted `quantum` each round and paying `cost` per serve,
    /// carrying the deficit across rounds. Returns items settled — the
    /// solver's transit velocity integrated over the window.
    fn throughput(quantum: u64, cost: u64, rounds: u64) -> u64 {
        let mut fibre = Fibre::new();
        let mut served = 0;
        for _ in 0..rounds {
            fibre.grant(quantum);
            while fibre.try_serve(cost) {
                served += 1;
            }
        }
        served
    }

    #[test]
    fn orthogonal_transit_costs_the_unit_floor() {
        let t = Transport::tuned();
        // Unstaked solver on a flat domain: the bare floor quantum.
        let q = t.quantum(0, 0);
        assert_eq!(q, 1, "unstaked + flat → unit quantum");
        assert_eq!(Transport::cost(0), 1, "Γ=0 transit is the unit floor");
        // One serve per round, exactly: cost 1, quantum 1.
        assert_eq!(throughput(q, Transport::cost(0), 10), 10);
    }

    #[test]
    fn curvature_dilates_transit_time_relative_to_orthogonal_space() {
        let t = Transport::tuned();
        let q = t.quantum(10_000, 0); // escrow weight caps at 64
        let rounds = 100;
        let orthogonal = throughput(q, Transport::cost(0), rounds);
        let congested = throughput(q, Transport::cost(3), rounds);
        // Velocity is quantum/(1+Γ): 4× the cost → a quarter the
        // throughput in the same wall-time. This is the whole point — and
        // it is exactly what fails if `cost` is pinned to a constant 1
        // (dropping Γ), which collapses congested == orthogonal.
        assert!(congested < orthogonal, "curvature dilates transit time");
        assert_eq!(orthogonal, q * rounds);
        assert_eq!(congested, q * rounds / 4);
    }

    #[test]
    fn torsion_lifts_the_quantum_and_flat_domains_collapse_to_the_escrow_floor() {
        let t = Transport::tuned();
        let escrow = 10_000; // escrow weight caps at 64
        let flat = t.quantum(escrow, 0);
        let knotted = t.quantum(escrow, 3);
        assert_eq!(flat, 64, "flat boundary → bare escrow weight");
        assert_eq!(knotted, 64 * 4, "torsion 3 lifts the quantum ×(1+3)");
        // Directive 1: the lift is a property of the active claim, so the
        // instant a solver reverts to a flat domain its priority collapses
        // back to the escrow base point. Removing the `(1+torsion)` factor
        // makes knotted == flat and fails this.
        assert!(knotted > flat);
    }

    #[test]
    fn a_novel_generator_meets_zero_curvature() {
        // supp(C) disjoint from every concurrently active support → Γ=0,
        // the orthogonal floor: minting a brand-new generator is the most
        // orthogonal move on the board, and the physics hands it maximum
        // un-dilated velocity — no ledger bonus required.
        let contending: BTreeSet<u32> = [1, 2, 3, 5, 8].into_iter().collect();
        assert_eq!(Transport::curvature(&[99], &contending), 0, "novel generator");
        assert_eq!(Transport::curvature(&[2, 3], &contending), 2, "two shared");
        assert_eq!(
            Transport::curvature(&[2, 3, 4], &contending),
            2,
            "the novel 4 adds no curvature"
        );
    }

    #[test]
    fn curvature_is_the_support_intersection_cardinality() {
        let contending: BTreeSet<u32> = [10, 20, 30, 40].into_iter().collect();
        assert_eq!(Transport::curvature(&[], &contending), 0, "empty support");
        assert_eq!(Transport::curvature(&[20, 40], &contending), 2);
        assert_eq!(Transport::curvature(&[10, 20, 30, 40], &contending), 4);
        let none: BTreeSet<u32> = BTreeSet::new();
        assert_eq!(
            Transport::curvature(&[1, 2, 3], &none),
            0,
            "no concurrency, no curvature"
        );
    }

    #[test]
    fn a_congested_solver_is_dilated_but_never_starved() {
        let t = Transport::tuned();
        let q = t.quantum(0, 0); // the weakest solver: unit quantum
        let cost = Transport::cost(7); // heavy congestion → cost 8
        // Deficit carries: a unit-quantum solver at cost 8 accrues 8 over
        // 8 rounds and serves exactly once — slow, never zero.
        assert_eq!(throughput(q, cost, 8), 1);
        // And throughput grows without bound in time: served later, never
        // never. A hard cap instead of a carried deficit would wall it out.
        assert_eq!(throughput(q, cost, 800), 100);
    }

    #[test]
    fn the_tuned_governor_leaves_the_machine_a_core_and_bounds_concurrency() {
        let g = StaticGovernor::tuned();
        assert!(g.worker_target() >= 1, "always at least one worker");
        // Whatever the host, the pool is finite — a court can never
        // occupy unbounded CPU.
        assert!(g.worker_target() < 1_000_000);
    }

    #[test]
    fn the_governor_sheds_with_busy_past_its_bounds_never_silently() {
        let g = StaticGovernor::with(2, 4, 2);
        assert_eq!(g.admit(&Load { queued: 0, queued_for_key: 0 }), Admission::Admit);
        // Per-key bound hit first — a single IP is shed before it fills
        // the shared queue.
        assert!(matches!(
            g.admit(&Load { queued: 1, queued_for_key: 2 }),
            Admission::Busy { .. }
        ));
        // Total queue bound hit.
        assert!(matches!(
            g.admit(&Load { queued: 4, queued_for_key: 0 }),
            Admission::Busy { .. }
        ));
    }

    #[test]
    fn load_reports_total_and_per_key_depth() {
        let q: FairQueue<&str, u32> = FairQueue::new();
        q.offer("A", 1);
        q.offer("A", 2);
        q.offer("B", 3);
        let a = q.load(&"A");
        assert_eq!(a.queued, 3);
        assert_eq!(a.queued_for_key, 2);
        let c = q.load(&"C");
        assert_eq!(c.queued_for_key, 0, "an unseen key has nothing queued");
    }

    /// The consensus boundary, held by reading this module's own
    /// source: scheduling must not name a settlement type. If a future
    /// edit reaches for the reward book or a credit here, this fails —
    /// the same instrument `assay`'s isolation test uses for floats.
    #[test]
    fn scheduling_never_names_a_settlement_type() {
        let src = include_str!("sched.rs");
        // Scan only the module code, not this test's own forbidden-word
        // list (which necessarily names them), and strip line comments
        // so the prose explaining the boundary does not trip the check.
        let module = src.split("#[cfg(test)]").next().unwrap_or(src);
        let code: String = module
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in ["RewardBook", "RewardAct", "credit_claim", "Bounty", "settle_answer"] {
            assert!(
                !code.contains(forbidden),
                "scheduling reached for a settlement type ({forbidden}) — load must never touch consensus"
            );
        }
    }
}
