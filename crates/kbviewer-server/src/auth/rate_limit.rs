//! Login rate limiting.
//!
//! Checked *before* password hashing: Argon2 is intentionally expensive, so an unlimited
//! login endpoint is both a credential-guessing oracle and a cheap way to exhaust the
//! server's memory. Limits are tracked per email and per client address, and the stricter
//! of the two wins — one attacker rotating source addresses is still bounded by the email
//! counter, and one address spraying many emails is bounded by the address counter.

use dashmap::DashMap;
use std::time::{Duration, Instant};

const MAX_ATTEMPTS: u32 = 5;
const WINDOW: Duration = Duration::from_secs(15 * 60);
const BASE_LOCKOUT: Duration = Duration::from_secs(60);
const MAX_LOCKOUT: Duration = Duration::from_secs(60 * 60);
/// How many times a repeat offender's lockout may double. Capped so the shift cannot
/// overflow; `MAX_LOCKOUT` is what actually bounds the wait.
const MAX_LOCKOUT_DOUBLINGS: u32 = 5;

#[derive(Debug, Clone)]
struct Attempts {
    count: u32,
    first_seen: Instant,
    locked_until: Option<Instant>,
    lockouts: u32,
}

pub struct RateLimiter {
    entries: DashMap<String, Attempts>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// Locked out; the value is how long the caller must wait.
    Deny(Duration),
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Check every key without recording anything. Called before doing expensive work.
    pub fn check(&self, keys: &[String]) -> Decision {
        let now = Instant::now();
        let longest = keys
            .iter()
            .filter_map(|key| self.remaining_lockout(key, now))
            .max()
            .unwrap_or(Duration::ZERO);

        if longest > Duration::ZERO {
            Decision::Deny(longest)
        } else {
            Decision::Allow
        }
    }

    /// How much longer this key stays locked out, or `None` if it may proceed.
    fn remaining_lockout(&self, key: &str, now: Instant) -> Option<Duration> {
        let entry = self.entries.get(key)?;
        let until = entry.locked_until?;
        (until > now).then(|| until - now)
    }

    /// Record a failure. Each additional lockout doubles the wait, so sustained guessing
    /// becomes impractical while a user who mistypes twice is barely inconvenienced.
    pub fn record_failure(&self, keys: &[String]) {
        let now = Instant::now();
        for key in keys {
            let mut entry = self.entries.entry(key.clone()).or_insert(Attempts {
                count: 0,
                first_seen: now,
                locked_until: None,
                lockouts: 0,
            });

            if now.duration_since(entry.first_seen) > WINDOW {
                entry.count = 0;
                entry.first_seen = now;
            }
            entry.count += 1;

            if entry.count >= MAX_ATTEMPTS {
                let shift = entry.lockouts.min(MAX_LOCKOUT_DOUBLINGS);
                let wait = (BASE_LOCKOUT * 2u32.saturating_pow(shift)).min(MAX_LOCKOUT);
                entry.locked_until = Some(now + wait);
                entry.lockouts += 1;
                entry.count = 0;
                entry.first_seen = now;
            }
        }
    }

    /// A successful login clears the counters for that identity.
    pub fn record_success(&self, keys: &[String]) {
        for key in keys {
            self.entries.remove(key);
        }
    }

    /// Drop entries that have aged out, so the map cannot grow without bound.
    pub fn prune(&self) {
        let now = Instant::now();
        self.entries.retain(|_, entry| {
            entry.locked_until.map(|until| until > now).unwrap_or(false)
                || now.duration_since(entry.first_seen) <= WINDOW
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(label: &str) -> Vec<String> {
        vec![format!("email:{label}"), format!("addr:{label}")]
    }

    #[test]
    fn allows_attempts_below_the_threshold() {
        let limiter = RateLimiter::new();
        let k = keys("a");
        for _ in 0..(MAX_ATTEMPTS - 1) {
            assert_eq!(limiter.check(&k), Decision::Allow);
            limiter.record_failure(&k);
        }
        assert_eq!(
            limiter.check(&k),
            Decision::Allow,
            "must not lock out one attempt early"
        );
    }

    #[test]
    fn locks_out_on_the_threshold_attempt() {
        let limiter = RateLimiter::new();
        let k = keys("b");
        for _ in 0..MAX_ATTEMPTS {
            limiter.record_failure(&k);
        }
        assert!(matches!(limiter.check(&k), Decision::Deny(_)));
    }

    #[test]
    fn a_success_clears_the_counters() {
        let limiter = RateLimiter::new();
        let k = keys("c");
        for _ in 0..(MAX_ATTEMPTS - 1) {
            limiter.record_failure(&k);
        }
        limiter.record_success(&k);
        for _ in 0..(MAX_ATTEMPTS - 1) {
            limiter.record_failure(&k);
        }
        assert_eq!(
            limiter.check(&k),
            Decision::Allow,
            "counters must have reset"
        );
    }

    #[test]
    fn lockouts_lengthen_with_repetition() {
        let limiter = RateLimiter::new();
        let k = keys("d");
        for _ in 0..MAX_ATTEMPTS {
            limiter.record_failure(&k);
        }
        let Decision::Deny(first) = limiter.check(&k) else {
            panic!("expected a lockout");
        };
        for _ in 0..MAX_ATTEMPTS {
            limiter.record_failure(&k);
        }
        let Decision::Deny(second) = limiter.check(&k) else {
            panic!("expected a second lockout");
        };
        assert!(
            second > first,
            "repeat lockouts must grow: {first:?} then {second:?}"
        );
    }

    #[test]
    fn locking_one_identity_does_not_lock_another() {
        let limiter = RateLimiter::new();
        for _ in 0..MAX_ATTEMPTS {
            limiter.record_failure(&keys("victim"));
        }
        assert_eq!(limiter.check(&keys("bystander")), Decision::Allow);
    }

    #[test]
    fn a_shared_address_still_limits_across_different_emails() {
        let limiter = RateLimiter::new();
        let address = "addr:1.2.3.4".to_string();
        for i in 0..MAX_ATTEMPTS {
            limiter.record_failure(&[format!("email:user{i}@x"), address.clone()]);
        }
        assert!(
            matches!(limiter.check(&[address]), Decision::Deny(_)),
            "spraying many emails from one address must still be caught"
        );
    }
}
