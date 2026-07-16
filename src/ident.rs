// SPDX-License-Identifier: MIT

//! Identifier interning.
//!
//! The m17n `.mim` grammar has two very different kinds of "identifier":
//!
//! 1. A **fixed vocabulary** defined by the grammar itself (action keywords, predefined markers,
//!    set/add/sub/mul/div, operators, modifier keys). These have a bounded, compile-time-known set
//!    of possibilities, so they are modeled as genuine Rust `enum`s directly in [`crate::ast`].
//! 2. **User-defined names** from the source file (state names, map names, variable names,
//!    macro names, marker names, module/function names). These are open-ended, a generic parser
//!    can't know them until it has read the file, so a literal `enum` with fixed variants isn't
//!    possible for this crate's parser/engine (which must handle *any* `.mim` file).
//!    Rather than fall back to `&'static str`/`String` comparisons everywhere, every such name is
//!    *interned* into an [`Ident`]: a small `Copy` integer newtype, compared and hashed as a plain
//!    integer instead of a string.

use std::collections::HashMap;

/// Interned identifier.
///
/// A cheap `Copy` handle standing in for a name (state name, map name, variable name, etc.)
/// that appeared in the source `.mim` file.
///
/// Compare/hash this directly. Use [`Interner::resolve`] only when you need the text back
/// for diagnostics, code generation, etc. Must be paired to the right [`Interner`] for this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ident(u32);
impl crate::ir::Id for Ident {}

/// Identifier interner.
///
/// It is used to provide cheap to compare IDs to various AST objects.
#[derive(Default, Debug, Clone)]
pub struct Interner {
    strings: Vec<String>,
    lookup: HashMap<String, Ident>,
}

impl Interner {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "infinitesimal chance that the number of ident reaches u32::MAX"
    )]
    pub(crate) fn intern(&mut self, s: &str) -> Ident {
        if let Some(&id) = self.lookup.get(s) {
            return id;
        }
        let id = Ident(self.strings.len() as u32);
        self.strings.push(s.to_string());
        self.lookup.insert(s.to_string(), id);
        id
    }

    /// Resolves the `id` back to its text.
    pub fn resolve(&self, id: Ident) -> &str {
        &self.strings[id.0 as usize]
    }
}
