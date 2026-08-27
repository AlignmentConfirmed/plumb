//! THE BOARD — applications, the survey, the price, the docket.
//!
//! A new kernel arrives and says *"I want to join the POWC network."*
//! What happens, in order:
//!
//! ```text
//! Application      applicant, requested shape, claimed work
//!      |
//! survey()         the calculation: where does this estate fit, and
//!      |           if nowhere, what space must be MADE (a new axis)
//! Proposal         the acts that would grant it, the estate class,
//!      |           the price
//! validate()       authenticity: the court's chain plus the proposed
//!      |           acts must be well-formed -- the future chain is
//!      |           checked before it is history
//! [the docket]     queued for the board; landing on the stored chain
//!                  is a deliberate act, like the mint
//! ```
//!
//! ## The estate scale
//!
//! Derived from the geometry, not declared:
//!
//! | grant | geometry |
//! |---|---|
//! | `Run` | a run on an existing line (a partition) |
//! | `Orbit` | a box in existing axes (a planet, a system — by volume) |
//! | `Galaxy` | a new axis opened, then a box in it — space **made** |
//!
//! A **moon** — an estate inside another holder's estate — needs a
//! sublet act the chain does not have yet (the holder consents, the
//! sub-deed nests). Proposed, not landed. And a **re-used system is
//! not re-issued ground**: retired space never reissues, so re-use is
//! only ever subletting inside a live estate.
//!
//! ## The economics
//!
//! `price = the space granted`, and for a galaxy `price = the space
//! CREATED`, which is larger than the box requested — opening a
//! direction multiplies everyone's room, and whoever causes that pays
//! for what it makes, not just for what they take.
//!
//! The applicant pays in **claimed convergence work** (PoWC). The
//! board refuses an application whose claim does not cover the price.
//! Verifying the claim itself — one engine's witness checked by the
//! other — is #43 and is stated as owed, not smuggled: today the
//! board balances *claims* against *space*, and the claim's own court
//! date comes with the witness machinery.

use isthmus::deed::{Act, Flaw, Ledger, Refused};
use isthmus::layout::Tag;

/// A request to join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    /// Who is applying, as they name themselves.
    pub applicant: String,
    /// The estate shape requested: one extent per axis the applicant
    /// wants. One entry asks for a run on the line; more asks for a
    /// box, and more axes than the court has asks for space to be made.
    pub shape: Vec<u128>,
    /// The applicant's standing **position** — per-pole offers of
    /// exact rationals, merging order-invariantly as deltas arrive.
    ///
    /// This replaced `claimed_work: u128`. A scalar in a negotiation
    /// and the boolean gate that held it (`claimed < price`) are both
    /// destroyed under propagation delay: no structure to merge, a
    /// verdict about an instant two parties do not share.
    /// `negotiation.rs` carries the mechanics and the measured
    /// destruction.
    pub position: crate::negotiation::Position,
    /// Where the offers' witnesses can be found when the court
    /// convenes (#43).
    pub witness: String,
}

/// What the survey found the estate to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estate {
    /// A run on an existing line.
    Run {
        /// The run, inclusive.
        low: Tag,
        /// Its end, inclusive.
        high: Tag,
    },
    /// A box in existing axes.
    Orbit {
        /// Per-axis inclusive ranges.
        region: Vec<(Tag, Tag)>,
    },
    /// A slab purchased from a live owner — building on a planet that
    /// is already relatively full. The space is bought, not conjured,
    /// and the owner is on the settlement.
    Parcel {
        /// Who sold it.
        from: String,
        /// The slab conveyed.
        region: Vec<(Tag, Tag)>,
    },
    /// New axes opened — as many as the kernel's mathematics requires —
    /// and the estate placed in the space they made. An 11-D kernel
    /// gets a 10-axis opening; the cosmology is the analogy, the
    /// dimension count is the requirement.
    Galaxy {
        /// The axes opened, in order.
        axes: Vec<String>,
        /// The estate within the new space.
        region: Vec<(Tag, Tag)>,
    },
}

/// How much space the mesh holds and how much this application needs —
/// the accounting the survey answers with, not just a verdict.
/// **Per axis, never a product.** `[2, 8]` and `[4, 4]` are different
/// requests and a volume calls them equal — see [`crate::extent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accounting {
    /// The mesh's space before this application, per axis.
    pub mesh: crate::extent::Extent,
    /// The space the requested shape needs, per axis.
    pub needed: crate::extent::Extent,
}

