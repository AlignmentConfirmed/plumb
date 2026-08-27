//! Multi-axial POW++ rewards for independent nodes.
//!
//! Accepts **boundary** (PoWC flux) and **shape** (PoUW structure) bodies.
//! Verifiers re-derive. Credit is **per-axis / per-orb**, never a scalar
//! difficulty, never a handed token, never a second credit on the same
//! **work_id**.
//!
//! ```text
//! body → WorkBody::parse → verify → work_id → credit axes
//!                              ↘ same structure twice → Replay
//! ```

use std::collections::HashSet;

use assay::credit_event::{ClaimClasses, CreditEvent};
use assay::work::{WorkBody, WorkId};
use assay::Upsilon;

use crate::extent::Extent;

/// Why a reward submission was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardRefused {
    /// Body would not decode as a known work domain.
    Malformed,
    /// Work does not verify (open boundary / empty or bad shape).
    OpenWork,
    /// This useful work was already credited (structure identity).
    Replay {
        /// Content address of the structure.
        work_id: WorkId,
    },
    /// Credit does not cover the asked price on every axis.
    Underfunded {
        /// What the claim earned, per axis.
        credit: Extent,
        /// What was required, per axis.
        price: Extent,
    },
    /// A proof cites a lemma this book has not settled (SQ2). A
    /// citation is a claim about the LEDGER, and the ledger answers.
    UnsettledDependency {
        /// The cited content address nothing here has settled.
        work_id: WorkId,
    },
    /// ARC-class domain is forged curriculum — never mintable (directive I).
    ///
    /// Present when a xylarium on-ramp presents
    /// `strand::pouw::issuance::TaskDomain::Forged`. Shape-domain bodies
    /// (assay domain byte 2) never take this path.
    ForgedDomain,
}

/// A successful credit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    /// Structure identity that was spent (cannot be reused).
    pub work_id: WorkId,
    /// Transport field from the submission (informational only).
    pub transport: u64,
    /// Per-axis / per-orb units credited.
    pub axes: Extent,
    /// In-process flux witness when the body was a closed boundary.
    /// Shape-domain credits leave this `None` — structure admits without
    /// minting [`Upsilon`].
    pub witness: Option<Upsilon>,
}

impl Credit {
    /// Neutral dual-claim event for edge + game sinks (diamond ♦1).
    ///
    /// Schema is tollway-agnostic: same [`CreditEvent`] whether the body
    /// arrived from strand, NS, or another portable Shape path.
    pub fn to_event(&self) -> CreditEvent {
        CreditEvent::with_classes(
            self.work_id.clone(),
            self.transport,
            self.axes.components().to_vec(),
            ClaimClasses::dual(),
        )
    }

    /// Event with explicit claim classes (specialized pools).
    pub fn to_event_with(&self, classes: ClaimClasses) -> CreditEvent {
        CreditEvent::with_classes(
            self.work_id.clone(),
            self.transport,
            self.axes.components().to_vec(),
            classes,
        )
    }
}

/// Append-only act on the reward book (for watchers such as market edge).
///
/// The court is the authority; external crates **observe** this log and
/// never write into the mesh. Replay refusals do **not** append — only
/// successful credits are acts.
///
/// Each successful credit also carries a portable [`CreditEvent`] so
/// edge and game sinks share one leaf type without importing each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardAct {
    /// A work identity was credited for the first time.
    Credited {
        /// Court-local credit record.
        credit: Credit,
        /// Portable dual-claim event (assay leaf).
        event: CreditEvent,
    },
    /// Fund / settlement epoch opened (D-L7).
    EpochOpened {
        /// Epoch id (monotonic on this book).
        epoch: u64,
        /// Human/ops label (corpus rev, bounty window, …).
        label: String,
    },
    /// Epoch closed — no further credits attach to this epoch id.
    EpochClosed {
        /// Closed epoch id.
        epoch: u64,
        /// How many Credited acts fell inside the open window.
        credits_in_epoch: u64,
    },
}

/// Why an epoch op refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochRefused {
    /// An epoch is already open.
    AlreadyOpen { epoch: u64 },
    /// No open epoch to close.
    NoneOpen,
    /// Empty label.
    EmptyLabel,
}

