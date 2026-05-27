use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lowercase_dir = manifest_dir.join("../../libs/tts-matching-sdk/examples/lowercase");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let out_wasm = out_dir.join("lowercase.wasm");

    println!(
        "cargo:rerun-if-changed={}",
        lowercase_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        lowercase_dir.join("Cargo.toml").display()
    );

    if !wasm32_target_installed() {
        println!(
            "cargo:warning=wasm32-unknown-unknown target not installed; skipping lowercase.wasm build (integration tests will be unable to load it)"
        );
        // Write an empty placeholder so `include_bytes!` still resolves at compile time.
        // Tests that depend on the bytes will fail at runtime, which is intentional —
        // they require the toolchain to be present.
        std::fs::write(&out_wasm, b"").expect("failed to write placeholder lowercase.wasm");
        return;
    }

    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .current_dir(&lowercase_dir)
        .status()
        .expect("failed to invoke cargo for lowercase WASM example");
    assert!(status.success(), "lowercase WASM build failed");

    let wasm_src = lowercase_dir.join("target/wasm32-unknown-unknown/release/lowercase.wasm");
    std::fs::copy(&wasm_src, &out_wasm).expect("failed to copy lowercase.wasm to OUT_DIR");
}

fn wasm32_target_installed() -> bool {
    let output = match Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.trim() == "wasm32-unknown-unknown")
}
