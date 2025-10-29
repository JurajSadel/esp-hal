//! Demonstrates deep sleep with timer wakeup

//% CHIPS: esp32 esp32c3 esp32c6 esp32s2 esp32s3 esp32c2

#![no_std]
#![no_main]

use core::time::Duration;

use esp_backtrace as _;
use esp_hal::{
    delay::Delay,
    main,
    rtc_cntl::{Rtc, SocResetReason, reset_reason, sleep::TimerWakeupSource, wakeup_cause},
    system::Cpu,
};
use esp_println::println;

esp_bootloader_esp_idf::esp_app_desc!();

#[used]
static mut BUFFER12: [u8; 10024] = [0; 10024];

#[cfg(feature = "esp32c3")]
#[used]
static mut BUFFER123: [u8; 10025] = [0; 10025];

#[used]
static LARGE_INIT_DATA: [u32; 5000] = [0xDEADBEEF; 5000];

#[used]
static MY_STRING: &'static str = "Hello, World!";

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let delay = Delay::new();
    let mut rtc = Rtc::new(peripherals.LPWR);

    println!("up and runnning!");
    let reason = reset_reason(Cpu::ProCpu).unwrap_or(SocResetReason::ChipPowerOn);
    println!("reset reason: {:?}", reason);
    let wake_reason = wakeup_cause();
    println!("wake reason: {:?}", wake_reason);

    let timer = TimerWakeupSource::new(Duration::from_secs(5));
    println!("sleeping!");
    delay.delay_millis(100);
    rtc.sleep_deep(&[&timer]);
}
