// SPDX-License-Identifier: MIT

//! Automatically generated Rust version of Khipro, a compositional keyboard layout or input method
//! concept for Bengali, aiming to be the fastest typing method available for the language.

mod data;

/// Construct a fresh engine instance for this input method.
pub use data::instantiate_engine as engine;

#[test]
fn khipro_testcases() {
    let cases = std::fs::read_to_string("testcases.tsv").unwrap();
    let mut engine = engine();
    for (i, case) in cases.lines().enumerate().skip(1) {
        let (input, output) = case.split_once('\t').unwrap();
        assert_eq!(engine.convert(input), *output, "input #{i}: {input}");
    }
}
