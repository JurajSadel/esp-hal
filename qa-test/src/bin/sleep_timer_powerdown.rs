//! Timer-woken light sleep with the CPU/TOP power domains powered down.
//!
//! Three sleep policies for the same timer wakeup:
//!
//! - **clock-gated** ([`RtcSleepConfig::default`]): plain light sleep, the CPU is only clock-gated
//!   and no register state is lost.
//! - **cpu-powerdown** ([`with_cpu_power_down`]): the CPU domain is powered off; its state is
//!   saved/restored in software (see `cpu_retention`).
//! - **top-powerdown** ([`with_top_power_down`]): the whole digital `TOP` domain is powered off;
//!   regDMA backs its peripherals up to RAM and the PMU restores them on wakeup. On the C6 this
//!   also powers the CPU down.
//!
//! Each round prints the time actually slept (from the always-on RTC) and
//! [`cpu_power_down_wake_count`], which is bumped from the ROM wake stub and so
//! advances only when the CPU domain really lost power.
//!
//! The example also proves the design end to end:
//!
//! - **Negative control**: one un-retained `TOP` register (I2C0 `SCL_LOW_PERIOD`) survives a
//!   clock-gated sleep (what a build without retention does) but is wiped by `top-powerdown` -
//!   direct evidence the domain lost power.
//! - **Safety net**: while UART1 is active but not retained it holds a `TOP` wake-lock, so a
//!   `top-powerdown` request degrades to clock-gating instead of destroying its state (the counter
//!   stays put).
//! - **Opt-in retention**: UART1, I2C0 and SPI2 are opted in via [`Uart::with_retention_memory`] /
//!   [`I2c::with_retention_memory`] / [`Spi::with_retention_memory`], then a config register of
//!   each is confirmed to survive `top-powerdown`.
//!
//! GPIO5 is driven high while awake and low while asleep, bracketing each sleep
//! window for a current meter / logic analyzer (e.g. Nordic PPK2).

//% CHIP_FILTER: esp32c6

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c, I2cRetentionMemory},
    main,
    rtc_cntl::{
        Rtc,
        cpu_retention::{CpuRetentionMemory, SystemRetentionMemory, cpu_power_down_wake_count},
        sleep::{RtcSleepConfig, TimerWakeupSource},
    },
    spi::master::{Config as SpiConfig, Spi, SpiRetentionMemory},
    time::Duration,
    uart::{Config as UartConfig, Uart, UartRetentionMemory},
};
use esp_println::println;

esp_bootloader_esp_idf::esp_app_desc!();

/// Allocate `$val` in a `static` and hand out a `&'static mut` to it.
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        STATIC_CELL.uninit().write($val)
    }};
}

/// Sleep duration per round, in milliseconds. Long enough to give a current
/// meter a wide, flat sleep plateau to average over.
const EVENT_MS: u64 = 1000;
/// Awake window between sleeps, in milliseconds, so the sleep plateaus are
/// clearly separated on a current/logic trace.
const AWAKE_MS: u32 = 200;
/// Rounds per mode.
const ROUNDS: u32 = 3;

