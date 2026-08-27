//! Multi-chain linkage in a **polytopal / spherical** environment —
//! horizontal within a chain, vertical between chains.
//!
//! **Spheres and hyperspheres of multi-chain knowledge**, not planar
//! or linear total orders. Each chain's observation horizon is a
//! [`Frontier`]; joining horizons is the envelope of what has been seen.
//! Estate geometry is separately **n-D polytopal** via `Act::Open`
//! (boxes, moons, 5–11D). See datum `decide/linkage-estates.md`
//! master equation **(S)** vs **(E)**.
//!
//! ## What was wrong with a total order of chains
//!
//! [`crate::deed::Ledger`] is a `Vec<Act>`, and a `Vec` is
//! totally ordered. That is correct for one chain: one party appends,
//! and "before" means "earlier in the vector". It stops being correct
//! the moment a second substrate appends to its own chain, because the
//! two share no instant. Asking which of two concurrent acts came first
//! is asking for a number nobody can measure.
//!
//! It is the same defect the negotiation had one layer up — *a scalar
//! is a demand* — sitting in the ledger's spine. A total order across
//! independent parties is a scalar clock, and at any real distance the
//! parties disagree about it while both being right.
//!
//! ## The shape (sphere of chains, polytopal land)
//!
//! ```text
//! horizontal   a chain's own acts, ordered within itself, because
//!              one party appends them
//! vertical     Act::Anchor { chain, height, digest } — "I observed
//!              that chain at that state"  (grants no estate axes)
//! horizon      Frontier = what of each chain this point has seen
//! estate       n-D polytopal boxes via Open — spheres/hyperspheres of
//!              capacity, not a 1-D tape and not a 2-D sheet
//! ```
//!
//! A [`Frontier`] is how much of each chain a point in history has
//! seen: `{datum: 14, strand: 6}`. Two frontiers **join** by taking the
//! larger height per chain — the **hypersphere envelope** of knowledge:
//!
//! ```text
//! (F ⊔ G)(c) = max(F(c), G(c))     // master equation (S)
//! ```
//!
//! Max needs no agreement about order (same as per-pole board merge).
//!
//! ## Conflicts are detected, never prevented
//!
//! Two chains can deed the same ground. Nothing here stops that, and
//! nothing could: preventing an independent party from appending would
//! be a lock nobody holds. What the frontier buys is **classification**
//! — [`Standoff`] separates the two cases that need different remedies:
//!
//! - **Concurrent.** Neither chain had seen the other's act. Both
//!   parties acted correctly on what they knew; the board arbitrates,
//!   and somebody is compensated.
//! - **Ordered.** One chain anchored the other *above* the conflicting
//!   act, so it had already observed the claim and deeded over it
//!   anyway. That is not a collision, it is a party at fault, and it
//!   is refusable.
//!
//! Collapsing those two into "conflict" would either punish an honest
//! party or excuse a dishonest one.
//!
//! ## What this module does not do
//!
//! It computes no digest and names no digest function. This crate is
//! the issuer; **datum is the authority**, and which function hashes a
//! chain is a fact about an edge. [`confirms`] takes the function in.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::deed::{chain, Act, Ledger};
use crate::layout::Tag;

/// How much of each chain a point in history has seen.
///
/// Heights are **counts**, so an absent chain and a height of zero mean
/// the same thing — nothing of it observed. That is deliberate: a
/// frontier that distinguished "never heard of" from "seen zero acts"
/// would make the join depend on which chains happened to be mentioned,
/// and the join must not depend on vocabulary.
///
/// A zero is therefore **never stored**, which is what makes the
/// derived equality the same relation as [`Frontier::compare`] answering
/// `Equal`. The first draft stored them, and `l1` caught it within the
/// hour: `{north: 0}` and `{}` compared equal while the join of the two
/// compared unequal, so the order and the merge disagreed about a pair
/// they both claimed to handle. Two notions of sameness in one type is
/// one too many.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontier {
    heights: BTreeMap<String, u64>,
}

