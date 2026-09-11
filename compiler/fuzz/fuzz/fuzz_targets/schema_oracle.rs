#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    ethos_fuzz::fuzz_schema_oracle_cov(data);
});