/// Sleep once for `EVENT_MS`, then report the wall-clock time spent asleep (from
/// the always-on RTC) and the CPU power-down wake counter. `marker` is driven
/// low for the sleep window (high while awake) so a PPK2/logic channel can
/// bracket each sleep.
fn sleep_round(
    rtc: &mut Rtc<'_>,
    marker: &mut Output<'_>,
    delay: &Delay,
    config: &RtcSleepConfig,
    label: &str,
    round: u32,
) {
    let timer = TimerWakeupSource::new(Duration::from_millis(EVENT_MS));

    let rtc_before = rtc.time_since_power_up().as_micros();
    marker.set_low();
    rtc.sleep(config, &[&timer]);
    marker.set_high();
    let slept_ms = (rtc.time_since_power_up().as_micros() - rtc_before) / 1000;

    println!(
        "{} #{}: slept ~{} ms (RTC), CPU power-downs = {}",
        label,
        round,
        slept_ms,
        cpu_power_down_wake_count()
    );

    delay.delay_millis(AWAKE_MS);
}

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut rtc = Rtc::new(peripherals.LPWR);
    let delay = Delay::new();

    // Awake = high, asleep = low. The IO domain stays powered through light
    // sleep, so the pin holds its level and a meter/scope sees a clean window.
    let mut marker = Output::new(peripherals.GPIO5, Level::High, OutputConfig::default());

    // Each power-down config gets its own caller-owned retention buffer;
    // clock-gating needs none. `RtcSleepConfig` is `Copy`, so `top_pd` is reused.
    let clock_gated = RtcSleepConfig::default();
    let cpu_pd = RtcSleepConfig::default()
        .with_cpu_power_down(mk_static!(CpuRetentionMemory, CpuRetentionMemory::new()));
    let top_pd = RtcSleepConfig::default().with_top_power_down(
        mk_static!(CpuRetentionMemory, CpuRetentionMemory::new()),
        mk_static!(SystemRetentionMemory, SystemRetentionMemory::new()),
    );

    println!("up and running!");

    // Negative control: an idle, un-retained I2C0 (a TOP peripheral, holding no
    // wake-lock) keeps its SCL_LOW_PERIOD register through a clock-gated sleep
    // but loses it to a top-powerdown - proof the domain really lost power.
    const I2C0_SCL_LOW_PERIOD: *const u32 = 0x6000_4000 as *const u32;
    let mut i2c0 = I2c::new(peripherals.I2C0, I2cConfig::default()).unwrap();
    // SAFETY: I2C0 is configured and clocked (driver alive), register readable.
    let scl_configured = unsafe { I2C0_SCL_LOW_PERIOD.read_volatile() };

    marker.set_low();
    rtc.sleep(
        &clock_gated,
        &[&TimerWakeupSource::new(Duration::from_millis(EVENT_MS))],
    );
    marker.set_high();
    let scl_after_clock_gate = unsafe { I2C0_SCL_LOW_PERIOD.read_volatile() };

    let downs_before_nc = cpu_power_down_wake_count();
    marker.set_low();
    rtc.sleep(
        &top_pd,
        &[&TimerWakeupSource::new(Duration::from_millis(EVENT_MS))],
    );
    marker.set_high();
    let scl_after_top_pd = unsafe { I2C0_SCL_LOW_PERIOD.read_volatile() };
    let downs_after_nc = cpu_power_down_wake_count();

    println!(
        "negative-control I2C0 SCL_LOW_PERIOD: configured = {:#x}",
        scl_configured
    );
    println!(
        "  after clock-gated (== no-retention/main behavior): {:#x} -> {}",
        scl_after_clock_gate,
        if scl_after_clock_gate == scl_configured {
            "SURVIVED (TOP stayed powered)"
        } else {
            "CHANGED?!"
        }
    );
    println!(
        "  after top-powerdown (no retention): {:#x} -> {} (CPU power-downs {} -> {})",
        scl_after_top_pd,
        if scl_after_top_pd != scl_configured && scl_after_top_pd == 0 {
            "WIPED (TOP lost power) -> POWER REALLY REMOVED"
        } else {
            "STILL SET?!"
        },
        downs_before_nc,
        downs_after_nc
    );

    // The power-down above reset I2C0; re-apply its config for the retention
    // proof below, which needs a valid, non-zero state.
    i2c0.apply_config(&I2cConfig::default()).unwrap();

    // UART1 (a TOP peripheral) is live but not yet retained, so its driver holds
    // a TOP wake-lock and a top-powerdown request must degrade to clock-gating
    // rather than destroy its config: the counter must not advance.
    const UART1_CLKDIV: *const u32 = 0x6000_1014 as *const u32;
    let uart1 = Uart::new(peripherals.UART1, UartConfig::default()).unwrap();

    let downs_before_block = cpu_power_down_wake_count();
    for round in 1..=2 {
        sleep_round(
            &mut rtc,
            &mut marker,
            &delay,
            &top_pd,
            "top-blocked  ",
            round,
        );
    }
    let downs_after_block = cpu_power_down_wake_count();
    println!(
        "safety-net: uart1 active + un-retained -> top-powerdown degraded, CPU power-downs {} (was {}) -> {}",
        downs_after_block,
        downs_before_block,
        if downs_after_block == downs_before_block {
            "BLOCKED (clock-gated) -> SAFE"
        } else {
            "POWERED DOWN -> UNSAFE"
        }
    );

    // Opt UART1 into retention: this drops its wake-lock and its registers are
    // saved/restored around the power-down instead. Kept alive for the proof.
    let _uart1_retention =
        uart1.with_retention_memory(mk_static!(UartRetentionMemory, UartRetentionMemory::new()));
    // SAFETY: UART1 is configured and clocked (driver alive), register readable.
    let clkdiv_before = unsafe { UART1_CLKDIV.read_volatile() };

    // Same for I2C0 (re-used from the negative control above). An idle I2C holds
    // no wake-lock, so it doesn't block pd_top; retention preserves its config.
    let _i2c0_retention =
        i2c0.with_retention_memory(mk_static!(I2cRetentionMemory, I2cRetentionMemory::new()));
    // SAFETY: I2C0 is configured and clocked (driver alive), register readable.
    let i2c_scl_before = unsafe { I2C0_SCL_LOW_PERIOD.read_volatile() };

    // ...and SPI2 (GPSPI2), whose CLOCK register holds the divider from `Spi::new`.
    const SPI2_CLOCK: *const u32 = 0x6008_100C as *const u32;
    let spi2 = Spi::new(peripherals.SPI2, SpiConfig::default()).unwrap();
    let _spi2_retention =
        spi2.with_retention_memory(mk_static!(SpiRetentionMemory, SpiRetentionMemory::new()));
    // SAFETY: SPI2 is configured and clocked (driver alive), register readable.
    let spi_clock_before = unsafe { SPI2_CLOCK.read_volatile() };

    // Same timer wakeup, increasingly aggressive power policies.
    let modes: [(&str, RtcSleepConfig); 3] = [
        ("clock-gated  ", clock_gated),
        ("cpu-powerdown", cpu_pd),
        ("top-powerdown", top_pd),
    ];

    for (label, config) in &modes {
        for round in 1..=ROUNDS {
            sleep_round(&mut rtc, &mut marker, &delay, config, label, round);
        }
    }

    // With retention, UART1's clock divider is unchanged across the power-down.
    let clkdiv_after = unsafe { UART1_CLKDIV.read_volatile() };
    println!(
        "UART1 CLKDIV: before = {:#x}, after top-powerdown = {:#x} -> {}",
        clkdiv_before,
        clkdiv_after,
        if clkdiv_after == clkdiv_before && clkdiv_after != 0 {
            "RETAINED"
        } else {
            "LOST"
        }
    );

    // Same check for I2C0's timing register.
    let i2c_scl_after = unsafe { I2C0_SCL_LOW_PERIOD.read_volatile() };
    println!(
        "I2C0 SCL_LOW_PERIOD: before = {:#x}, after top-powerdown = {:#x} -> {}",
        i2c_scl_before,
        i2c_scl_after,
        if i2c_scl_after == i2c_scl_before && i2c_scl_after != 0 {
            "RETAINED"
        } else {
            "LOST"
        }
    );

    // ...and for SPI2's clock register.
    let spi_clock_after = unsafe { SPI2_CLOCK.read_volatile() };
    println!(
        "SPI2 CLOCK: before = {:#x}, after top-powerdown = {:#x} -> {}",
        spi_clock_before,
        spi_clock_after,
        if spi_clock_after == spi_clock_before && spi_clock_after != 0 {
            "RETAINED"
        } else {
            "LOST"
        }
    );

    // Keep going in the deepest mode so the counter can be watched climbing and
    // a meter has a steady stream of identical sleep windows to average.
    let mut round = ROUNDS;
    loop {
        round += 1;
        sleep_round(
            &mut rtc,
            &mut marker,
            &delay,
            &top_pd,
            "top-powerdown",
            round,
        );
    }
}
