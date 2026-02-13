fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure().compile_protos(&["audio_engine.proto"], &["../../protos"])?;

    println!("cargo:rerun-if-changed=../../protos/audio_engine.proto");

    Ok(())
}
