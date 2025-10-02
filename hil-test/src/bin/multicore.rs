//! Multicore flash cache coherency test
//!
//! This test verifies that flash operations remain correct when running
//! on one core while the other core creates significant cache pressure.
//! It tests the `multicore_auto_park()` functionality.

//% CHIPS: esp32 esp32s3
//% FEATURES: unstable esp-storage esp-storage/defmt defmt

#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

use core::ptr::addr_of_mut;

use defmt::info;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    peripherals::FLASH,
    system::{Cpu, CpuControl, Stack},
    time::{Duration, Instant},
};
use esp_storage::FlashStorage;
use hil_test as _;

// Pick which core does flash access
const FLASH_ON_CORE_0: bool = true;

// Test write location (must be in writable flash region)
const TEST_ADDR: u32 = 0x9000;

// Stack for app core
static mut APP_CORE_STACK: Stack<{ 4096 * 8 }> = Stack::new();

/// Macro to generate a block of NOP instructions for creating cache pressure
macro_rules! nop_block {
    ($count:expr) => {
        unsafe {
            core::arch::asm!(concat!(
                ".rept ",
                stringify!($count),
                "\n",
                "    nop\n",
                ".endr\n"
            ));
        }
    };
}

/// Creates cache pressure by executing many NOPs and memory operations
fn create_cache_pressure() {
    for _ in 0..50 {
        nop_block!(100);
    }
}

/// Task that performs flash operations while under cache pressure from other core
fn flash_access(flash: esp_hal::peripherals::FLASH) -> ! {
    info!("flash access task started");

    let mut flash = FlashStorage::new(flash).multicore_auto_park();
    let mut buffer = [0xAAu8; 512];

    for (i, byte) in buffer.iter_mut().enumerate() {
        *byte = (i + 1) as u8;
    }

    // Initial erase to start with clean flash
    info!("performing initial flash erase");
    match flash.erase(TEST_ADDR, TEST_ADDR + 4096) {
        Ok(()) => info!("initial erase successful"),
        Err(e) => panic!("initial erase failed: {:?}", e),
    }

    let mut round = 0;
    let mut success_count = 0;
    let mut fail_count = 0;

    loop {
        info!("Test Round {}", round);
        create_cache_pressure();

        // Write to flash without erasing - this tests cache coherency
        info!("writing to flash (no erase)");
        match flash.write(TEST_ADDR, &buffer) {
            Ok(()) => info!("write successful"),
            Err(e) => {
                info!("write failed: {:?}", e);
                fail_count += 1;
                continue;
            }
        }

        create_cache_pressure();

        // Read back to verify
        let mut readback = [0u8; 512];
        match flash.read(TEST_ADDR, &mut readback) {
            Ok(()) => info!("read successful"),
            Err(e) => {
                info!("read failed: {:?}", e);
                fail_count += 1;
                continue;
            }
        }

        // Verify data integrity
        if buffer[..] == readback[..] {
            info!("VERIFY SUCCESS");
            success_count += 1;
        } else {
            info!("VERIFY FAILED");
            fail_count += 1;

            // Diagnostic: show first mismatch
            for i in 0..buffer.len() {
                if buffer[i] != readback[i] {
                    info!(
                        "first mismatch at {}: expected 0x{:02x}, got 0x{:02x}",
                        i, buffer[i], readback[i]
                    );
                    break;
                }
            }

            // Recovery: re-erase and reset pattern after failure
            info!("recovering with flash erase");
            match flash.erase(TEST_ADDR, TEST_ADDR + 4096) {
                Ok(()) => info!("recovery erase successful"),
                Err(e) => info!("recovery erase failed: {:?}", e),
            }

            // Reset to known pattern
            for (i, byte) in buffer.iter_mut().enumerate() {
                *byte = (i + 1) as u8;
            }
        }

        info!(
            "Cumulative results: {} successes, {} failures",
            success_count, fail_count
        );

        // Modify pattern for next round
        for byte in buffer.iter_mut() {
            *byte = byte.wrapping_add(1);
        }

        round += 1;
        Delay::new().delay_millis(300);
    }
}

/// Task that creates continuous cache pressure
fn cache_pressure_task() -> ! {
    info!("cache pressure task started");

    // Working data set to create actual cache pressure
    let mut data = [0u32; 1024];

    loop {
        // Computational workload
        for i in 0..data.len() {
            data[i] = data[i].wrapping_mul(1664525).wrapping_add(1013904223);
            nop_block!(2);
        }

        // Additional NOP pressure
        create_cache_pressure();

        // Memory access pattern that stresses cache
        let mut sum: u32 = 0;
        for i in (0..data.len()).step_by(7) {
            sum = sum.wrapping_add(data[i]);
            nop_block!(1);
        }

        Delay::new().delay_millis(50);
    }
}

#[embedded_test::tests]
mod tests {
    use super::*;

    #[init]
    fn init() -> esp_hal::peripherals::Peripherals {
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()))
    }

    #[test]
    #[timeout(30)]
    fn test_multicore_flash_cache_coherency(mut peripherals: esp_hal::peripherals::Peripherals) {
        info!("Starting multicore flash cache coherency test");
        info!("Configuration: FLASH_ON_CORE_0 = {}", FLASH_ON_CORE_0);

        let mut cpu_control = CpuControl::new(peripherals.CPU_CTRL);

        // Start the second core
        let _guard = cpu_control
            .start_app_core(unsafe { &mut *addr_of_mut!(APP_CORE_STACK) }, move || {
                info!("App core started");

                if !FLASH_ON_CORE_0 {
                    info!("Running flash access on app core");
                    flash_access(peripherals.FLASH);
                } else {
                    info!("Running cache pressure on app core");
                    cache_pressure_task();
                }

                loop {}
            })
            .unwrap();

        // Allow app core to initialize
        Delay::new().delay_millis(1500);

        // Run complementary task on main core
        if FLASH_ON_CORE_0 {
            info!("Running flash access on main core");
            flash_access(unsafe { FLASH::steal() });
        } else {
            info!("Running cache pressure on main core");
            cache_pressure_task();
        }

        // Test will run until timeout
        loop {}
    }
}
