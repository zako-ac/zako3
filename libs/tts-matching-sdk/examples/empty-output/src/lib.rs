use zako3_tts_matching_sdk::prelude::*;

/// Always returns empty text — used to test that the pipeline accepts
/// empty Output::text as a valid "block TTS" signal rather than treating
/// it as "no change".
pub fn process(_input: Input) -> Output {
    Output::text("")
}

export_mapper!(process);
