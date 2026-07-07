//! Power-domain locks for light sleep (ESP32-C6).
//!
//! Powering a domain down during light sleep destroys the register state of
//! every peripheral in it, so it is opt-in. A peripheral in a power-downable
//! domain that is active but not set up for retention holds a
//! [`PowerDomainLock`]: unlike a [`WakeLock`](crate::rtc_cntl::WakeLock) it does
//! not prevent light sleep, it only forbids powering its domain down (light
//! sleep degrades to clock-gating instead) so the peripheral can't lose its
//! state. Opting the peripheral into retention drops the lock and lets regDMA
//! save/restore its state around the power-down instead.

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

/// Per-domain count of active, unretained peripherals holding the domain
/// powered.
#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU32 = AtomicU32::new(0);
static LOCKS: [AtomicU32; DOMAIN_COUNT] = [ZERO; DOMAIN_COUNT];

/// A guard that keeps a power domain powered across light sleep while held.
///
/// It forbids powering `domain` down (light sleep degrades to clock-gating) but,
/// unlike a [`WakeLock`](crate::rtc_cntl::WakeLock), does not prevent sleep
/// itself.
pub(crate) struct PowerDomainLock {
    domain: Domain,
}

impl PowerDomainLock {
    /// Acquire a lock keeping `domain` powered until the guard is dropped.
    pub(crate) fn new(domain: Domain) -> Self {
        LOCKS[domain as usize].fetch_add(1, Ordering::AcqRel);
        Self { domain }
    }
}

impl Drop for PowerDomainLock {
    fn drop(&mut self) {
        LOCKS[self.domain as usize].fetch_sub(1, Ordering::AcqRel);
    }
}

/// Whether `domain` may be powered down, i.e. nothing holds it powered. On the
/// C6 powering `TOP` down also tears down the CPU domain, so it requires both to
/// be free.
pub(crate) fn can_power_down(domain: Domain) -> bool {
    let blocked = match domain {
        Domain::Cpu => LOCKS[Domain::Cpu as usize].load(Ordering::Acquire) != 0,
        Domain::Top => {
            LOCKS[Domain::Top as usize].load(Ordering::Acquire) != 0
                || LOCKS[Domain::Cpu as usize].load(Ordering::Acquire) != 0
        }
    };
    !blocked
}
