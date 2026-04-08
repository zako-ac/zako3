use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lowercase_dir = manifest_dir.join("../../libs/tts-matching-sdk/examples/lowercase");

    // Only rebuild when source changes
    println!(
        "cargo:rerun-if-changed={}",
        lowercase_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        lowercase_dir.join("Cargo.toml").display()
    );

    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .current_dir(&lowercase_dir)
        .status()
        .expect("failed to invoke cargo for lowercase WASM example");
    assert!(status.success(), "lowercase WASM build failed");

    let wasm_src = lowercase_dir.join("target/wasm32-unknown-unknown/release/lowercase.wasm");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::copy(&wasm_src, out_dir.join("lowercase.wasm"))
        .expect("failed to copy lowercase.wasm to OUT_DIR");
}
