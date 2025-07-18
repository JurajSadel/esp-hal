#![cfg_attr(not(feature = "build-script"), no_std)]
#[cfg(all(not(feature = "build-script"), feature = "esp32"))]
include!("_generated_esp32.rs");
#[cfg(all(not(feature = "build-script"), feature = "esp32c2"))]
include!("_generated_esp32c2.rs");
#[cfg(all(not(feature = "build-script"), feature = "esp32c3"))]
include!("_generated_esp32c3.rs");
#[cfg(all(not(feature = "build-script"), feature = "esp32c6"))]
include!("_generated_esp32c6.rs");
#[cfg(all(not(feature = "build-script"), feature = "esp32h2"))]
include!("_generated_esp32h2.rs");
#[cfg(all(not(feature = "build-script"), feature = "esp32s2"))]
include!("_generated_esp32s2.rs");
#[cfg(all(not(feature = "build-script"), feature = "esp32s3"))]
include!("_generated_esp32s3.rs");
#[cfg(feature = "build-script")]
include!("_build_script_utils.rs");
