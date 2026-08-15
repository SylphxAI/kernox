#![no_main]

use kernox_core::{CompositionSpec, GraphBuilder};
use libfuzzer_sys::fuzz_target;

const MAX_FUZZ_INPUT_BYTES: usize = 1_048_576;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }
    if let Ok(spec) = serde_json::from_slice::<CompositionSpec>(data) {
        let _result = GraphBuilder::from_spec(spec).resolve();
    }
});