/// A surveyed application: the acts that would grant it, classified
/// and priced, awaiting the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Who it grants to.
    pub applicant: String,
    /// The acts, in order, that the landing would append.
    pub acts: Vec<Act>,
    /// What the estate is.
    pub estate: Estate,
    /// What it costs, **per axis**: space granted, purchased, or — for
    /// a galaxy — created.
    ///
    /// Was `u128`, the product of the granted region's extents. Two
    /// applications wanting `[2, 8]` and `[4, 4]` priced identically,
    /// reported the same accounting, and were indistinguishable to
    /// every downstream reader. A product across axes is a paper
    /// operation.
    pub price: crate::extent::Extent,
    /// Who is owed what out of the price. Empty for open space and for
    /// creation; for a purchase, the sellers. **This is the downhill
    /// roll's ledger**: every party whose space the grant consumes is
    /// listed with what they are paid for the space they would have
    /// occupied. Today's slab purchase has one seller; a containment
    /// grant (#47) cascades, and this list is already shaped for it.
    pub settlement: Vec<(String, crate::extent::Extent)>,
    /// The mesh-space accounting behind the verdict.
    pub accounting: Accounting,
    /// What the survey demands, per pole. Geometry fixes the ask; the
    /// position negotiates against it. The base pole is `"convergence"`
    /// until the capacity tranche names the economy's own.
    pub ask: crate::negotiation::Ask,
}

/// The base pole the survey prices in, until the capacity tranche
/// lands kernel-named poles.
pub const BASE_POLE: &str = "convergence";

/// Why the board turned an application away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Turned {
    /// No estate of the requested shape exists and none can be made.
    /// Geometry, not negotiation — geometry may refuse.
    NoSpace {
        /// What the issuer said about the attempt.
        refusal: Refused,
    },
    /// A shape with a zero extent, or no extents at all.
    Shapeless,
    /// The proposal's future chain is not well-formed.
    Invalid(Flaw),
}
// `Underfunded` was a variant here: `claimed < price`, one u128 against
// another, met or refused. A scalar in a negotiation, held by a boolean
// gate — both destroyed under propagation delay, and the destruction is
// measured in tests/negotiation.rs. Funding stopped being a refusal:
// the survey attaches an Ask, the applicant holds a Position, and a
// short position gets a Counter on a standing docket entry. Only
// geometry refuses.

/// Multi-axis price quote for a shape (diamond D-L4 product surface).
///
/// Pure: clones the court via [`survey`], returns **per-axis**
/// [`crate::extent::Extent`] price — never a product fold. Callers
/// compare with [`crate::extent::Extent::fits_in`] / `compare`.
///
/// ```text
/// quote([2,8]) vs quote([4,4]) → incomparable prices (not equal)
/// ```
pub fn quote(
    court: &Ledger,
    applicant: impl Into<String>,
    shape: Vec<u128>,
) -> Result<Quote, Turned> {
    let application = Application {
        applicant: applicant.into(),
        shape,
        position: crate::negotiation::Position::default(),
        witness: String::new(),
    };
    let need = crate::extent::Extent::new(application.shape.clone());
    let proposal = survey(court, &application)?;
    Ok(Quote {
        need,
        price: proposal.price,
        estate: proposal.estate,
        ask: proposal.ask,
    })
}

/// Read-only multi-axis estate price (no docket, no landing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    /// Requested shape as multi-axis need (never a product).
    ///
    /// `[2, 8]` and `[4, 4]` share product 16 but are **incomparable**
    /// under [`crate::extent::Extent::compare`].
    pub need: crate::extent::Extent,
    /// Per-axis price of the grant / creation / purchase.
    pub price: crate::extent::Extent,
    /// What geometry the survey selected.
    pub estate: Estate,
    /// Negotiation ask attached to the price (per pole / axis).
    pub ask: crate::negotiation::Ask,
}

