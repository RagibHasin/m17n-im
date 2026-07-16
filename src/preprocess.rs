// SPDX-License-Identifier: MIT

use crate::ast::{Action, Branch, CondClause, InputMethodFile, Macro};

/// Configuration and output of a preprocess pass e.g. inlining macros, checking for
/// unupported actions, etc.
#[derive(Debug)]
pub struct Preprocessor {
    /// Maximum depth allowed for macros.
    pub max_depth: u8,
    /// Warnings generated while preprocessing.
    pub warnings: Vec<PreprocessorWarning>,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self {
            max_depth: u8::MAX,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
/// A warning generated while preprocessing.
pub enum PreprocessorWarning {
    /// A macro reached maximum allowed depth.
    MaxDepthReached,
    /// A macro was not found.
    UndefinedMacro,
    /// A `select` action was not encountered, which is unsupported.
    UnsupportedSelect,
    /// A `call` action was not encountered, which is unsupported.
    UnsupportedCall,
    /// A `push` action was not encountered, which is unsupported.
    UnsupportedPushback,
}

impl InputMethodFile {
    /// Inline all macros into their call sites.
    pub fn inline_macros(&mut self, preprocessor: &mut Preprocessor) {
        for map in &mut self.maps {
            for rule in &mut map.rules {
                rule.actions = inline_actions(&rule.actions, &self.macros, 0, preprocessor);
            }
        }
        for state in &mut self.states {
            for branch in &mut state.branches {
                match branch {
                    Branch::Map { actions, .. }
                    | Branch::Nil { actions }
                    | Branch::Init { actions } => {
                        *actions = inline_actions(actions, &self.macros, 0, preprocessor);
                    }
                }
            }
        }
    }

    /// Checks for unsupported actions.
    pub fn check_unsupported_actions(&self, preprocessor: &mut Preprocessor) {
        for state in &self.states {
            for branch in &state.branches {
                match branch {
                    Branch::Map { actions, .. }
                    | Branch::Nil { actions }
                    | Branch::Init { actions } => {
                        for action in actions {
                            warn_unsupported(action, &mut preprocessor.warnings);
                        }
                    }
                }
            }
        }
        for map in &self.maps {
            for rule in &map.rules {
                for action in &rule.actions {
                    warn_unsupported(action, &mut preprocessor.warnings);
                }
            }
        }
    }
}

fn inline_actions(
    actions: &[Action],
    macros: &[Macro],
    depth: u8,
    preprocessor: &mut Preprocessor,
) -> Vec<Action> {
    let mut out = Vec::with_capacity(actions.len());
    for a in actions {
        inline_action(a, macros, depth, preprocessor, &mut out);
    }
    out
}

fn inline_action(
    a: &Action,
    macros: &[Macro],
    depth: u8,
    preprocessor: &mut Preprocessor,
    out: &mut Vec<Action>,
) {
    match a {
        Action::MacroCall(name) => {
            if depth > preprocessor.max_depth {
                preprocessor
                    .warnings
                    .push(PreprocessorWarning::MaxDepthReached);
                return;
            }
            match macros.iter().find(|m| m.name == *name) {
                Some(body) => out.extend(inline_actions(
                    &body.actions,
                    macros,
                    depth + 1,
                    preprocessor,
                )),
                None => preprocessor
                    .warnings
                    .push(PreprocessorWarning::UndefinedMacro),
            }
        }
        Action::If(cond, then, els) => out.push(Action::If(
            cond.clone(),
            inline_actions(then, macros, depth, preprocessor),
            inline_actions(els, macros, depth, preprocessor),
        )),
        Action::Cond(clauses) => out.push(Action::Cond(
            clauses
                .iter()
                .map(|c| CondClause {
                    test: c.test.clone(),
                    actions: inline_actions(&c.actions, macros, depth, preprocessor),
                })
                .collect(),
        )),
        other => out.push(other.clone()),
    }
}

fn warn_unsupported(action: &Action, warnings: &mut Vec<PreprocessorWarning>) {
    match action {
        Action::SelectMarker(_) | Action::SelectIndex(_) | Action::SelectVariable(_) => {
            warnings.push(PreprocessorWarning::UnsupportedSelect);
        }
        Action::Call { .. } => warnings.push(PreprocessorWarning::UnsupportedCall),
        Action::PushbackKeySeq(_) => warnings.push(PreprocessorWarning::UnsupportedPushback),
        Action::If(_, then, els) => {
            then.iter().for_each(|a| warn_unsupported(a, warnings));
            els.iter().for_each(|a| warn_unsupported(a, warnings));
        }
        Action::Cond(clauses) => clauses
            .iter()
            .for_each(|c| c.actions.iter().for_each(|a| warn_unsupported(a, warnings))),
        _ => {}
    }
}
