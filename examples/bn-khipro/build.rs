// SPDX-License-Identifier: MIT

fn main() {
    println!("cargo::rerun-if-changed=bn-khipro.mim");

    m17n_im::Parser::parse_from_path("bn-khipro.mim")
        .unwrap()
        .generate(
            &mut m17n_im::Preprocessor::default(),
            &mut std::fs::File::create("src/data.rs").unwrap(),
        )
        .unwrap();
}