/// The calculation: where the estate fits, and if nowhere, what space
/// must be made. Pure — the court is cloned, never touched.
pub fn survey(court: &Ledger, application: &Application) -> Result<Proposal, Turned> {
    if application.shape.is_empty() || application.shape.contains(&0) {
        return Err(Turned::Shapeless);
    }
    // The requested shape IS the per-axis need. It was folded into a
    // product here, which made `[2, 8]` and `[4, 4]` the same request.
    let needed = crate::extent::Extent::new(application.shape.clone());

    // First: does it fit in the space that exists?
    //
    // A shape with MORE axes than the court skips this branch entirely.
    // An earlier draft `resize`d the shape down to the court's axes —
    // silently truncating an 11-D kernel's requirement to whatever the
    // mesh happened to have. A dimensional requirement is a
    // requirement; the survey makes the manifold or refuses, it never
    // quietly flattens.
    let mut trial = court.clone();
    let axes = trial.axes().len();
    if application.shape.len() <= axes {
        let mut shape = application.shape.clone();
        shape.resize(axes, 1);
        match trial.issue_box(&application.applicant, &shape) {
            Ok(deed) => {
                let price = crate::extent::Extent::of(&deed.region);
                let estate = if deed.region.iter().skip(1).all(|(low, _)| *low == 0)
                    && application.shape.len() == 1
                {
                    Estate::Run {
                        low: deed.low(),
                        high: deed.high(),
                    }
                } else {
                    Estate::Orbit {
                        region: deed.region.clone(),
                    }
                };
                return priced(court, application, trial, estate, price, Vec::new(), needed.clone());
            }
            Err(Refused::NoBox { .. } | Refused::NoRun { .. }) => {
                // Fall through: buy, or make.
            }
            Err(other) => return Err(Turned::NoSpace { refusal: other }),
        }

        // Second: the planet is relatively full — BUY the space from an
        // owner. A slab of the requested shape, cut from a live estate,
        // with the owner paid for the space they would have occupied.
        if let Some(proposal) = purchase(court, application, &shape, needed.clone()) {
            return proposal;
        }
    }

    // Third: space must be MADE — as many axes as the kernel's
    // mathematics requires, each wide enough for its extent.
    let extent_before = crate::extent::Extent::of_court(court);
    let mut making = court.clone();
    let mut opened = Vec::new();
    for (dim, extent) in application
        .shape
        .iter()
        .enumerate()
        .skip(making.axes().len())
    {
        let axis = format!("{}-d{}", application.applicant, dim);
        let max = Tag::try_from(*extent).unwrap_or(Tag::MAX);
        making.open_axis(&axis, max);
        opened.push(axis);
    }
    if opened.is_empty() {
        // Same dimensionality, and neither open space nor a seller —
        // the galaxy move proper: one new direction, the estate placed
        // off the zero slice, everyone on the line staying put.
        let axis = format!("{}-d{}", application.applicant, application.shape.len());
        making.open_axis(&axis, 1);
        opened.push(axis);
    }

    let mut shape = application.shape.clone();
    shape.resize(making.axes().len(), 1);
    match making.issue_box(&application.applicant, &shape) {
        Ok(deed) => {
            // Opening directions multiplies everyone's room; the opener
            // pays for what was CREATED, not just what they took.
            // Opening directions multiplies everyone's room; the opener
            // pays for what was CREATED, per axis. Created space on an
            // axis is what the court gained there, and the opener pays
            // at least the extent it took.
            let after = crate::extent::Extent::of_court(&making);
            let taken = crate::extent::Extent::of(&deed.region);
            let created = extent_before
                .taken_from(&after)
                .unwrap_or_else(|| after.clone());
            let price = crate::extent::Extent::new(
                created
                    .components()
                    .iter()
                    .zip(taken.components().iter())
                    .map(|(made, took)| (*made).max(*took))
                    .collect(),
            );
            let estate = Estate::Galaxy {
                axes: opened,
                region: deed.region.clone(),
            };
            priced(court, application, making, estate, price, Vec::new(), needed)
        }
        Err(refusal) => Err(Turned::NoSpace { refusal }),
    }
}

/// The purchase branch: find a live owner whose estate can yield a slab
/// of the requested shape, convey it, and put the owner on the
/// settlement for exactly the space they gave up.
fn purchase(
    court: &Ledger,
    application: &Application,
    shape: &[u128],
    needed: crate::extent::Extent,
) -> Option<Result<Proposal, Turned>> {
    for owner in court.deeds().into_iter().filter(|d| d.live) {
        // A slab of `shape` cut from the high end of one axis, full on
        // the others: find an axis where the owner's extent covers the
        // requested extent and every other axis matches exactly.
        let extents: Vec<u128> = owner
            .region
            .iter()
            .map(|(low, high)| {
                u128::from(*high)
                    .saturating_sub(u128::from(*low))
                    .saturating_add(1)
            })
            .collect();
        if extents.len() != shape.len() {
            continue;
        }
        let cut_axis = (0..shape.len()).find(|axis| {
            let cuttable = matches!(
                (shape.get(*axis), extents.get(*axis)),
                (Some(s), Some(e)) if s < e
            );
            cuttable
                && shape
                    .iter()
                    .zip(extents.iter())
                    .enumerate()
                    .all(|(other, (s, e))| other == *axis || s == e)
        });
        let Some(axis) = cut_axis else { continue };

        let mut slab = owner.region.clone();
        let Some((_, high)) = slab.get(axis).copied() else { continue };
        let Some(wanted) = shape.get(axis) else { continue };
        let Ok(span) = Tag::try_from(wanted.saturating_sub(1)) else { continue };
        let Some(low) = high.checked_sub(span) else { continue };
        let Some(slot) = slab.get_mut(axis) else { continue };
        *slot = (low, high);

        let mut trial = court.clone();
        return match trial.cede(&owner.holder, &application.applicant, &slab) {
            Ok(deed) => {
                let price = crate::extent::Extent::of(&deed.region);
                let settlement = vec![(owner.holder.clone(), price.clone())];
                let estate = Estate::Parcel {
                    from: owner.holder.clone(),
                    region: deed.region,
                };
                Some(priced(court, application, trial, estate, price, settlement, needed))
            }
            Err(_) => continue,
        };
    }
    None
}

