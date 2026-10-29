#[embedded_test::tests(default_timeout = 10, executor = hil_test::Executor::new())]
mod tests {
    use embassy_time::{Duration, Instant, Timer};
    use esp_hal::{
        clock::CpuClock,
        interrupt::Priority,
        peripherals::Peripherals,
        timer::timg::TimerGroup,
    };
    use esp_radio::wifi::WifiController;
    use esp_rtos::embassy::InterruptExecutor;
    use portable_atomic::{AtomicU32, Ordering};
    use static_cell::StaticCell;

    #[init]
    fn init() -> Peripherals {
        crate::init_heap();

        let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
        esp_hal::init(config)
    }

    // Calibration takes 39 ms (ESP32-C6) to 558 ms (ESP32-C5).
    #[test]
    async fn interrupts_run_while_wifi_calibrates(p: Peripherals) {
        static LONGEST_GAP_US: AtomicU32 = AtomicU32::new(0);

        #[embassy_executor::task]
        async fn measure_gaps() {
            let mut last = Instant::now();
            loop {
                Timer::after(Duration::from_millis(1)).await;
                let now = Instant::now();
                LONGEST_GAP_US.fetch_max((now - last).as_micros() as u32, Ordering::Relaxed);
                last = now;
            }
        }

        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0);

        static EXECUTOR: StaticCell<InterruptExecutor<2>> = StaticCell::new();
        let spawner = EXECUTOR
            .init(InterruptExecutor::new(p.FROM_CPU_INTR2))
            .start(Priority::Priority3);
        spawner.spawn(measure_gaps().unwrap());

        let _controller = WifiController::new(p.WIFI, Default::default()).unwrap();

        let longest_gap_us = LONGEST_GAP_US.load(Ordering::Relaxed);
        hil_test::assert!(
            longest_gap_us < 20_000,
            "Interrupts were blocked for {} us",
            longest_gap_us
        );
    }

    // Wi-Fi starts in a higher-priority thread while BLE calibrates. It must wait, and its clocks
    // must survive the calibration's clock restore.
    #[cfg(all(any(esp32c5, esp32c6, esp32c61), feature = "esp-radio-unstable"))]
    #[test]
    fn wifi_starts_while_ble_calibrates(p: Peripherals) {
        use esp_hal::peripherals::MODEM_SYSCON;
        use esp_radio::ble::controller::BleConnector;
        use esp_rtos::{CurrentThreadHandle, thread::ThreadSpawner};

        const WIFI_BASEBAND_CLOCKS: u32 = 0x1ff;
        fn wifi_baseband_clocks() -> u32 {
            MODEM_SYSCON::regs().clk_conf1().read().bits() & WIFI_BASEBAND_CLOCKS
        }

        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0);

        // Reset and earlier tests leave them on.
        MODEM_SYSCON::regs()
            .clk_conf1()
            .modify(|r, w| unsafe { w.bits(r.bits() & !WIFI_BASEBAND_CLOCKS) });

        let wifi = p.WIFI;
        let wifi_thread = ThreadSpawner::new(16384).with_priority(10).spawn(move || {
            while wifi_baseband_clocks() != WIFI_BASEBAND_CLOCKS
                && esp_radio::last_calibration_result().is_none()
            {
                CurrentThreadHandle::get().delay(esp_hal::time::Duration::from_micros(500));
            }
            let started_during_calibration = esp_radio::last_calibration_result().is_none();

            let controller = WifiController::new(wifi, Default::default()).unwrap();
            (controller, started_during_calibration)
        });

        let _connector = BleConnector::new(p.BT, Default::default()).unwrap();
        let ((_controller, started_during_calibration), _) = wifi_thread.join();

        hil_test::assert!(
            started_during_calibration,
            "The calibration did not let the Wi-Fi thread run"
        );
        hil_test::assert_eq!(wifi_baseband_clocks(), WIFI_BASEBAND_CLOCKS);
    }
}
