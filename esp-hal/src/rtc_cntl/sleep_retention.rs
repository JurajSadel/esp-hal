//! Sleep retention bookkeeping (ESP32-C6).
//!
//! Powering a domain down during light sleep destroys the register state of
//! every peripheral in it, so retention is opt-in. A peripheral in a
//! power-downable domain is either:
//!
//! - **active and not retained**: it holds a domain-scoped
//!   [`WakeLock`](crate::rtc_cntl::WakeLock), which forbids powering that domain
//!   down, so light sleep degrades to clock-gating; or
//! - **retained**: the caller gave it a backing buffer, so it drops its
//!   wake-lock and its state is saved/restored around the sleep instead.
//!
//! This module only counts, per [`Domain`], how many active-but-unretained
//! peripherals hold a wake-lock, so the sleep path can query [`can_power_down`]
//! before powering a domain off; it knows nothing about *how* state is saved.
//! `TOP`-domain drivers take a lock automatically while active (via
//! `WakeLock::new_top_domain`) and release it when opted into retention.

use core::sync::atomic::{AtomicU32, Ordering};

/// A power domain that can be independently powered down during light sleep.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Domain {
    /// The CPU power domain (`pd_cpu`).
    Cpu = 0,
    /// The digital `TOP` power domain (`pd_top`).
    Top = 1,
}

const DOMAIN_COUNT: usize = 2;

/// Per-domain count of active, unretained peripherals holding the domain awake.
#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU32 = AtomicU32::new(0);
static WAKELOCKS: [AtomicU32; DOMAIN_COUNT] = [ZERO; DOMAIN_COUNT];

/// Record that a peripheral in `domain` is active but not retained. Balanced by
/// [`release`].
pub(crate) fn acquire(domain: Domain) {
    WAKELOCKS[domain as usize].fetch_add(1, Ordering::AcqRel);
}

/// Release a hold previously taken with [`acquire`].
pub(crate) fn release(domain: Domain) {
    WAKELOCKS[domain as usize].fetch_sub(1, Ordering::AcqRel);
}

/// Whether `domain` may be powered down, i.e. nothing holds it awake. On the C6
/// powering `TOP` down also tears down the CPU domain, so it requires *both* to
/// be free of wake-locks.
pub(crate) fn can_power_down(domain: Domain) -> bool {
    let blocked = match domain {
        Domain::Cpu => WAKELOCKS[Domain::Cpu as usize].load(Ordering::Acquire) != 0,
        Domain::Top => {
            WAKELOCKS[Domain::Top as usize].load(Ordering::Acquire) != 0
                || WAKELOCKS[Domain::Cpu as usize].load(Ordering::Acquire) != 0
        }
    };
    !blocked
}
