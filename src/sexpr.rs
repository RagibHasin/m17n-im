// SPDX-License-Identifier: MIT

//! An (opportunistic) zero-copy S-expression parser for `.mim` source.
//!
//! This replaces an earlier version of this crate that used the
//! `s-expression` crate plus a whole separate preprocessing pass
//! (`preprocess.rs`) to work around that crate's limitations: its
//! tokenizer had no notion of string literals (so `"a b"` split into two
//! tokens), no comment syntax, and no escape handling. Writing the reader
//! ourselves means those concerns are handled *correctly, in one place*,
//! during tokenization -- no pre-pass required.
//!
//! ## Zero-copy
//!
//! [`SExpr::Symbol`] always borrows directly from the input (`&'s str`,
//! never allocated). [`SExpr::Str`] borrows too ([`Cow::Borrowed`]) for the
//! very common case of a string with no backslash escapes -- which is most
//! strings in a typical `.mim` file (single characters/short syllables).
//! Only strings that actually contain an escape sequence pay for an
//! allocation ([`Cow::Owned`]), because escape-decoding necessarily
//! produces bytes that differ from the source.
//!
//! ## Grammar
//!
//! A `.mim` file is a sequence of top-level forms, each either:
//! - a **list**: `(` form* `)`, arbitrarily nested;
//! - a **string**: `"..."`, with m17n's backslash escapes (`\t \n \r \e
//!   \xXX \<char>`, where any other character after `\` stands for itself);
//! - a **symbol**: any run of characters not whitespace, `(`, `)`, `"`, or
//!   `;` (this covers m17n symbols, integers, and operators alike --
//!   distinguishing "is this symbol actually an integer" is left to
//!   `crate::parser`, matching the grammar's own ambiguity between `SYMBOL`
//!   and `INTEGER` tokens).
//! - `;` starts a line comment, extending to the next newline.

use std::borrow::Cow;

use winnow::ascii::{escaped, hex_uint, multispace1, till_line_ending};
use winnow::combinator::{alt, cut_err, delimited, preceded, repeat, terminated};
use winnow::error::{StrContext, StrContextValue};
use winnow::token::take_till;
use winnow::{ModalResult, Parser};

#[derive(Debug, Clone, PartialEq)]
#[expect(unnameable_types, reason = "implementation details")]
pub enum SExpr<'s> {
    /// A bare atom: could be a name, an operator, or (per `crate::parser`'s later interpretation)
    /// an integer literal. The reader itself makes no distinction, matching the grammar.
    Symbol(&'s str),
    /// A `"..."` string literal, already escape-decoded. Borrowed when the source had no
    /// escape sequence to resolve, owned otherwise.
    Str(Cow<'s, str>),
    List(Vec<Self>),
}

impl std::fmt::Display for SExpr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SExpr::Symbol(s) => f.write_str(s),
            SExpr::Str(s) => f.write_str(
                &s.replace('"', "\\\"")
                    .replace('\\', "\\\\")
                    .replace('\t', "\\t")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('', "\\e"),
            ),
            SExpr::List(items) => {
                f.write_str("(")?;
                for item in items {
                    item.fmt(f)?;
                    f.write_str(" ")?;
                }
                f.write_str(")")?;
                Ok(())
            }
        }
    }
}

/// Parse a complete `.mim` source string into its top-level forms.
pub(crate) fn file<'s>(input: &mut &'s str) -> ModalResult<Vec<SExpr<'s>>> {
    terminated(repeat(0.., preceded(ws_or_comment0, form)), ws_or_comment0).parse_next(input)
}

// MARK: Whitespace & comments

/// `;` line comment, extending to (but not including) the next newline.
fn comment(input: &mut &str) -> ModalResult<()> {
    (';', till_line_ending).void().parse_next(input)
}

/// Zero or more whitespace runs and/or comments, in any interleaving.
fn ws_or_comment0(input: &mut &str) -> ModalResult<()> {
    repeat(0.., alt((multispace1.void(), comment))).parse_next(input)
}

// MARK: Forms

fn form<'s>(input: &mut &'s str) -> ModalResult<SExpr<'s>> {
    alt((list, string, symbol)).parse_next(input)
}

