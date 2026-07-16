// SPDX-License-Identifier: MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme

//! A generalized parser, interpreter engine, and Rust code generator for
//! the m17n input method (`.mim`) database format.
//!
//! It has three features:
//!
//! - `codegen`: Used in build scripts to generate Rust module from `.mim` file.
//! - `dynamic`: Used for parsing and interpreting a `.mim` file.
//! - `embed`: Used for executing the generated Rust module.
//!
//! Examples for both can be found [in the repository][examples].
//!
//! [examples]: https://github.com/RagibHasin/m17n-im/tree/master/examples

// LINEBENDER LINT SET - lib.rs - v4
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod ir;

#[cfg_attr(docsrs, doc(cfg(feature = "codegen")))]
#[cfg(feature = "codegen")]
mod codegen;

#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
pub(crate) mod ident;

#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
pub mod ast;

#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
mod parser;
#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
pub use parser::{MimError, Parser};

#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
mod sexpr;

#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
mod dynamic;
#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
pub use dynamic::ParsedInputMethod;

#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
mod preprocess;
#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
#[cfg(feature = "dynamic")]
pub use preprocess::{Preprocessor, PreprocessorWarning};

#[cfg_attr(docsrs, doc(cfg(any(feature = "dynamic", feature = "embed"))))]
#[cfg(any(feature = "dynamic", feature = "embed"))]
mod engine;
#[cfg_attr(docsrs, doc(cfg(any(feature = "dynamic", feature = "embed"))))]
#[cfg(any(feature = "dynamic", feature = "embed"))]
pub use engine::Engine;

#[cfg_attr(docsrs, doc(cfg(feature = "embed")))]
#[cfg(feature = "embed")]
mod embeddable;
#[cfg_attr(docsrs, doc(cfg(feature = "embed")))]
#[cfg(feature = "embed")]
pub use embeddable::EmbeddableInputMethod;
#[cfg_attr(docsrs, doc(cfg(feature = "embed")))]
#[cfg(feature = "embed")]
#[doc(hidden)]
pub use embeddable::helpers as __embed;
