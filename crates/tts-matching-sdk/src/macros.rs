/// Declare a WASM mapper function that will be exported to the host
///
/// Usage:
/// ```ignore
/// use zako3_tts_matching_sdk::prelude::*;
///
/// fn process(input: Input) -> Output {
///     let text = input.text.replace("btw", "by the way");
///     Output::text(text)
/// }
///
/// export_mapper!(process);
/// ```
///
/// This macro generates `alloc` and `process` exports that:
/// 1. Allocate memory for JSON input
/// 2. Deserialize the input
/// 3. Call your function
/// 4. Serialize and return the output
#[macro_export]
macro_rules! export_mapper {
    ($fn:ident) => {
        #[cfg(target_arch = "wasm32")]
        mod __wasm_mapper_exports {
            use super::*;
            use $crate::__private::serde_json;

            #[unsafe(no_mangle)]
            pub extern "C" fn alloc(size: i32) -> i32 {
                let layout = ::std::alloc::Layout::from_size_align(size as usize, 1)
                    .expect("invalid layout");
                unsafe { ::std::alloc::alloc(layout) as i32 }
            }

            #[unsafe(no_mangle)]
            pub extern "C" fn process(input_ptr: i32, input_len: i32) -> i64 {
                let bytes = unsafe {
                    ::std::slice::from_raw_parts(input_ptr as *const u8, input_len as usize)
                };
                let input: $crate::types::Input = match serde_json::from_slice(bytes) {
                    Ok(v) => v,
                    Err(e) => {
                        return $crate::macros::encode_output($crate::types::Output::error(
                            e.to_string(),
                        ))
                    }
                };
                let output = super::$fn(input);
                $crate::macros::encode_output(output)
            }
        }
    };
}

/// Helper function to encode output as a packed i64
/// (output_ptr << 32) | output_len
/// The bytes are kept alive in WASM linear memory via std::mem::forget
pub fn encode_output(output: crate::types::Output) -> i64 {
    let bytes = serde_json::to_vec(&output).unwrap_or_default();
    let ptr = bytes.as_ptr() as i64;
    let len = bytes.len() as i64;
    std::mem::forget(bytes);
    (ptr << 32) | len
}
