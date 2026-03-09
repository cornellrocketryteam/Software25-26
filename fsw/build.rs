//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");

    // SV test duration configuration (compile-time, since this is #![no_std])
    let sv_dur_secs = env::var("SV_DURATION_SECS").unwrap_or_else(|_| "2".to_string());
    let sv_dur_ms: u64 = sv_dur_secs
        .parse::<u64>()
        .expect("SV_DURATION_SECS must be a valid integer")
        * 1000;
    File::create(out.join("sv_test_config.rs"))
        .unwrap()
        .write_all(format!("pub const SV_TEST_DURATION_MS: u64 = {};", sv_dur_ms).as_bytes())
        .unwrap();
    println!("cargo:rerun-if-env-changed=SV_DURATION_SECS");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
