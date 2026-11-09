#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| conduit::fuzzing::request(data));
