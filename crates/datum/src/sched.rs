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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
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

/// One coordinate of the fibre: a generator (a basis vector / cell of the
/// combinatorial complex) together with the homological dimension it lives
/// in. The two indices play distinct roles and are never conflated —
/// collapsing them was the dimensional loss #57–#59 remediate:
/// - `gen` is the fibre coordinate: the axis the generator's deficit
///   transports along, and the axis by which **congestion couples** (the
///   diagonal connection is indexed by `gen`).
/// - `dim` is the homological grade `k`: it selects the graded torsion
///   `T_k` that **lifts this axis's quantum**, and nothing else — an `H_1`
///   lift never perturbs an `H_0` axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Axis {
    /// The generator's **global** identity. A `DeclaredComplex`'s cell
    /// indices are local (`0..cells[k]`), so two universes' "cell 3" are
    /// different generators; `gen` is a collision-resistant 64-bit hash of
    /// `(registered-domain tag, dim, cell)` (`datum::geometry`), which
    /// gives cells in the *same* chain-registered universe a shared axis
    /// and cells in different universes disjoint ones. A 64-bit space is
    /// not injective (a `Tag` alone is 64 bits), but curvature is
    /// turn-order-only, so an astronomically-rare collision is a harmless
    /// scheduling hint, never a settlement fault.
    pub gen: u64,
    /// The homological dimension `k` of the generator (which `C_k` it
    /// spans). Selects the graded torsion `T_k`.
    pub dim: u32,
}

impl Axis {
    /// A generator axis at homological grade `dim`.
    #[must_use]
    pub fn new(gen: u64, dim: u32) -> Self {
        Self { gen, dim }
    }
}

/// The transport physics of the scheduling fibre (Phase 5, #55).
///
/// The scheduler is a vector bundle π: E → M. The base `M` is the ledger's
/// exact incidence geometry; the fibre `E_x` is a solver's ephemeral
/// transit state — a **sparse vector over generator-space**, never a
/// scalar (a scalar collapse would contaminate orthogonal axes; #57–#59).
/// This type computes the two integer quantities that move a claim across
/// the fibre, per axis, and nothing else — it reads no settlement book and
/// names no settlement type, the boundary the module's source-scan holds.
///
/// - **Quantum** `Q(escrow, T_k) = EscrowWeight::of(escrow)·(1 + T_k)`,
///   capped, granted to **each active axis** of homological grade `k`. The
///   escrow term is the static base point on `M` (concave, floored,
///   bounded — see [`EscrowWeight`]); the `(1 + T_k)` factor is the graded
///   torsion of the *open boundary currently in the fibre* (#58), applied
///   strictly to that grade's generators. A **flat** boundary (`T_k = 0`)
///   lifts by ×1 — the quantum collapses to the bare escrow floor, so
///   priority follows the geometry a solver transports right now, never
///   accumulated history. Closes incumbent-farming; preserves "not a name."
/// - **Cost** `cost[g] = 1 + inflight[g]` (#59, Directive 2): the
///   **diagonal connection** over generator-space. Transporting a claim
///   spends, on each of its axes `g`, one unit of transit dilated by the
///   multiplicity of concurrently active claims congesting *that* axis.
///   Off-diagonal coupling is deliberately absent: independent generators
///   are causally disjoint propositions, and entangling them would
///   violate the algebraic independence of the basis. Uncontested axes
///   (`inflight[g] = 0`) transport at the un-dilated floor of 1 — and
///   breadth is **not** charged (a claim on ten orthogonal axes still
///   costs ten units of *floor*, never a magnitude penalty; size is priced
///   at consensus, not here).
///
/// Cost is a **read-only kinematic variable**: it reorders transit, it
/// never modulates yield. A pivot to a novel generator meets `inflight = 0`
/// and is handed maximum un-dilated velocity — the reward for orthogonality
/// is throughput, paid by the physics of the fibre, not a number minted on
/// the manifold (which would violate the invariant).
///
/// `depth` is the **flag-filtration bound** `k`: the maximum number of
/// generator-axes a fibre may span at once (contained expansion — the
/// vector fibre cannot grow without limit, `Q(F_p) ⊆ F_p`). It bounds the
/// *fibre*, never the *endpoint*: a claim's boundary is always verified
/// exactly by `assay`, whatever its dimension; `depth` only caps how many
/// axes the ephemeral scheduler *meters*.
#[derive(Debug, Clone, Copy)]
pub struct Transport {
    weight: EscrowWeight,
    ceiling: u64,
    depth: usize,
}

/// The default flag-filtration depth: generous enough that real boundaries
/// are metered in full, tight enough that a solver cannot expand a fibre
/// without bound. Purely a resource cap; it never enters verification.
const TUNED_DEPTH: usize = 256;

