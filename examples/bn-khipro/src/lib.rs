// SPDX-License-Identifier: MIT

//! Automatically generated Rust version of Khipro, a compositional keyboard layout or input method
//! concept for Bengali, aiming to be the fastest typing method available for the language.

mod classic;
mod dark;

/// Construct a fresh engine instance for Khipro Dark input method.
pub use dark::instantiate_engine as engine_dark;

/// Construct a fresh engine instance for Khipro Classic input method.
pub use classic::instantiate_engine as engine_classic;

#[test]
fn khipro_testcases() {
    fn evaluate<IM: m17n_im::ir::InputMethod + 'static>(
        path: &str,
        mut engine: m17n_im::Engine<IM>,
    ) {
        let cases = std::fs::read_to_string(path).unwrap();

        let mut n_failed = 0;
        for (i, case) in cases.lines().enumerate().skip(1) {
            let (input, expected) = case.split_once('\t').unwrap();
            let actual = engine.convert(input);
            if expected != actual {
                println!(
                    r#"
input #{i}: {input}
expected: {expected}
actual: {actual}"#
                );
                n_failed += 1;
            }
        }

        assert!(n_failed == 0, "Failed {} test cases", n_failed);
    }

    evaluate("testcases-classic.tsv", engine_classic());
    evaluate("testcases-dark.tsv", engine_dark());
}
