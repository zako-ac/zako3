use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let sdk_dir = manifest_dir.join("../../libs/tts-matching-sdk");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Build lowercase example
    let lowercase_dir = sdk_dir.join("examples/lowercase");
    let out_lowercase = out_dir.join("lowercase.wasm");
    build_example(&lowercase_dir, &out_lowercase);

    // Build empty-output example
    let empty_output_dir = sdk_dir.join("examples/empty-output");
    let out_empty_output = out_dir.join("empty-output.wasm");
    build_example(&empty_output_dir, &out_empty_output);
}

fn build_example(example_dir: &Path, out_wasm: &Path) {
    println!(
        "cargo:rerun-if-changed={}",
        example_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        example_dir.join("Cargo.toml").display()
    );

    if !wasm32_target_installed() {
        println!(
            "cargo:warning=wasm32-unknown-unknown target not installed; skipping {} build (integration tests will be unable to load it)",
            example_dir.file_name().unwrap().to_string_lossy()
        );
        std::fs::write(out_wasm, b"").expect("failed to write placeholder wasm");
        return;
    }

    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release"])
        .current_dir(example_dir)
        .status()
        .expect("failed to invoke cargo for WASM example");
    assert!(status.success(), "WASM example build failed");

    let crate_name = example_dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .replace('-', "_");
    let wasm_src = example_dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{crate_name}.wasm"));
    std::fs::copy(&wasm_src, out_wasm).expect("failed to copy wasm to OUT_DIR");
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