impl Frontier {
    /// Nothing observed anywhere. The identity of [`Frontier::join`].
    pub fn new() -> Self {
        Self::default()
    }

    /// How much of `chain` this frontier has seen. Zero if it has not
    /// been mentioned — see the type's note on why those are one case.
    pub fn height_of(&self, chain: &str) -> u64 {
        self.heights.get(chain).copied().unwrap_or(0)
    }

    /// Every chain this frontier has observed, in name order.
    pub fn chains(&self) -> Vec<&str> {
        self.heights.keys().map(String::as_str).collect()
    }

    /// Observe `chain` at `height`. **Takes the larger.**
    ///
    /// Never the newer, because there is no newer — that would need the
    /// instant this whole module exists because nobody has. Two reports
    /// about the same chain are two observations of a prefix, and the
    /// longer prefix contains the shorter, so max loses nothing.
    pub fn observe(&mut self, chain: &str, height: u64) {
        // A zero is the absence, so recording one records nothing —
        // see the type's note on why there is only one representation.
        if height == 0 {
            return;
        }
        let seen = self.heights.entry(chain.to_owned()).or_insert(0);
        *seen = (*seen).max(height);
    }

    /// The join: per-chain maximum — hypersphere envelope of knowledge.
    ///
    /// Idempotent, commutative, associative. Two parties merge what they
    /// know without agreeing on an order to merge it in.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        let mut out = self.clone();
        for (chain, height) in &other.heights {
            out.observe(chain, *height);
        }
        out
    }

    /// The causal order — **partial**, and the `None` is the point.
    ///
    /// - `Some(Less)` — everything this frontier saw, the other saw
    ///   too, and the other saw more. This one happened before.
    /// - `Some(Equal)` — the same observations.
    /// - `Some(Greater)` — the mirror of `Less`.
    /// - `None` — **concurrent**: each saw something the other did not.
    ///   Neither is before the other, and no amount of further looking
    ///   will settle it, because there is nothing to settle.
    ///
    /// A `bool` here would have to answer *something* for the
    /// concurrent case, and both answers are lies. This is the same
    /// refusal-to-collapse the negotiation makes when it compares
    /// per-pole positions.
    pub fn compare(&self, other: &Self) -> Option<Ordering> {
        let mut names: BTreeSet<&str> = self.heights.keys().map(String::as_str).collect();
        names.extend(other.heights.keys().map(String::as_str));

        let mut ahead = false;
        let mut behind = false;
        for name in names {
            match self.height_of(name).cmp(&other.height_of(name)) {
                Ordering::Greater => ahead = true,
                Ordering::Less => behind = true,
                Ordering::Equal => {}
            }
        }
        match (ahead, behind) {
            (false, false) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Greater),
            (false, true) => Some(Ordering::Less),
            (true, true) => None,
        }
    }

    /// Whether these two frontiers are concurrent — [`Frontier::compare`]
    /// answering `None`, named so the condition reads as itself.
    pub fn concurrent_with(&self, other: &Self) -> bool {
        self.compare(other).is_none()
    }
}

impl Ledger {
    /// What this chain has seen, of itself and of everyone it anchored.
    ///
    /// Its own height comes from [`Ledger::name`], so an **unnamed
    /// chain's frontier omits itself**. That is not a gap to paper
    /// over: a chain nobody can address cannot be anchored, so nothing
    /// can ever be ordered against its acts, so it has no position in
    /// the causal order to report. Downstream works, upstream does not
    /// — stated in the data structure rather than in a comment.
    pub fn frontier(&self) -> Frontier {
        let mut frontier = Frontier::new();
        if let Some(name) = self.name() {
            frontier.observe(name, self.height());
        }
        for act in self.acts() {
            if let Act::Anchor { chain, height, .. } = act {
                frontier.observe(chain, *height);
            }
        }
        frontier
    }

