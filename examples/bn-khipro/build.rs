// SPDX-License-Identifier: MIT

fn main() {
    println!("cargo::rerun-if-changed=bn-khipro-classic.mim");

    m17n_im::Parser::parse_from_path("bn-khipro-classic.mim")
        .unwrap()
        .generate(
            &mut m17n_im::Preprocessor::default(),
            &mut std::fs::File::create("src/classic.rs").unwrap(),
        )
        .unwrap();

    println!("cargo::rerun-if-changed=bn-khipro-dark.mim");

    m17n_im::Parser::parse_from_path("bn-khipro-dark.mim")
        .unwrap()
        .generate(
            &mut m17n_im::Preprocessor::default(),
            &mut std::fs::File::create("src/dark.rs").unwrap(),
        )
        .unwrap();
}