/// Authority-side reward ledger: work_ids seen + cumulative credit.
#[derive(Debug, Default, Clone)]
pub struct RewardBook {
    seen: HashSet<WorkId>,
    /// Cumulative credited units per axis (grows by padding zeros).
    total: Vec<u128>,
    /// Append-only credit acts (index is a stable cursor for watchers).
    acts: Vec<RewardAct>,
    /// Open epoch id, if any.
    open_epoch: Option<u64>,
    /// Next epoch id to assign.
    next_epoch: u64,
    /// Credits counted since the open epoch started.
    credits_in_open: u64,
}

impl RewardBook {
    /// Empty book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Work identities already credited.
    pub fn seen(&self) -> &HashSet<WorkId> {
        &self.seen
    }

    /// Cumulative credit as an [`Extent`].
    pub fn total(&self) -> Extent {
        Extent::new(self.total.clone())
    }

    /// Full act log (append-only).
    pub fn acts(&self) -> &[RewardAct] {
        &self.acts
    }

    /// Acts with index `>= cursor` (for edge/watchers).
    pub fn acts_since(&self, cursor: usize) -> &[RewardAct] {
        self.acts.get(cursor..).unwrap_or(&[])
    }

    /// Next act index (watermark after reading `acts_since`).
    pub fn act_len(&self) -> usize {
        self.acts.len()
    }

    /// Currently open epoch, if any.
    pub fn open_epoch(&self) -> Option<u64> {
        self.open_epoch
    }

    // ── court_store restore hooks (not public product API surface) ───

    /// Append-only act log for durable court restore.
    pub fn acts_mut_for_store(&mut self) -> &mut Vec<RewardAct> {
        &mut self.acts
    }

    /// Next epoch id (store).
    pub fn next_epoch_for_store(&self) -> u64 {
        self.next_epoch
    }

    /// Set next epoch id after restore.
    pub fn set_next_epoch_for_store(&mut self, v: u64) {
        self.next_epoch = v;
    }

    /// Set open epoch after restore.
    pub fn set_open_epoch_for_store(&mut self, v: Option<u64>) {
        self.open_epoch = v;
    }

    /// Set credits-in-open counter after restore.
    pub fn set_credits_in_open_for_store(&mut self, v: u64) {
        self.credits_in_open = v;
    }

    /// Open a fund/settlement epoch (D-L7). Appends [`RewardAct::EpochOpened`].
    pub fn open_epoch_named(&mut self, label: impl Into<String>) -> Result<u64, EpochRefused> {
        if let Some(e) = self.open_epoch {
            return Err(EpochRefused::AlreadyOpen { epoch: e });
        }
        let label = label.into();
        if label.is_empty() {
            return Err(EpochRefused::EmptyLabel);
        }
        let epoch = self.next_epoch;
        self.next_epoch = self.next_epoch.saturating_add(1);
        self.open_epoch = Some(epoch);
        self.credits_in_open = 0;
        self.acts.push(RewardAct::EpochOpened { epoch, label });
        Ok(epoch)
    }

    /// Close the open epoch (D-L7). Appends [`RewardAct::EpochClosed`].
    pub fn close_epoch(&mut self) -> Result<u64, EpochRefused> {
        let epoch = self.open_epoch.take().ok_or(EpochRefused::NoneOpen)?;
        let credits_in_epoch = self.credits_in_open;
        self.credits_in_open = 0;
        self.acts.push(RewardAct::EpochClosed {
            epoch,
            credits_in_epoch,
        });
        Ok(epoch)
    }

    /// Restore a durable epoch-open marker (court_store decode).
    pub fn restore_epoch_opened(&mut self, epoch: u64, label: String) {
        self.acts.push(RewardAct::EpochOpened { epoch, label });
        self.open_epoch = Some(epoch);
        self.credits_in_open = 0;
        if self.next_epoch <= epoch {
            self.next_epoch = epoch.saturating_add(1);
        }
    }