    /// The frontier as it stood after this chain's first `at` acts.
    ///
    /// What a *later* reader needs in order to ask "had this chain seen
    /// that one when it deeded tag 64?" — the question that separates a
    /// concurrent collision from a party at fault.
    pub fn frontier_at(&self, at: usize) -> Frontier {
        self.at(at).frontier()
    }
}

/// Whether an anchor tells the truth about the chain it names.
///
/// The digest function comes in, because this crate has none: it frames
/// records and folds sessions, and picking a hash would be picking a
/// security property on behalf of every integrator. Pass the one the
/// edge agreed on.
///
/// The digest is taken over the **canonical re-encoding** of the cited
/// prefix, not over whatever bytes arrived. Two peers that framed the
/// same acts differently would otherwise disagree about a history they
/// agree on, and the acts are the chain — the bytes are how it travelled.
///
/// `None` when the act is not an anchor, when it names a different
/// chain than the one supplied, or when the cited height exceeds what
/// that chain has: an anchor citing the future is unanswerable rather
/// than false, and saying `false` would accuse a peer of lying about a
/// prefix we simply do not have yet.
pub fn confirms(
    anchor: &Act,
    observed: &Ledger,
    digest: impl Fn(&[u8]) -> Vec<u8>,
) -> Option<bool> {
    let Act::Anchor {
        chain,
        height,
        digest: claimed,
        ..
    } = anchor
    else {
        return None;
    };
    if observed.name() != Some(chain.as_str()) {
        return None;
    }
    let height = usize::try_from(*height).ok()?;
    if height > observed.acts().len() {
        return None;
    }
    Some(&digest(&chain::encode(observed.at(height).acts())) == claimed)
}

/// Two chains deeding the same ground, and **which kind of wrong it is**.
///
/// See the module note: the two arms need different remedies, and a
/// type that merged them would either punish an honest party or excuse
/// a dishonest one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standoff {
    /// The disputed point, in the coordinates both chains name it by.
    pub point: Vec<Tag>,
    /// The chain, holder, and act index of one claim.
    pub here: Claim,
    /// The same, on the other chain.
    pub there: Claim,
    /// Whether either party had seen the other's act.
    pub order: Precedence,
}

/// One side of a [`Standoff`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The chain the claim is on, as it names itself.
    pub chain: String,
    /// Who was deeded the ground.
    pub holder: String,
    /// The act's index in that chain — its height, minus one.
    pub at: usize,
}

/// Who, if anyone, had already seen the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precedence {
    /// **Neither had seen the other.** Both parties acted correctly on
    /// what they knew, and the ground is genuinely double-claimed. The
    /// board arbitrates and somebody is compensated; no one is at
    /// fault, and treating either as at fault would punish a party for
    /// the speed of light.
    Concurrent,
    /// `here` had already anchored `there`'s chain above the
    /// conflicting act, and deeded over it anyway. Not a collision — a
    /// party at fault, and refusable.
    HereSawThere,
    /// The mirror.
    ThereSawHere,
}

