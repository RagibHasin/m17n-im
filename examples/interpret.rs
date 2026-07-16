// SPDX-License-Identifier: MIT

//! A tiny CLI interpreter example for a m17n input method (.mim) file.
//!
//! Use it like this:
//!
//! ```sh
//! cargo r --example interpret --features=dynamic -- examples/bn-khipro/bn-khipro.mim "ami baxlay gan gai"
//! ```

use std::env;

fn main() -> Result<(), ()> {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!(
            "usage: cargo r --example interpret --features=dynamic -- <input.mim> [text-to-convert]"
        );
        return Err(());
    };

    let file = match m17n_im::Parser::parse_from_path(&path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("{e}");
            return Err(());
        }
    };

    println!("Parsed: {path}");
    if let Some(decl) = &file.declaration {
        println!(
            "  input-method: ({} {} {})",
            file.interner.resolve(decl.language),
            file.interner.resolve(decl.name),
            decl.extra_id
                .map(|id| file.interner.resolve(id))
                .unwrap_or_default()
        );
    }
    if let Some(title) = &file.title {
        println!("  title: {title:?}");
    }
    println!("  variables: {}", file.variables.len());
    println!("  commands: {}", file.commands.len());
    println!("  modules: {}", file.modules.len());
    println!("  macros: {}", file.macros.len());
    println!("  maps: {}", file.maps.len());
    for map in &file.maps {
        println!(
            "    {}: {}",
            file.interner.resolve(map.name),
            map.rules.len()
        );
    }
    println!("  states: {}", file.states.len());
    for state in &file.states {
        println!(
            "    {}: {}",
            file.interner.resolve(state.name),
            state.branches.len()
        );
    }
    match file.unknown_forms.len() {
        0 => println!("  ✓ every top-level form recognized"),
        n => println!("  ⚠ unrecognized top-level forms: {n}",),
    }

    if let Some(text) = args.next() {
        let mut engine = m17n_im::Engine::new(
            file.into_input_method(&mut m17n_im::Preprocessor::default())
                .unwrap(),
        );
        print!("\nconverting : {text}\n           ⤷ ");
        for word in text.split_whitespace() {
            print!("{} ", engine.convert(word));
        }
    }

    Ok(())
}