fn list<'s>(input: &mut &'s str) -> ModalResult<SExpr<'s>> {
    delimited(
        '(',
        repeat(0.., preceded(ws_or_comment0, form)).map(SExpr::List),
        cut_err(preceded(ws_or_comment0, ')'))
            .context(StrContext::Expected(StrContextValue::CharLiteral(')'))),
    )
    .parse_next(input)
}

fn symbol<'s>(input: &mut &'s str) -> ModalResult<SExpr<'s>> {
    take_till(1.., (char::is_whitespace, '(', ')', '"', ';'))
        .map(SExpr::Symbol)
        .parse_next(input)
}

// MARK: Strings

fn string<'s>(input: &mut &'s str) -> ModalResult<SExpr<'s>> {
    preceded(
        '"',
        cut_err(terminated(
            escaped(
                take_till(1.., ['\\', '"']),
                '\\',
                alt((
                    '\\',
                    '"',
                    't'.value('\t'),
                    'n'.value('\n'),
                    'r'.value('\r'),
                    'e'.value(''),
                    (alt(('x', 'X')), hex_uint)
                        .try_map(|(_, hex)| char::from_u32(hex).ok_or(InvalidCodepoint)),
                )),
            ),
            '"'.context(StrContext::Expected(StrContextValue::CharLiteral('"'))),
        )),
    )
    .map(SExpr::Str)
    .parse_next(input)
}

#[derive(Debug)]
struct InvalidCodepoint;
impl std::fmt::Display for InvalidCodepoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid codepoint")
    }
}
impl std::error::Error for InvalidCodepoint {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_borrow_directly_from_source() {
        let src = "(hello world)";
        let forms = file.parse(src).unwrap();
        let list = match &forms[0] {
            SExpr::List(items) => items,
            _ => panic!("expected list"),
        };
        match &list[0] {
            SExpr::Symbol(s) => {
                // the symbol's data pointer must fall within the source
                // string's byte range -- proof it's a genuine borrow, not a copy.
                let src_range = src.as_ptr() as usize..src.as_ptr() as usize + src.len();
                assert!(src_range.contains(&(s.as_ptr() as usize)));
            }
            other => panic!("expected symbol, got {other:?}"),
        }
    }

    #[test]
    fn strings_without_escapes_are_borrowed_not_copied() {
        let src = r#"("plain text")"#;
        let forms = file.parse(src).unwrap();
        let list = match &forms[0] {
            SExpr::List(items) => items,
            _ => panic!("expected list"),
        };
        match &list[0] {
            SExpr::Str(Cow::Borrowed(s)) => assert_eq!(*s, "plain text"),
            other => panic!("expected a borrowed string, got {other:?}"),
        }
    }

    #[test]
    fn strings_with_escapes_are_owned() {
        let src = r#"("a\tb")"#;
        let forms = file.parse(src).unwrap();
        let list = match &forms[0] {
            SExpr::List(items) => items,
            _ => panic!("expected list"),
        };
        match &list[0] {
            SExpr::Str(Cow::Owned(s)) => assert_eq!(s, "a\tb"),
            other => panic!("expected an owned string, got {other:?}"),
        }
    }

    #[test]
    fn strings_containing_whitespace_and_parens_parse_as_one_token() {
        let src = r#"("a (b) c" other)"#;
        let forms = file.parse(src).unwrap();
        let list = match &forms[0] {
            SExpr::List(items) => items,
            _ => panic!("expected list"),
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], SExpr::Str(Cow::Borrowed("a (b) c")));
        assert_eq!(list[1], SExpr::Symbol("other"));
    }

    #[test]
    fn trailing_escaped_backslash_does_not_swallow_the_rest_of_the_file() {
        let src = r#"("\\" "next")"#;
        let forms = file.parse(src).unwrap();
        let list = match &forms[0] {
            SExpr::List(items) => items,
            _ => panic!("expected list"),
        };
        assert_eq!(list[0], SExpr::Str(Cow::Owned("\\".to_string())));
        assert_eq!(list[1], SExpr::Str(Cow::Borrowed("next")));
    }

    #[test]
    fn comments_are_skipped() {
        let src = "; a comment\n(a b) ; trailing\n(c)";
        let forms = file.parse(src).unwrap();
        assert_eq!(forms.len(), 2);
    }

    #[test]
    fn multiple_top_level_forms() {
        let src = "(a) (b) (c)";
        let forms = file.parse(src).unwrap();
        assert_eq!(forms.len(), 3);
    }
}