/// Every point where two chains have both deeded ground, classified.
///
/// **Detection, not prevention.** Nothing here refuses an append; the
/// chains have already happened. What this produces is the docket.
///
/// Only live deeds are compared: retired ground is spent on both sides
/// and a dispute over it settles nothing. The comparison is over the
/// deeds' *corners* rather than every point in them, because a box
/// overlap is witnessed by a corner and enumerating an 11-D region to
/// find that out is a way to never finish.
pub fn standoffs(a: &Ledger, b: &Ledger) -> Vec<Standoff> {
    let (Some(a_name), Some(b_name)) = (a.name(), b.name()) else {
        // An unnamed chain cannot be anchored, so no act of it can ever
        // be ordered against another chain's. Every conflict would be
        // reported `Concurrent` regardless of what either party
        // actually saw — a classification that is right by accident is
        // not a classification, so this refuses to produce one.
        return Vec::new();
    };

    let mut out = Vec::new();
    for (a_at, a_holder, a_region) in live_issues(a) {
        for (b_at, b_holder, b_region) in live_issues(b) {
            let width = a_region.len().max(b_region.len());
            let here = pad(&a_region, width);
            let there = pad(&b_region, width);
            let Some(point) = meeting(&here, &there) else {
                continue;
            };
            // A holder overlapping its own ground on two chains is one
            // party's estate seen twice, not two parties in dispute.
            if a_holder == b_holder {
                continue;
            }
            out.push(Standoff {
                point,
                here: Claim {
                    chain: a_name.to_owned(),
                    holder: a_holder.clone(),
                    at: a_at,
                },
                there: Claim {
                    chain: b_name.to_owned(),
                    holder: b_holder.clone(),
                    at: b_at,
                },
                order: precedence(a, a_at, b_name, b, b_at, a_name),
            });
        }
    }
    out
}

/// Which party, if either, had already observed the other's act.
///
/// Read off the frontiers **as they stood at each act**, not as they
/// stand now: anchoring a chain today says nothing about what was known
/// when the ground was deeded, and using the present frontier would
/// convict a party for what it learned afterwards.
fn precedence(
    a: &Ledger,
    a_at: usize,
    b_name: &str,
    b: &Ledger,
    b_at: usize,
    a_name: &str,
) -> Precedence {
    // `a_at` is an index; the frontier at the moment of that act is the
    // fold over everything strictly before it.
    let a_knew = a.frontier_at(a_at).height_of(b_name);
    let b_knew = b.frontier_at(b_at).height_of(a_name);

    // Height is a count, so "had seen act at index `i`" is `height > i`.
    match (a_knew > b_at as u64, b_knew > a_at as u64) {
        // Each anchored the other above the other's act. Impossible
        // for two honest parties and not decidable from here either
        // way, so it is reported as what it is observationally: no
        // agreed order. The board sees both anchors and can say more.
        (true, true) | (false, false) => Precedence::Concurrent,
        (true, false) => Precedence::HereSawThere,
        (false, true) => Precedence::ThereSawHere,
    }
}

/// One issue, sited: the act index, the holder, the ground.
type Issued = (usize, String, Vec<(Tag, Tag)>);

/// Live issued ground, with the act index that issued it.
///
/// Folded from the acts rather than from `deeds()`, because a standoff
/// has to cite *where* — a deed knows its region and its holder, and
/// the position is what makes the claim checkable against the chain.
fn live_issues(ledger: &Ledger) -> Vec<Issued> {
    let live: BTreeSet<String> = ledger
        .deeds()
        .into_iter()
        .filter(|d| d.live)
        .map(|d| d.holder)
        .collect();

    let mut out = Vec::new();
    for (at, act) in ledger.acts().iter().enumerate() {
        let (holder, region) = match act {
            Act::Issue { holder, low, high } => (holder, vec![(*low, *high)]),
            Act::IssueBox { holder, region } => (holder, region.clone()),
            _ => continue,
        };
        if live.contains(holder) {
            out.push((at, holder.clone(), region));
        }
    }
    out
}

fn pad(region: &[(Tag, Tag)], to: usize) -> Vec<(Tag, Tag)> {
    let mut out = region.to_vec();
    out.resize(to, (0, 0));
    out
}

/// A point inside both boxes, if they meet — the witness of the overlap.
fn meeting(a: &[(Tag, Tag)], b: &[(Tag, Tag)]) -> Option<Vec<Tag>> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let mut point = Vec::new();
    for ((alow, ahigh), (blow, bhigh)) in a.iter().zip(b.iter()) {
        let low = *alow.max(blow);
        let high = *ahigh.min(bhigh);
        if low > high {
            return None;
        }
        point.push(low);
    }
    Some(point)
}