    /// Restore a durable epoch-close marker (court_store decode).
    pub fn restore_epoch_closed(&mut self, epoch: u64, credits_in_epoch: u64) {
        self.acts.push(RewardAct::EpochClosed {
            epoch,
            credits_in_epoch,
        });
        if self.open_epoch == Some(epoch) {
            self.open_epoch = None;
            self.credits_in_open = 0;
        }
        if self.next_epoch <= epoch {
            self.next_epoch = epoch.saturating_add(1);
        }
    }

    /// Credit a portable work body after re-derivation.
    ///
    /// **P3:** always re-parses and re-verifies the body (Shape admit /
    /// boundary close). No trusted token path. For ARC-class domains,
    /// call [`Self::admit_arc_domain`] first or use
    /// [`Self::credit_claim_with_domain`].
    pub fn credit_claim(&mut self, body: &[u8]) -> Result<Credit, RewardRefused> {
        self.credit_claim_inner(body)
    }


    fn credit_claim_inner(&mut self, body: &[u8]) -> Result<Credit, RewardRefused> {
        let work = WorkBody::parse(body).map_err(|_| RewardRefused::Malformed)?;
        // SQ2 — citations answer to the book, and to nothing else. A
        // settled lemma is trusted as closed (it could not have entered
        // `seen` otherwise) and costs no re-derivation: the replay set
        // IS the memoization cache, here spent as one.
        if let WorkBody::Proof(p) = &work {
            for dep in &p.deps {
                let cited = WorkId::from_bytes(dep.clone());
                if !self.seen.contains(&cited) {
                    return Err(RewardRefused::UnsettledDependency { work_id: cited });
                }
            }
        }
        if !work.verifies() {
            return Err(RewardRefused::OpenWork);
        }
        let work_id = work.work_id();
        if self.seen.contains(&work_id) {
            return Err(RewardRefused::Replay { work_id });
        }
        let axes = work.credit_axes();
        if axes.is_empty() {
            return Err(RewardRefused::OpenWork);
        }
        self.seen.insert(work_id.clone());
        while self.total.len() < axes.len() {
            self.total.push(0);
        }
        for (i, unit) in axes.iter().enumerate() {
            if let Some(slot) = self.total.get_mut(i) {
                *slot = slot.saturating_add(*unit);
            }
        }
        let witness = match &work {
            WorkBody::Boundary(c) => c.verify(),
            WorkBody::Shape(_) | WorkBody::Declared(_) | WorkBody::Proof(_) => None,
        };
        let credit = Credit {
            work_id,
            transport: work.transport(),
            axes: Extent::new(axes),
            witness,
        };
        let event = credit.to_event();
        self.acts.push(RewardAct::Credited {
            credit: credit.clone(),
            event,
        });
        if self.open_epoch.is_some() {
            self.credits_in_open = self.credits_in_open.saturating_add(1);
        }
        Ok(credit)
    }

    /// Credit and return the portable [`CreditEvent`] (diamond dual-sink).
    pub fn credit_claim_event(&mut self, body: &[u8]) -> Result<CreditEvent, RewardRefused> {
        let credit = self.credit_claim_inner(body)?;
        Ok(credit.to_event())
    }

    /// Admit a portable [`CreditEvent`] that a peer court already settled.
    ///
    /// **Diamond ♦2 multi-node handoff.** Does **not** re-parse a body —
    /// the event is the court projection after verification elsewhere.
    /// Same `work_id` still refuses ([`RewardRefused::Replay`]). Υ is
    /// never restored (non-wire; durable path carries axes + identity).
    ///
    /// Empty axes refuse as open work. Claim classes are preserved on
    /// the stored act for dual sinks.
    pub fn admit_event(&mut self, event: CreditEvent) -> Result<Credit, RewardRefused> {
        if event.axes.is_empty() {
            return Err(RewardRefused::OpenWork);
        }
        if self.seen.contains(&event.work_id) {
            return Err(RewardRefused::Replay {
                work_id: event.work_id,
            });
        }
        self.seen.insert(event.work_id.clone());
        while self.total.len() < event.axes.len() {
            self.total.push(0);
        }
        for (i, unit) in event.axes.iter().enumerate() {
            if let Some(slot) = self.total.get_mut(i) {
                *slot = slot.saturating_add(*unit);
            }
        }
        let credit = Credit {
            work_id: event.work_id.clone(),
            transport: event.transport,
            axes: Extent::new(event.axes.clone()),
            witness: None,
        };
        self.acts.push(RewardAct::Credited {
            credit: credit.clone(),
            event,
        });
        if self.open_epoch.is_some() {
            self.credits_in_open = self.credits_in_open.saturating_add(1);
        }
        Ok(credit)
    }