impl Transport {
    /// A tuned transport: the tuned escrow weight, per-axis quantum capped
    /// at 4096 (the escrow ceiling of 64 lifted by a torsion grade of up
    /// to 64), and the tuned flag-filtration depth.
    #[must_use]
    pub fn tuned() -> Self {
        Self {
            weight: EscrowWeight::tuned(),
            ceiling: 4096,
            depth: TUNED_DEPTH,
        }
    }

    /// Explicit parameters — for tests and pinned deployments.
    #[must_use]
    pub fn with(weight: EscrowWeight, ceiling: u64) -> Self {
        Self {
            weight,
            ceiling: ceiling.max(1),
            depth: TUNED_DEPTH,
        }
    }

    /// Set the flag-filtration depth `k` (clamped to ≥ 1). Builder-style,
    /// for tests and pinned deployments.
    #[must_use]
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth.max(1);
        self
    }

    /// The flag-filtration depth `k` — the maximum axes a fibre may span.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// The per-axis serving quantum for a holder staking `escrow`,
    /// transporting an axis whose homological grade carries torsion
    /// `torsion_k`. Saturating and capped: never panics, never exceeds the
    /// ceiling. A flat grade (`torsion_k = 0`) lifts by ×1, collapsing the
    /// quantum to the bare escrow weight.
    #[must_use]
    pub fn quantum(&self, escrow: u128, torsion_k: u64) -> u64 {
        let base = self.weight.of(escrow);
        let lift = torsion_k.saturating_add(1); // flat grade → ×1
        base.saturating_mul(lift).min(self.ceiling)
    }

    /// The diagonal transport cost of one serve along an axis congested by
    /// `inflight` concurrently active claims: exactly `1 + inflight`. An
    /// uncontested axis (`inflight = 0`) costs the un-dilated floor of 1;
    /// each concurrent claim on the *same* generator dilates the cost — and
    /// so the transit time — by one. Saturating, so pathological congestion
    /// cannot wrap.
    #[must_use]
    pub fn cost(inflight: u32) -> u64 {
        u64::from(inflight).saturating_add(1)
    }
}

/// The kinematic fibre `E_x` over one holder: a **sparse deficit vector**
/// `V: generator → ℤ`, carried across rounds. This is the entire
/// persistent scheduler state for a holder; the base coordinate — escrow —
/// is projected from the ledger at section-time (#57), never shadowed. No
/// holder identity, no escrow amount, no settlement lives here — only
/// ephemeral per-axis transit credit.
///
/// Indexing by generator (not by homological dimension) is deliberate
/// (#57, Directive 1): projecting the fibre onto `k` would collapse
/// independent basis vectors into a degenerate scalar class, and orthogonal
/// axes could no longer transport without contaminating one another.
#[derive(Debug, Clone, Default)]
pub struct Fibre {
    deficit: BTreeMap<u64, i64>,
}

impl Fibre {
    /// A fresh fibre with an empty deficit vector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Grant `quantum` of transit credit to a single generator axis at the
    /// start of a turn: `V[gen] += Q`. Saturating, so an unbounded run of
    /// turns cannot overflow the coordinate.
    pub fn grant(&mut self, gen: u64, quantum: u64) {
        let credit = i64::try_from(quantum).unwrap_or(i64::MAX);
        let v = self.deficit.entry(gen).or_insert(0);
        *v = v.saturating_add(credit);
    }

    /// Whether the carried deficit covers `costs` on **every** axis — the
    /// transport is all-or-nothing across a claim's support, since a claim
    /// moves as one rigid body through the fibre. `costs` is `(gen, cost)`
    /// per support axis.
    #[must_use]
    pub fn can_transport(&self, costs: &[(u64, u64)]) -> bool {
        costs.iter().all(|&(gen, cost)| {
            self.deficit.get(&gen).copied().unwrap_or(0) >= i64::try_from(cost).unwrap_or(i64::MAX)
        })
    }

    /// Spend the transport cost along each axis: `V[gen] -= cost`. The
    /// caller must have checked [`Fibre::can_transport`] first; the
    /// remainder on each axis carries into the next round.
    ///
    /// Carrying the per-axis remainder forward is the anti-starvation
    /// guarantee: each active axis is granted a quantum ≥ 1 per round, so
    /// after enough rounds its deficit clears any finite `1 + inflight`.
    /// Even a maximally congested axis transports — later, never *never*.
    pub fn transport(&mut self, costs: &[(u64, u64)]) {
        for &(gen, cost) in costs {
            if let Some(v) = self.deficit.get_mut(&gen) {
                *v = v.saturating_sub(i64::try_from(cost).unwrap_or(i64::MAX));
            }
        }
    }

