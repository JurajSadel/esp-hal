use std::{
    path::{Path, PathBuf},
    process::Command,
    result::Result::Ok,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow, bail};
use clap::Parser;
#[derive(Parser)]
struct Args {
    #[arg(short = 'b', long)]
    binary: PathBuf,

    #[arg(long, default_value = "wifi_ap")]
    ap_test: String,

    #[arg(long, default_value = "wifi_dhcp")]
    sta_test: String,

    #[arg(long)]
    probes: Option<String>,

    #[arg(long, default_value = "120")]
    timeout: u64,
}

/// Run a radio test on 2 devices (AP and STA)
pub fn run_radio_test(
    binary_path: &Path,
    ap_test: &str,
    sta_test: &str,
    timeout_secs: u64,
    probes: Option<String>,
) -> Result<()> {
    use std::time::Duration;

    log::debug!("Binary: {}", binary_path.display());
    log::debug!("AP Test: {}", ap_test);
    log::debug!("STA Test: {}", sta_test);

    if !binary_path.exists() {
        bail!("Binary not found: {}", binary_path.display());
    }

    // Detect or use provided probes
    let probes_str = if let Some(p) = probes {
        p
    } else {
        detect_probes()?
    };

    let probes: Vec<&str> = probes_str.split(',').collect();

    if probes.len() != 2 {
        bail!("Expected 2 probes for radio test, got {}", probes.len());
    }

    let ap_probe = probes[0].trim();
    let sta_probe = probes[1].trim();

    log::debug!("AP Probe:  {}", ap_probe);
    log::debug!("STA Probe: {}", sta_probe);

    log::info!("\nResetting devices...");
    reset_probe(ap_probe)?;
    reset_probe(sta_probe)?;

    log::info!("Running radio tests...\n");

    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    let timeout = Duration::from_secs(timeout_secs);
    let binary_str = binary_path.to_string_lossy().to_string();
    let ap_test_name = ap_test.to_string();
    let sta_test_name = sta_test.to_string();
    let ap_probe_str = ap_probe.to_string();
    let sta_probe_str = sta_probe.to_string();

    let binary_str_cloned = binary_str.clone();
    let ap_should_stop = Arc::new(Mutex::new(false));
    let ap_should_stop_clone = Arc::clone(&ap_should_stop);

    // Spawn AP thread
    let ap_thread = thread::spawn(move || {
        run_test_with_rtt(
            "AP",
            &binary_str_cloned,
            &ap_test_name,
            &ap_probe_str,
            timeout,
            Some(ap_should_stop_clone),
        )
    });

    // Wait for AP to start
    log::debug!("[STA] Waiting 2000 ms for AP to start...");
    thread::sleep(Duration::from_millis(2000));
    log::debug!("[STA] Starting now...");

    // Spawn STA thread
    let sta_thread = thread::spawn(move || {
        run_test_with_rtt(
            "STA",
            &binary_str,
            &sta_test_name,
            &sta_probe_str,
            timeout,
            None,
        )
    });

    // Wait for STA to complete
    let sta_result = sta_thread.join().unwrap_or(Ok(false)).unwrap_or(false);

    // Signal AP to stop if STA passed
    if sta_result {
        log::debug!("\n[MAIN] STA passed! Signaling AP to stop...");
        {
            let mut should_stop = ap_should_stop.lock().unwrap();
            *should_stop = true;
        }
        thread::sleep(Duration::from_millis(500));
    }

    // Wait for AP to complete
    let ap_result = ap_thread.join().unwrap_or(Ok(false)).unwrap_or(false);

    if ap_result && sta_result {
        Ok(())
    } else {
        panic!("Radio test(s) failed");
    }
}

fn detect_probes() -> Result<String> {
    let output = Command::new("probe-rs").args(&["list"]).output()?;

    if !output.status.success() {
        return Err(anyhow!("Failed to run probe-rs list"));
    }

    let list_output = String::from_utf8_lossy(&output.stdout);

    let mut probes = Vec::new();
    for line in list_output.lines() {
        if line.contains("ESP") {
            if let Some(start) = line.find("--") {
                let after_dash = &line[start + 2..].trim();
                if let Some(end) = after_dash.find(' ') {
                    let serial = &after_dash[..end];
                    probes.push(serial.to_string());
                }
            }
        }
    }

    if probes.len() < 2 {
        return Err(anyhow!("Expected 2 probes, found {}", probes.len()));
    }

    Ok(format!("{},{}", probes[0], probes[1]))
}

fn run_test_with_rtt(
    name: &str,
    binary: &str,
    test_name: &str,
    probe: &str,
    timeout: Duration,
    should_stop: Option<Arc<Mutex<bool>>>,
) -> Result<bool> {
    log::info!("[{}] Running: {}", name, test_name);

    let mut child = Command::new("probe-rs")
        .args(&["run", "--probe", probe, binary, test_name])
        .env("DEFMT_LOG", "info")
        .spawn()
        .map_err(|e| anyhow!("[{}] Failed to spawn: {}", name, e))?;

    log::info!("[{}] Waiting for test to complete...", name);

    let start = Instant::now();
    loop {
        // Check if we should stop (for AP when STA passes)
        if let Some(ref stop_flag) = should_stop {
            if let Ok(should_stop_val) = stop_flag.lock() {
                if *should_stop_val {
                    let _ = child.kill();
                    return Ok(true);
                }
            }
        }

        if start.elapsed() > timeout {
            log::info!("[{}] TIMEOUT", name);
            let _ = child.kill();
            return Ok(false);
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(true);
                } else {
                    log::info!(
                        "[{}] FAILED (exit code: {})",
                        name,
                        status.code().unwrap_or(-1)
                    );
                    return Ok(false);
                }
            }
            Ok(None) => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(anyhow!("[{}] Error: {}", name, e));
            }
        }
    }
}

fn reset_probe(probe: &str) -> Result<()> {
    Command::new("probe-rs")
        .args(&["reset", "--probe", probe])
        .output()?;

    thread::sleep(Duration::from_millis(500));

    Ok(())
}
