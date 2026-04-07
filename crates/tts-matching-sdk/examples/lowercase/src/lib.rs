use zako3_tts_matching_sdk::prelude::*;

fn process(input: Input) -> Output {
    Output::text(input.text.to_lowercase())
}

export_mapper!(process);