    /// The carried deficit on one generator axis (0 if untouched).
    #[must_use]
    pub fn deficit_of(&self, gen: u64) -> i64 {
        self.deficit.get(&gen).copied().unwrap_or(0)
    }

    /// The number of generator-axes this fibre currently spans — its
    /// dimension in the flag filtration.
    #[must_use]
    pub fn spanned(&self) -> usize {
        self.deficit.len()
    }

    /// Retract the fibre onto at most `depth` axes (the flag-filtration
    /// bound `Q(F_p) ⊆ F_p`, #57 contained-expansion): keep every
    /// `protected` axis (the head claim's support — it must survive so the
    /// active claim can transport), then fill the remaining budget with the
    /// highest-deficit carried axes, and drop the rest. Ties break on the
    /// smaller generator id, so the retract is **deterministic** — same
    /// fibre, same bound, same survivors on every node.
    ///
    /// Dropping a carried axis forfeits its accumulated credit: a fibre
    /// that returns to a long-idle generator starts it fresh. That is the
    /// price of a closed manifold — bounded volume costs the credit on
    /// rarely-touched axes, never the exactness of any settlement.
    pub fn retract(&mut self, protected: &[u64], depth: usize) {
        if self.deficit.len() <= depth {
            return;
        }
        let guarded: BTreeSet<u64> = protected.iter().copied().collect();
        // Candidates for eviction: everything not protected, ranked by
        // (deficit desc, gen asc) so the survivors are deterministic.
        let mut others: Vec<(u64, i64)> = self
            .deficit
            .iter()
            .filter(|(gen, _)| !guarded.contains(gen))
            .map(|(&gen, &v)| (gen, v))
            .collect();
        others.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        // Keep all guarded axes plus as many top-ranked others as the depth
        // budget allows (never negative: a claim's support is itself capped
        // at `depth`, so `guarded.len() ≤ depth`).
        let keep_others = depth.saturating_sub(guarded.len());
        let survivors: BTreeSet<u64> = guarded
            .iter()
            .copied()
            .chain(others.iter().take(keep_others).map(|&(gen, _)| gen))
            .collect();
        self.deficit.retain(|gen, _| survivors.contains(gen));
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

/// One unit of settlement work awaiting transport across the fibre — the
/// thing [`Transit`] schedules. It carries the two declared geometric
/// facts the scheduler orders it by, and an opaque `payload` it never
/// inspects.
///
/// `support` and `torsion` are **declarations read only to order
/// transit**, never to settle: settlement re-derives everything from the
/// invariant chain. A false declaration (understated support to feign
/// orthogonality, overstated torsion to feign complexity) therefore buys
/// only a better place in *this court's* line — never yield, never a
/// ledger entry — and a signed one that the settled chain contradicts is
/// separately a slashable broken commitment (#56c). The scheduler trusts
/// the cheap declaration for turn-order and lets settlement be the truth.
pub struct Claim<T> {
    /// The economic identity whose fibre this claim rides in — the DRR
    /// key. An empty string is the base lane (an unauthenticated or
    /// unstaked session), served at the escrow floor.
    pub holder: String,
    /// The claim's declared support: the generator axes it transports,
    /// each graded by homological dimension ([`Axis`]), assumed
    /// deduplicated by `gen`. Read for the diagonal connection (by `gen`)
    /// and the graded quantum lift (by `dim`) — never to settle.
    pub support: Box<[Axis]>,
    /// The declared graded torsion `T_k = Tor(H_{k-1}(C))`: `torsion[k]`
    /// lifts the quantum of every support axis of dimension `k` (#58). An
    /// absent grade (index past the end) is `T_k = 0` (a flat grade, ×1
    /// lift). A wholly flat boundary is the empty vector.
    pub torsion: Box<[u64]>,
    /// The settlement work itself, opaque to the scheduler.
    pub payload: T,
}

impl<T> Claim<T> {
    /// The torsion at homological grade `dim`, or 0 if the boundary is
    /// flat there (index at or past the end of the graded vector).
    fn torsion_at(&self, dim: u32) -> u64 {
        self.torsion.get(dim as usize).copied().unwrap_or(0)
    }
}

struct TransitInner<T> {
    /// Per-holder FIFO of pending claims.
    queues: HashMap<String, VecDeque<Claim<T>>>,
    /// Round-robin order over active (non-empty) holders.
    order: VecDeque<String>,
    /// Per-holder carried deficit — the fibre `E_x`. Dropped when a holder
    /// drains (an inactive flow forfeits hoarded credit, standard DRR).
    fibres: HashMap<String, Fibre>,
    /// Holders whose quantum has already been granted for their current
    /// turn (so a burst of serves within one turn is not re-granted).
    open_turn: HashSet<String>,
    /// The live congestion field: a multiset over generators, `inflight[g]`
    /// = how many claims currently **in flight** (taken, not yet completed)
    /// touch generator `g`. The diagonal connection reads it directly —
    /// `cost[g] = 1 + inflight[g]` — so a generator congested by three
    /// concurrent claims dilates transit along that axis three-fold. A
    /// runtime hint that never touches consensus: it is literally
    /// who-else-is-working-on-this-generator-right-now.
    inflight: BTreeMap<u64, u32>,
    /// Total pending claims across all holders.
    total: usize,
}

/// The shared claim-dispatch layer — the seam where the full transport
/// metric attaches (Phase 5, #60). A vector bundle over holders: the base
/// coordinate (escrow) is **projected from the ledger at section-time**
/// (`take*` takes an `escrow_of` projector, never a stored amount, #57);
/// the fibre is the per-holder deficit plus the live in-flight field.
///
/// Dispatch is deterministic given the sequence of `offer`/`take`/
/// `complete` calls and the projected escrow at each `take`: a pure
/// integer state machine (per-axis deficit round-robin with diagonal cost
/// `cost[g] = 1 + inflight[g]` and graded quantum
/// `EscrowWeight(escrow)·(1 + T_k)` per axis). Two courts will diverge in
/// turn-order because their in-flight fields differ by live timing — that
/// is the fibre, and it is exactly what A4 permits, because none of it
/// reaches a settlement. The module's source-scan test holds that line
/// over this type too.
pub struct Transit<T> {
    transport: Transport,
    inner: Mutex<TransitInner<T>>,
    available: Condvar,
}

impl<T> Default for Transit<T> {
    fn default() -> Self {
        Self::new(Transport::tuned())
    }
}

impl<T> Transit<T> {
    /// A transit scheduler with the given transport schedule.
    #[must_use]
    pub fn new(transport: Transport) -> Self {
        Self {
            transport,
            inner: Mutex::new(TransitInner {
                queues: HashMap::new(),
                order: VecDeque::new(),
                fibres: HashMap::new(),
                open_turn: HashSet::new(),
                inflight: BTreeMap::new(),
                total: usize::from(false), // 0, without a bare literal
            }),
            available: Condvar::new(),
        }
    }

    /// The tuned schedule.
    #[must_use]
    pub fn tuned() -> Self {
        Self::new(Transport::tuned())
    }

    /// Enqueue a claim under its holder and wake one worker.
    pub fn offer(&self, claim: Claim<T>) {
        if let Ok(mut inner) = self.inner.lock() {
            let key = claim.holder.clone();
            let empty_before = inner.queues.get(&key).is_none_or(VecDeque::is_empty);
            inner.queues.entry(key.clone()).or_default().push_back(claim);
            inner.total = inner.total.saturating_add(1);
            if empty_before {
                inner.order.push_back(key);
            }
        }
        self.available.notify_one();
    }

    /// The number of claims pending (not counting in-flight).
    #[must_use]
    pub fn pending(&self) -> usize {
        self.inner.lock().map_or(0, |inner| inner.total)
    }

    /// The diagonal transport cost of a claim's support against the current
    /// in-flight field: `(gen, 1 + inflight[gen])` per support axis. Each
    /// axis is priced independently — the connection is diagonal, so a
    /// congested generator dilates only its own coordinate, never a
    /// causally disjoint one.
    fn axis_costs(support: &[Axis], inflight: &BTreeMap<u64, u32>) -> Vec<(u64, u64)> {
        support
            .iter()
            .map(|axis| {
                let congestion = inflight.get(&axis.gen).copied().unwrap_or(0);
                (axis.gen, Transport::cost(congestion))
            })
            .collect()
    }

    /// Advance the DRR state machine by exactly one served claim, or
    /// `None` if nothing is pending. Grants a holder its per-axis quantum
    /// once at the start of its turn (each support generator lifted by the
    /// graded torsion of its own dimension); serves the head claim while
    /// its deficit covers the diagonal cost on **every** support axis;
    /// rotates and carries the per-axis remainder when it cannot.
    /// Guaranteed to terminate while `total > 0`: the in-flight field is
    /// fixed under the lock, so per-axis costs are fixed, and each rotation
    /// re-grants a quantum ≥ 1 on every axis, so a claim's deficits clear
    /// their costs within a bounded number of passes.
    fn dispatch_locked(
        transport: &Transport,
        inner: &mut TransitInner<T>,
        escrow_of: &impl Fn(&str) -> u128,
    ) -> Option<Claim<T>> {
        loop {
            let key = inner.order.front()?.clone();
            // Skip/clear a holder whose queue emptied under it.
            if inner.queues.get(&key).is_none_or(VecDeque::is_empty) {
                inner.order.pop_front();
                inner.queues.remove(&key);
                inner.fibres.remove(&key);
                inner.open_turn.remove(&key);
                continue;
            }
            // Grant the quantum once, at the start of this holder's turn,
            // to each generator axis of the head claim — lifted by the
            // graded torsion of that axis's own dimension (an H_1 lift
            // never perturbs an H_0 axis). The head is the boundary in the
            // fibre right now, not the actor's past.
            if !inner.open_turn.contains(&key) {
                let escrow = escrow_of(&key);
                let grants: Vec<(u64, u64)> = inner
                    .queues
                    .get(&key)
                    .and_then(VecDeque::front)
                    .map(|head| {
                        head.support
                            .iter()
                            .map(|axis| {
                                (axis.gen, transport.quantum(escrow, head.torsion_at(axis.dim)))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let protected: Vec<u64> = grants.iter().map(|&(gen, _)| gen).collect();
                let fibre = inner.fibres.entry(key.clone()).or_default();
                for (gen, quantum) in grants {
                    fibre.grant(gen, quantum);
                }
                // Contained expansion: hold the fibre inside the flag
                // filtration, protecting the head axes so the active claim
                // still transports (#57, Q(F_p) ⊆ F_p).
                fibre.retract(&protected, transport.depth());
                inner.open_turn.insert(key.clone());
            }
            // Diagonal cost: each support axis dilated by its own live
            // congestion, recomputed per serve since the field moves.
            let costs = {
                let head = inner.queues.get(&key).and_then(VecDeque::front)?;
                Self::axis_costs(&head.support, &inner.inflight)
            };
            let afford = inner
                .fibres
                .get(&key)
                .is_some_and(|f| f.can_transport(&costs));
            if !afford {
                // Turn over: carry the per-axis deficit, re-grant next visit.
                inner.open_turn.remove(&key);
                inner.order.pop_front();
                inner.order.push_back(key);
                continue;
            }
            if let Some(fibre) = inner.fibres.get_mut(&key) {
                fibre.transport(&costs);
            }
            let claim = inner.queues.get_mut(&key)?.pop_front()?;
            inner.total = inner.total.saturating_sub(1);
            // The served claim joins the in-flight field: each of its
            // generators now congests that axis until `complete` clears it.
            for axis in &claim.support {
                *inner.inflight.entry(axis.gen).or_insert(0) += 1;
            }
            if inner.queues.get(&key).is_none_or(VecDeque::is_empty) {
                // Drained: drop the holder and forfeit its idle deficit.
                inner.order.pop_front();
                inner.queues.remove(&key);
                inner.fibres.remove(&key);
                inner.open_turn.remove(&key);
            }
            // else: the holder keeps the head and its open turn — the next
            // take serves it again from the remaining deficit (its burst).
            return Some(claim);
        }
    }

    /// Non-blocking dispatch — the next claim by transport order, or
    /// `None` if nothing is pending. `escrow_of` projects the base
    /// coordinate at section-time. Used by tests; workers call
    /// [`Transit::take_blocking`].
    #[must_use]
    pub fn take(&self, escrow_of: impl Fn(&str) -> u128) -> Option<Claim<T>> {
        let mut inner = self.inner.lock().ok()?;
        Self::dispatch_locked(&self.transport, &mut inner, &escrow_of)
    }

    /// Block until a claim is available, then dispatch it by transport
    /// order. `None` only if the lock is poisoned.
    #[must_use]
    pub fn take_blocking(&self, escrow_of: impl Fn(&str) -> u128) -> Option<Claim<T>> {
        let mut inner = self.inner.lock().ok()?;
        loop {
            if let Some(claim) = Self::dispatch_locked(&self.transport, &mut inner, &escrow_of) {
                return Some(claim);
            }
            inner = self.available.wait(inner).ok()?;
        }
    }

    /// Signal that an in-flight claim has settled: decrement each of its
    /// generators from the congestion field, so they no longer dilate the
    /// transit of claims dispatched after it. A worker MUST call this once
    /// per taken claim (settled or failed) — an uncompleted claim would
    /// congest its axes forever. `support` is the taken claim's own support.
    pub fn complete(&self, support: &[Axis]) {
        if let Ok(mut inner) = self.inner.lock() {
            for axis in support {
                if let Some(count) = inner.inflight.get_mut(&axis.gen) {
                    *count -= 1;
                    if *count == 0 {
                        inner.inflight.remove(&axis.gen);
                    }
                }
            }
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

    /// A single axis's throughput: run `rounds` per-axis DRR turns on one
    /// generator, granted `quantum` each round and paying `cost` per serve,
    /// carrying the deficit. Returns items settled — the axis's transit
    /// velocity integrated over the window.
    fn throughput(quantum: u64, cost: u64, rounds: u64) -> u64 {
        let g = 7; // an arbitrary generator axis
        let costs = [(g, cost)];
        let mut fibre = Fibre::new();
        let mut served = 0;
        for _ in 0..rounds {
            fibre.grant(g, quantum);
            while fibre.can_transport(&costs) {
                fibre.transport(&costs);
                served += 1;
            }
        }
        served
    }

    #[test]
    fn the_diagonal_connection_prices_each_axis_by_its_own_congestion() {
        // cost[g] = 1 + inflight[g]. An uncontested (novel) axis is the
        // un-dilated floor; each concurrent claim on the SAME generator
        // adds one. Multiplicity COUNTS — the presence-collapse (where any
        // congestion was a flat "+1") is gone: cost(4) is 5, not 2.
        assert_eq!(Transport::cost(0), 1, "uncontested / novel axis: floor 1");
        assert_eq!(Transport::cost(1), 2, "one concurrent claim: +1");
        assert_eq!(Transport::cost(4), 5, "four concurrent claims: +4, not +1");
    }

    #[test]
    fn fibre_orthogonal_axes_transport_independently() {
        // THE anti-collapse guarantee (#57): a deficit on generator 7 is
        // untouched by spending on generator 12. Under the old scalar
        // fibre a single pool served both, so congestion on one starved the
        // other; the vector fibre gives each axis its own volume.
        let mut f = Fibre::new();
        f.grant(7, 10);
        f.grant(12, 10);
        let hot = [(7u64, 5u64)]; // 7 is congested: cost 5
        let cold = [(12u64, 1u64)]; // 12 is orthogonal: cost 1
        assert!(f.can_transport(&hot));
        f.transport(&hot); // V[7]: 10 → 5
        f.transport(&hot); // V[7]: 5 → 0
        assert!(!f.can_transport(&hot), "generator 7 is drained");
        // Generator 12 is completely unperturbed — still full velocity.
        assert_eq!(f.deficit_of(12), 10, "orthogonal axis uncontaminated");
        assert!(f.can_transport(&cold));
    }

    #[test]
    fn fibre_retract_bounds_the_span_protecting_the_head() {
        let mut f = Fibre::new();
        f.grant(1, 5);
        f.grant(2, 50);
        f.grant(3, 10);
        f.grant(4, 40);
        f.grant(5, 1);
        f.grant(6, 30);
        assert_eq!(f.spanned(), 6);
        // Retract to depth 3, protecting head axis 5 (the LOWEST deficit).
        f.retract(&[5], 3);
        assert_eq!(f.spanned(), 3, "the fibre is bounded to depth 3");
        // The head survives despite lowest deficit; the two remaining slots
        // go to the highest-deficit others (2=50, 4=40). 6,3,1 are evicted
        // and forfeit their credit.
        assert!(f.deficit_of(5) > 0, "protected head axis kept");
        assert_eq!(f.deficit_of(2), 50, "highest-deficit other kept");
        assert_eq!(f.deficit_of(4), 40, "second-highest other kept");
        assert_eq!(f.deficit_of(6), 0, "evicted axis forfeits its credit");
        assert_eq!(f.deficit_of(1), 0);
    }

    #[test]
    fn fibre_retract_is_a_noop_within_depth() {
        let mut f = Fibre::new();
        f.grant(1, 3);
        f.grant(2, 7);
        f.retract(&[1], 5);
        assert_eq!(f.spanned(), 2, "within depth: nothing dropped");
        assert_eq!(f.deficit_of(2), 7);
    }

    #[test]
    fn transit_fibre_stays_within_the_flag_depth() {
        // A holder floods claims each on a DISTINCT generator. Without a
        // bound its fibre would accumulate one axis per generator forever;
        // the flag filtration retracts it to `depth` on every grant.
        let depth = 3;
        let q: Transit<u32> = Transit::new(Transport::tuned().with_depth(depth));
        for i in 0..40u32 {
            q.offer(claim("flood", &[axis(u64::from(i), 0)], &[], i));
        }
        let esc = |_: &str| 0u128;
        let mut max_span = 0;
        let mut served = 0;
        for _ in 0..40 {
            let Some(c) = q.take(esc) else { break };
            served += 1;
            if let Some(f) = q.inner.lock().expect("lock").fibres.get("flood") {
                max_span = max_span.max(f.spanned());
            }
            q.complete(&c.support);
        }
        assert!(max_span <= depth, "fibre never exceeds the flag depth: {max_span}");
        assert!(served > 30, "the holder was actually drained, not stalled");
    }

    #[test]
    fn orthogonal_transit_costs_the_unit_floor() {
        let t = Transport::tuned();
        // Unstaked solver on a flat grade: the bare floor quantum.
        let q = t.quantum(0, 0);
        assert_eq!(q, 1, "unstaked + flat → unit quantum");
        assert_eq!(Transport::cost(0), 1, "uncontested axis is the unit floor");
        assert_eq!(throughput(q, Transport::cost(0), 10), 10);
    }

    #[test]
    fn curvature_dilates_transit_time_relative_to_orthogonal_space() {
        let t = Transport::tuned();
        let q = t.quantum(10_000, 0); // escrow weight caps at 64
        let rounds = 100;
        let orthogonal = throughput(q, Transport::cost(0), rounds);
        let congested = throughput(q, Transport::cost(3), rounds);
        // Velocity is quantum/(1+inflight): 4× the cost → a quarter the
        // throughput in the same wall-time. Pinning cost to a constant
        // (dropping the congestion term) collapses congested == orthogonal.
        assert!(congested < orthogonal, "congestion dilates transit time");
        assert_eq!(orthogonal, q * rounds);
        assert_eq!(congested, q * rounds / 4);
    }

    #[test]
    fn torsion_lifts_the_quantum_and_flat_grades_collapse_to_the_escrow_floor() {
        let t = Transport::tuned();
        let escrow = 10_000; // escrow weight caps at 64
        let flat = t.quantum(escrow, 0);
        let knotted = t.quantum(escrow, 3);
        assert_eq!(flat, 64, "flat grade → bare escrow weight");
        assert_eq!(knotted, 64 * 4, "torsion 3 lifts the quantum ×(1+3)");
        // Directive 1: the lift is a property of the active grade, so the
        // instant a solver reverts to a flat grade its priority collapses
        // back to the escrow base point.
        assert!(knotted > flat);
    }

    #[test]
    fn graded_torsion_reads_by_dimension_flat_past_the_end() {
        // T_k applies strictly to its own grade: an H_1 lift never perturbs
        // an H_0 axis, and an unlisted grade is flat (×1).
        let c = claim("h", &[axis(1, 0), axis(2, 1)], &[0, 3], 0);
        assert_eq!(c.torsion_at(0), 0, "H_0 grade is flat here");
        assert_eq!(c.torsion_at(1), 3, "H_1 grade carries torsion 3");
        assert_eq!(c.torsion_at(2), 0, "past the graded vector → flat");
    }

    #[test]
    fn a_congested_axis_is_dilated_but_never_starved() {
        let t = Transport::tuned();
        let q = t.quantum(0, 0); // the weakest solver: unit quantum
        let cost = Transport::cost(7); // heavy congestion → cost 8
        // Per-axis deficit carries: unit quantum at cost 8 accrues 8 over 8
        // rounds and serves exactly once — slow, never zero.
        assert_eq!(throughput(q, cost, 8), 1);
        // Throughput grows without bound in time: served later, never never.
        assert_eq!(throughput(q, cost, 800), 100);
    }

    fn axis(gen: u64, dim: u32) -> Axis {
        Axis::new(gen, dim)
    }

    fn claim(holder: &str, support: &[Axis], torsion: &[u64], payload: u32) -> Claim<u32> {
        Claim {
            holder: holder.to_owned(),
            support: Box::from(support),
            torsion: Box::from(torsion),
            payload,
        }
    }

    /// Drain up to `n` claims, completing each so no congestion accrues —
    /// isolates escrow/torsion ordering from curvature. Returns holders in
    /// served order.
    fn drain_completing(q: &Transit<u32>, esc: impl Fn(&str) -> u128 + Copy, n: usize) -> Vec<String> {
        let mut served = Vec::new();
        for _ in 0..n {
            let Some(c) = q.take(esc) else { break };
            served.push(c.holder.clone());
            q.complete(&c.support);
        }
        served
    }

    #[test]
    fn transit_serves_a_lone_holder_in_fifo_order() {
        let q: Transit<u32> = Transit::tuned();
        for i in 0..3 {
            q.offer(claim("solo", &[axis(1, 0)], &[], i));
        }
        let esc = |_: &str| 0u128;
        let got: Vec<u32> =
            std::iter::from_fn(|| q.take(esc).map(|c| c.payload)).collect();
        assert_eq!(got, vec![0, 1, 2], "a single holder is drained in order");
    }

    #[test]
    fn transit_escrow_weight_orders_two_holders_without_starving() {
        // Orthogonal axes (rich on gen 1, poor on gen 2), completed each
        // serve so only escrow separates them. rich (escrow 10_000 →
        // weight 64) serves its whole quantum before poor (escrow 0 →
        // weight 1) gets a turn: velocity ∝ escrow weight, poor not starved.
        let q: Transit<u32> = Transit::tuned();
        for i in 0..64 {
            q.offer(claim("rich", &[axis(1, 0)], &[], i));
        }
        for i in 0..64 {
            q.offer(claim("poor", &[axis(2, 0)], &[], 100 + i));
        }
        let esc = |h: &str| if h == "rich" { 10_000u128 } else { 0 };
        let holders = drain_completing(&q, esc, 65);
        assert!(
            holders.iter().take(64).all(|h| h == "rich"),
            "rich serves its full quantum-64 burst first"
        );
        assert_eq!(
            holders.get(64).map(String::as_str),
            Some("poor"),
            "then poor gets its turn — never starved"
        );
    }

    #[test]
    fn transit_graded_torsion_lifts_a_holders_quantum() {
        // Equal escrow (0 → base weight 1); B's head declares torsion 3 at
        // grade 1 (its axis is H_1), lifting that axis's quantum ×4. In the
        // same window B settles four claims to A's flat one.
        let q: Transit<u32> = Transit::tuned();
        for i in 0..4 {
            q.offer(claim("A", &[axis(1, 0)], &[], i));
        }
        for i in 0..4 {
            q.offer(claim("B", &[axis(2, 1)], &[0, 3], 100 + i));
        }
        let esc = |_: &str| 0u128;
        let holders = drain_completing(&q, esc, 5);
        assert_eq!(holders.iter().filter(|h| *h == "A").count(), 1);
        assert_eq!(
            holders.iter().filter(|h| *h == "B").count(),
            4,
            "torsion 3 at grade 1 lifts B's H_1 axis to serve 4× in the window"
        );
    }

    #[test]
    fn transit_congestion_dilates_a_holder_by_multiplicity() {
        // Two single-claim seeds (escrow-rich, so they serve at once) are
        // left IN FLIGHT on generator 1 → inflight[1] = 2, so A's cost on
        // generator 1 is 1+2 = 3, while B on generator 2 costs 1. B settles
        // strictly more, and by ~3× — multiplicity compounds, it does not
        // saturate at "present" (the anti-presence-collapse).
        let q: Transit<u32> = Transit::tuned();
        q.offer(claim("seedX", &[axis(1, 0)], &[], 0));
        q.offer(claim("seedY", &[axis(1, 0)], &[], 1));
        for i in 0..40 {
            q.offer(claim("A", &[axis(1, 0)], &[], 100 + i));
        }
        for i in 0..40 {
            q.offer(claim("B", &[axis(2, 0)], &[], 200 + i));
        }
        let esc = |h: &str| if h.starts_with("seed") { 1_000_000u128 } else { 0 };
        // Take both seeds and hold them in flight (do NOT complete).
        assert_eq!(q.take(esc).expect("seedX").holder, "seedX");
        assert_eq!(q.take(esc).expect("seedY").holder, "seedY");
        assert_eq!(
            q.inner.lock().expect("lock").inflight.get(&1).copied(),
            Some(2),
            "generator 1 congested by both seeds"
        );
        let mut a = 0;
        let mut b = 0;
        for _ in 0..40 {
            let Some(c) = q.take(esc) else { break };
            match c.holder.as_str() {
                "A" => a += 1,
                "B" => b += 1,
                _ => {}
            }
            q.complete(&c.support); // keep the field at exactly the two seeds
        }
        assert!(a > 0, "the congested holder is dilated, never starved");
        assert!(b >= 2 * a, "multiplicity-2 congestion dilates ~3×: {b} vs {a}");
    }

    #[test]
    fn transit_complete_clears_the_congestion_field() {
        let q: Transit<u32> = Transit::tuned();
        q.offer(claim("h", &[axis(1, 0), axis(2, 0), axis(3, 0)], &[], 0));
        let esc = |_: &str| 0u128;
        let c = q.take(esc).expect("one claim");
        {
            // In flight: generators 1,2,3 each congest their own axis once.
            let inner = q.inner.lock().expect("lock");
            assert_eq!(inner.inflight.get(&2).copied(), Some(1), "generator 2 in flight");
            assert_eq!(inner.inflight.len(), 3);
        }
        q.complete(&c.support);
        let inner = q.inner.lock().expect("lock");
        assert!(inner.inflight.is_empty(), "completion clears the field");
    }

    #[test]
    fn transit_dispatch_is_deterministic_within_a_court() {
        // Identical offer/take/escrow sequences produce identical served
        // order: the fibre is non-deterministic ACROSS courts (in-flight
        // fields differ by live timing), but a pure integer state machine
        // WITHIN one. This is what keeps it testable and pinnable.
        let run = || {
            let q: Transit<u32> = Transit::tuned();
            for i in 0..10 {
                q.offer(claim("x", &[axis(1, 0)], &[2], i));
                q.offer(claim("y", &[axis(1, 0)], &[], 100 + i));
            }
            let esc = |h: &str| if h == "x" { 5_000u128 } else { 25 };
            std::iter::from_fn(|| q.take(esc).map(|c| c.payload)).collect::<Vec<_>>()
        };
        assert_eq!(run(), run(), "same inputs → same dispatch, exactly");
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