/// Price the estate and cut the proposal to exactly the acts the trial
/// added beyond the court.
///
/// **No funding gate here.** The survey answers geometry and attaches
/// the ask; whether the applicant's position clears it is the
/// negotiation's fold, witnessed at [`enact`] and never conducted by a
/// branch in this function.
fn priced(
    court: &Ledger,
    application: &Application,
    trial: Ledger,
    estate: Estate,
    price: crate::extent::Extent,
    settlement: Vec<(String, crate::extent::Extent)>,
    needed: crate::extent::Extent,
) -> Result<Proposal, Turned> {
    // ONE DEMAND PER AXIS, named by the axis it is for.
    //
    // This folded the price into a single number and put it on one
    // pole, which was two collapses in a row: the axes multiplied out,
    // then the product placed on a pole named for nothing. Geometry has
    // axes and the court names them, so a demand for space is a demand
    // per axis and says which.
    //
    // The pole names come from the court, so an edge that opens a
    // direction gets a demand on it without anybody adding a constant.
    // Named from the TRIAL court, not the original: the price is
    // measured in the space that will exist, so a galaxy's new
    // direction is named by the axis that was opened rather than by a
    // synthetic fallback.
    let mut ask = crate::negotiation::Ask::default();
    let axes = trial.axes();
    for (at, component) in price.components().iter().enumerate() {
        let pole = axes
            .get(at)
            .map_or_else(|| format!("{BASE_POLE}-{at}"), |axis| axis.name.clone());
        ask.demand(
            &pole,
            isthmus::ratio::Exact::from(num_bigint::BigInt::from(*component)),
        );
    }
    let already = court.acts().len();
    let acts = trial.acts().get(already..).unwrap_or(&[]).to_vec();
    Ok(Proposal {
        applicant: application.applicant.clone(),
        acts,
        estate,
        price,
        settlement,
        accounting: Accounting {
            mesh: crate::extent::Extent::of_court(court),
            needed,
        },
        ask,
    })
}

/// The fold of the applicant's position against the proposal's ask.
///
/// `Ok(())` is the fixpoint — the balance clears on every pole. The
/// `Err` is a [`Counter`](crate::negotiation::Counter), which is a
/// **standing answer, not a refusal**: the proposal stays on the
/// docket, the applicant's next deltas merge order-invariantly, and
/// this fold is re-taken whenever either side moves.
pub fn clears(
    proposal: &Proposal,
    position: &crate::negotiation::Position,
) -> Result<(), crate::negotiation::Counter> {
    let folded = crate::negotiation::balance(position, &proposal.ask);
    match folded.counter() {
        None => Ok(()),
        Some(counter) => Err(counter),
    }
}

/// Authenticity: the future chain — the court's acts plus the
/// proposal's — must be well-formed **before** it becomes history.
///
/// This is where a tampered proposal dies: an act edited after the
/// survey lands on space the survey never granted, and the replayed
/// induction names it.
pub fn validate(court: &Ledger, proposal: &Proposal) -> Result<(), Turned> {
    let mut acts = court.acts().to_vec();
    acts.extend(proposal.acts.iter().cloned());
    let future = Ledger::replay(court.layout().clone(), acts);
    future.well_formed().map_err(Turned::Invalid)
}

/// The board's enactment, in memory: witness the fixpoint, validate,
/// append.
///
/// The clearing check here **witnesses** a fold that already holds —
/// it conducts nothing, because the negotiation happened in position
/// space where arrival order cannot exist. Landing on the **stored**
/// chain is a deliberate act like the mint — this is the in-memory
/// half every landing runs first.
pub fn enact(
    court: &Ledger,
    proposal: &Proposal,
    position: &crate::negotiation::Position,
) -> Result<Ledger, EnactRefused> {
    if let Err(counter) = clears(proposal, position) {
        return Err(EnactRefused::NotCleared(counter));
    }
    if let Err(turned) = validate(court, proposal) {
        return Err(EnactRefused::Turned(turned));
    }
    let mut acts = court.acts().to_vec();
    acts.extend(proposal.acts.iter().cloned());
    Ok(Ledger::replay(court.layout().clone(), acts))
}

/// Why an enactment did not land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnactRefused {
    /// The balance does not clear — the counter stands, the docket
    /// holds, nothing about the proposal died.
    NotCleared(crate::negotiation::Counter),
    /// The future chain is unlawful — geometry or history, and those
    /// do refuse.
    Turned(Turned),
}