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

    // Check that the air-brake-controls submodule has been initialised.
    // If someone cloned without --recurse-submodules the directory will be
    // empty and the build would fail with a cryptic "can't find crate" error.
    let submodule_marker = std::path::Path::new("../air-brake-controls/controller_in_rust_v2/Cargo.toml");
    if !submodule_marker.exists() {
        panic!(
            "\n\n\
            ============================================================\n\
            ERROR: air-brake-controls submodule is not initialised.\n\
            Run:  git submodule update --init\n\
            Or clone with: git clone --recurse-submodules https://github.com/cornellrocketryteam/Software25-26.git\n\
            ============================================================\n"
        );
    }

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
