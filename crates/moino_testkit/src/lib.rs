//! Development-only acceptance vector loading and controlled clocks; depends on moino_jobs.
#![forbid(unsafe_code)]

mod vectors;
pub use vectors::{Vector, VectorError, load_vectors, parse_vectors};

use moino_jobs::{Clock, Tick};
use std::{cell::Cell, time::Duration};

/// A deterministic, explicitly advanced monotonic clock.
#[derive(Debug, Default)]
pub struct ControlledClock {
    now: Cell<Tick>,
}

impl ControlledClock {
    /// Advances time atomically; overflow leaves the clock unchanged.
    pub fn advance(&self, duration: Duration) -> Option<Tick> {
        let next = self.now.get().checked_add(duration)?;
        self.now.set(next);
        Some(next)
    }
}

impl Clock for ControlledClock {
    fn now(&self) -> Tick {
        self.now.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn time_only_advances_explicitly_and_overflow_is_atomic() {
        let clock = ControlledClock::default();
        assert_eq!(clock.now(), Tick::default());
        assert_eq!(
            clock.advance(Duration::from_millis(10)),
            Some(Tick::new(Duration::from_millis(10)))
        );
        assert!(clock.advance(Duration::MAX).is_none());
        assert_eq!(clock.now().elapsed(), Duration::from_millis(10));
        assert_eq!(clock.advance(Duration::ZERO), Some(clock.now()));
    }
}
