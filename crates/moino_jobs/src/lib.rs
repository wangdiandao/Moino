//! L0 job identity, cancellation and monotonic clock contracts; no internal dependencies.
#![forbid(unsafe_code)]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// A monotonic timestamp relative to an executor-owned origin.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Tick(Duration);

impl Tick {
    /// Creates a timestamp without reading a system clock.
    pub const fn new(elapsed: Duration) -> Self {
        Self(elapsed)
    }
    /// Returns elapsed time since the clock origin.
    pub const fn elapsed(self) -> Duration {
        self.0
    }
    /// Returns a later timestamp, or `None` on overflow.
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration).map(Self)
    }
}

/// Supplies executor time; L0 code never reads the system clock directly.
pub trait Clock {
    /// Returns the current monotonic timestamp.
    fn now(&self) -> Tick;
}

/// Identifies one owner's generation and content revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobIdentity {
    owner: JobOwnerId,
    epoch: JobEpoch,
    revision: JobRevision,
}

/// Opaque identity of the owner of a background result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobOwnerId(u128);

impl JobOwnerId {
    /// Wraps an owner identity at the scheduling boundary.
    pub const fn new(value: u128) -> Self {
        Self(value)
    }
}

/// Generation captured when scheduling a background result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobEpoch(u64);

impl JobEpoch {
    /// Wraps a generation at the scheduling boundary.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Content revision captured when scheduling a background result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobRevision(u64);

impl JobRevision {
    /// Wraps a revision at the scheduling boundary.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

impl JobIdentity {
    /// Creates a complete submission identity.
    pub const fn new(owner: JobOwnerId, epoch: JobEpoch, revision: JobRevision) -> Self {
        Self {
            owner,
            epoch,
            revision,
        }
    }
}

/// Shared, idempotent cooperative cancellation.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Cancels this token and all its clones.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Returns whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Validates ownership, cancellation and deadline before accepting a result.
#[derive(Clone, Debug)]
pub struct SubmissionGuard {
    identity: JobIdentity,
    cancellation: CancellationToken,
    deadline: Tick,
}

impl SubmissionGuard {
    /// Creates a guard with an executor-supplied deadline.
    pub fn new(identity: JobIdentity, cancellation: CancellationToken, deadline: Tick) -> Self {
        Self {
            identity,
            cancellation,
            deadline,
        }
    }
    /// Returns whether the result still belongs to the current state and is timely.
    pub fn is_current(&self, current: JobIdentity, clock: &impl Clock) -> bool {
        self.identity == current && !self.cancellation.is_cancelled() && clock.now() < self.deadline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed(Tick);
    impl Clock for Fixed {
        fn now(&self) -> Tick {
            self.0
        }
    }
    fn identity(owner: u128, epoch: u64, revision: u64) -> JobIdentity {
        JobIdentity::new(
            JobOwnerId::new(owner),
            JobEpoch::new(epoch),
            JobRevision::new(revision),
        )
    }

    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn submission_rejects_deadline_cancellation_and_stale_identity() {
        let current = identity(1, 2, 3);
        let token = CancellationToken::default();
        let guard =
            SubmissionGuard::new(current, token.clone(), Tick::new(Duration::from_millis(10)));
        let before = Fixed(Tick::new(Duration::from_millis(9)));
        assert!(guard.is_current(current, &before));
        assert!(!guard.is_current(identity(2, 2, 3), &before));
        assert!(!guard.is_current(identity(1, 3, 3), &before));
        assert!(!guard.is_current(identity(1, 2, 4), &before));
        assert!(!guard.is_current(current, &Fixed(Tick::new(Duration::from_millis(10)))));
        token.cancel();
        token.cancel();
        assert!(!guard.is_current(current, &before));
    }

    #[test]
    // Spec: docs/acceptance/m0.md#自动检查
    fn ticks_are_checked_and_preserve_units() {
        assert_eq!(
            Tick::default()
                .checked_add(Duration::from_millis(1))
                .expect("small duration")
                .elapsed(),
            Duration::from_millis(1)
        );
        assert!(
            Tick::new(Duration::MAX)
                .checked_add(Duration::from_nanos(1))
                .is_none()
        );
    }
}
