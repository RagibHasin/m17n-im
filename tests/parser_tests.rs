// SPDX-License-Identifier: MIT

//! Parser tests

use m17n_im::{Engine, Parser, Preprocessor, ast::*};

#[test]
fn parses_declaration_title_description() {
    let file = Parser::parse(
        r#"
        (input-method bn khipro)
        (description "hello \"world\"")
        (title "Khipro")
        "#,
    )
    .unwrap();
    let decl = file.declaration.as_ref().unwrap();
    assert_eq!(file.interner.resolve(decl.language), "bn");
    assert_eq!(file.interner.resolve(decl.name), "khipro");
    assert_eq!(
        file.description.as_ref().unwrap().text.as_deref(),
        Some("hello \"world\"")
    );
    assert_eq!(file.title.as_deref(), Some("Khipro"));
}

#[test]
fn resolves_backslash_escapes_including_trailing_backslash_edge_case() {
    // Regression test: a string ending in an escaped backslash must not
    // swallow the rest of the source (see `preprocess`'s module docs).
    let file = Parser::parse(
        r#"
        (map
          (m
            ("\\\\" "\\")
            ("x" "y")))
        "#,
    )
    .unwrap();
    let m = &file.maps[0];
    match &m.rules[0].key_seq {
        KeySeq::Text(t) => assert_eq!(t, "\\\\"),
        _ => panic!("expected text keyseq"),
    }
    match &m.rules[0].actions[0] {
        Action::InsertText(t) => assert_eq!(t, "\\"),
        other => panic!("unexpected action: {other:?}"),
    }
    match &m.rules[1].key_seq {
        KeySeq::Text(t) => assert_eq!(t, "x"),
        _ => panic!("expected text keyseq"),
    }
}

#[test]
fn resolves_strings_containing_spaces() {
    // The underlying `s-expression` tokenizer splits on whitespace with no
    // string awareness; this is exactly what `preprocess` fixes.
    let file = Parser::parse(r#"(description "a multi word description")"#).unwrap();
    assert_eq!(
        file.description.unwrap().text.as_deref(),
        Some("a multi word description")
    );
}

#[test]
fn parses_char_literals_in_key_events_and_insert_actions() {
    let file = Parser::parse(r#"(map (m ((?a) (insert ?b))))"#).unwrap();
    let rule = &file.maps[0].rules[0];
    match &rule.key_seq {
        KeySeq::Events(evs) => assert!(matches!(evs[0], KeyEvent::Code(97))),
        _ => panic!("expected event keyseq"),
    }
    assert!(matches!(rule.actions[0], Action::InsertChar('b')));
}

#[test]
fn parses_modifier_key_symbols() {
    let file = Parser::parse(r#"(map (m ((S-M-H-Return) (commit))))"#).unwrap();
    match &file.maps[0].rules[0].key_seq {
        KeySeq::Events(evs) => match &evs[0] {
            KeyEvent::Symbol { modifiers, base } => {
                assert_eq!(
                    modifiers,
                    &[KeyModifier::Shift, KeyModifier::Meta, KeyModifier::Hyper]
                );
                assert_eq!(base, "Return");
            }
            _ => panic!("expected symbol key event"),
        },
        _ => panic!("expected event keyseq"),
    }
}

#[test]
fn parses_set_cond_and_expressions() {
    let out = Parser::parse(
        r#"
        (state
          (init
            (m
              (cond
                ((= CID 0) (shift init))
                (1 (set CID 5) (shift other))))))
        "#,
    )
    .unwrap();
    let branch = &out.states[0].branches[0];
    match branch {
        Branch::Map { actions, .. } => match &actions[0] {
            Action::Cond(clauses) => {
                assert_eq!(clauses.len(), 2);
                match &clauses[0].test {
                    Expr::Op(Operator::Eq, args) => {
                        assert!(matches!(args[0], Expr::Variable(_)));
                        assert!(matches!(args[1], Expr::Integer(0)));
                    }
                    other => panic!("unexpected expr: {other:?}"),
                }
            }
            other => panic!("unexpected action: {other:?}"),
        },
        other => panic!("unexpected branch: {other:?}"),
    }
}

#[test]
fn direct_ast_engine_converts_simple_rules() {
    let file = Parser::parse(
        r#"
        (map (m ("ka" "কা") ("k" "ক")))
        (state (init (m (shift init))))
        "#,
    )
    .unwrap();
    let mut engine = Engine::new(
        file.into_input_method(&mut Preprocessor::default())
            .unwrap(),
    );
    assert_eq!(engine.convert("ka"), "কা");
}

#[test]
fn direct_ast_engine_handles_multibyte_utf8_key_sequences() {
    // Regression test for the index-based cursor: keys and lookahead must
    // advance by byte length, not char count, or this panics/mis-slices on
    // multi-byte UTF-8 rule text.
    let file = Parser::parse(
        r#"
        (map (m ("ক" "X") ("কখ" "Y")))
        (state (init (m (shift init))))
        "#,
    )
    .unwrap();
    let mut engine = Engine::new(
        file.into_input_method(&mut Preprocessor::default())
            .unwrap(),
    );
    assert_eq!(engine.convert("কখ"), "Y");
    assert_eq!(engine.convert("ক"), "X");
}

#[test]
fn direct_ast_engine_pushback_requeues_generated_text_not_input_bytes() {
    // `pushback` re-queues characters from the *pre-edit* (generated)
    // buffer, which in general aren't a substring of the original input --
    // here typing "a" inserts "xy", then pushes "y" back to be re-matched
    // as though freshly typed, hitting the second rule.
    let file = Parser::parse(
        r#"
        (map (m ("a" (insert "xy") (pushback 1)) ("y" "Z")))
        (state (init (m (shift init))))
        "#,
    )
    .unwrap();
    let mut engine = Engine::new(
        file.into_input_method(&mut Preprocessor::default())
            .unwrap(),
    );
    assert_eq!(engine.convert("a"), "xZ");
}

#[test]
fn parses_bundled_bn_khipro_example_with_zero_unknown_forms() {
    let file = Parser::parse_from_path("examples/bn-khipro/bn-khipro.mim").unwrap();
    assert!(file.declaration.is_some());
    assert_eq!(file.unknown_forms.len(), 0);
    assert_eq!(file.maps.len(), 18);
    assert_eq!(file.states.len(), 7);
}

#[test]
fn direct_engine_matches_known_conversions_for_bundled_example() {
    let file = Parser::parse_from_path("examples/bn-khipro/bn-khipro.mim").unwrap();
    let cases = [
        ("tumi", "তুমি"),
        ("sob", "সব"),
        ("d;i", "দই"),
        ("amar", "আমার"),
        ("kalam", "কালাম"),
    ];
    let mut engine = Engine::new(
        file.into_input_method(&mut Preprocessor::default())
            .unwrap(),
    );
    for (input, expected) in cases {
        assert_eq!(engine.convert(input), expected, "converting {input:?}");
    }
}

#[test]
fn khipro_testcases() {
    let file = Parser::parse_from_path("examples/bn-khipro/bn-khipro.mim").unwrap();
    let mut engine = Engine::new(
        file.into_input_method(&mut Preprocessor::default())
            .unwrap(),
    );
    let cases = std::fs::read_to_string("examples/bn-khipro/testcases.tsv").unwrap();
    for (i, case) in cases.lines().enumerate().skip(1) {
        let (input, output) = case.split_once('\t').unwrap();
        assert_eq!(engine.convert(input), *output, "input #{i}: {input}");
    }
}