    /// Merge another book's acts into this one (gossip / multi-node).
    ///
    /// Returns how many new **credit** acts were admitted. Already-seen
    /// `work_id`s are skipped. Epoch open/close acts are replayed as
    /// markers only when not already present at the same epoch id.
    pub fn merge_acts_from(&mut self, other: &RewardBook) -> usize {
        let mut added = 0usize;
        for act in &other.acts {
            match act {
                RewardAct::Credited { event, .. } => {
                    if self.admit_event(event.clone()).is_ok() {
                        added = added.saturating_add(1);
                    }
                }
                RewardAct::EpochOpened { epoch, label } => {
                    if !self.acts.iter().any(|a| {
                        matches!(a, RewardAct::EpochOpened { epoch: e, .. } if e == epoch)
                    }) {
                        self.acts.push(RewardAct::EpochOpened {
                            epoch: *epoch,
                            label: label.clone(),
                        });
                        if self.next_epoch <= *epoch {
                            self.next_epoch = epoch.saturating_add(1);
                        }
                    }
                }
                RewardAct::EpochClosed {
                    epoch,
                    credits_in_epoch,
                } => {
                    if !self.acts.iter().any(|a| {
                        matches!(a, RewardAct::EpochClosed { epoch: e, .. } if e == epoch)
                    }) {
                        self.acts.push(RewardAct::EpochClosed {
                            epoch: *epoch,
                            credits_in_epoch: *credits_in_epoch,
                        });
                        if self.open_epoch == Some(*epoch) {
                            self.open_epoch = None;
                            self.credits_in_open = 0;
                        }
                        if self.next_epoch <= *epoch {
                            self.next_epoch = epoch.saturating_add(1);
                        }
                    }
                }
            }
        }
        added
    }

    /// Whether cumulative credit covers `price` on every axis.
    pub fn covers(&self, price: &Extent) -> bool {
        price.fits_in(&self.total())
    }


    /// Admit settlement only when credit covers price on every axis.
    pub fn settle_against(&self, price: &Extent) -> Result<(), RewardRefused> {
        let credit = self.total();
        if price.fits_in(&credit) {
            Ok(())
        } else {
            Err(RewardRefused::Underfunded {
                credit,
                price: price.clone(),
            })
        }
    }

    /// Stack a multi-axial allocation (merge principal / carry legs).
    ///
    /// Does **not** consume a work_id — used after merge admit for
    /// economic routing of bulk vs residual. Zero-only extents are no-ops
    /// on length; arity may grow by padding zeros then adding.
    pub fn add_extent(&mut self, ext: &Extent) {
        let comps = ext.components();
        while self.total.len() < comps.len() {
            self.total.push(0);
        }
        for (i, unit) in comps.iter().enumerate() {
            if let Some(slot) = self.total.get_mut(i) {
                *slot = slot.saturating_add(*unit);
            }
        }
    }
}

/// Build a closed 2-axis box claim (helper for tests and examples).
pub fn closed_box_claim(nonce: u64, flux: i64) -> assay::Claim {
    use assay::{whole, Boundary, Facet, Orientation};
    let mut b = Boundary::new(2);
    let f = whole(flux);
    let _ = b.face(Facet::new(0, Orientation::Low, f.clone()));
    let _ = b.face(Facet::new(0, Orientation::High, f.clone()));
    let _ = b.face(Facet::new(1, Orientation::Low, f.clone()));
    let _ = b.face(Facet::new(1, Orientation::High, f));
    assay::Claim::new(nonce, b)
}

/// Re-export shape fixture.
pub use assay::shape::triangle_claim;
