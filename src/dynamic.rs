// SPDX-License-Identifier: MIT

use crate::{MimError, ast, ident::Ident, ir::*, preprocess::Preprocessor};

/// An [`InputMethod`] that holds macro inlined intermediate representation (IR) of a `.mim` file.
///
/// This is a leaky handle, as generating this leaks strings held by the AST and the generated
/// `Vec`s of the IR structs.
#[derive(Debug)]
pub struct ParsedInputMethod {
    pub(crate) maps: Vec<Map<Ident, Ident, Ident, Ident>>,
    pub(crate) states: Vec<State<Ident, Ident, Ident, Ident>>,
    pub(crate) initial_state: Ident,
}

impl InputMethod for ParsedInputMethod {
    type MapId = Ident;
    type MarkerId = Ident;
    type StateId = Ident;
    type VarId = Ident;

    fn get_map(
        &self,
        id: Self::MapId,
    ) -> Option<Map<Self::MapId, Self::MarkerId, Self::StateId, Self::VarId>> {
        self.maps.iter().find(|m| m.id == id).copied()
    }

    fn get_state(
        &self,
        id: Self::StateId,
    ) -> Option<State<Self::MapId, Self::MarkerId, Self::StateId, Self::VarId>> {
        self.states.iter().find(|s| s.id == id).copied()
    }

    fn initial_state(&self) -> Self::StateId {
        self.initial_state
    }
}

impl ast::InputMethodFile {
    /// Inlines the macros and generates a [`ParsedInputMethod`].
    pub fn into_input_method(
        mut self,
        preprocessor: &mut Preprocessor,
    ) -> Result<ParsedInputMethod, MimError> {
        self.inline_macros(preprocessor);

        let initial_state = self
            .states
            .iter()
            .find(|s| self.interner.resolve(s.name) == "init")
            .or_else(|| self.states.first())
            .map(|s| s.name)
            .ok_or_else(|| MimError::Semantic("no STATE defined".into()))?;

        let maps = self.maps.into_iter().map(Into::into).collect();
        let states = self.states.into_iter().map(Into::into).collect();

        Ok(ParsedInputMethod {
            maps,
            states,
            initial_state,
        })
    }
}

impl From<ast::Map> for Map<Ident, Ident, Ident, Ident> {
    fn from(map: ast::Map) -> Self {
        let mut text_rules = Vec::new();
        let mut event_rules = Vec::new();
        for rule in map.rules {
            match rule.key_seq {
                ast::KeySeq::Text(keyseq) => text_rules.push(TextRule {
                    keyseq: keyseq.leak(),
                    actions: map_and_leak(rule.actions),
                }),
                ast::KeySeq::Events(events) => event_rules.push(EventRule {
                    events: events
                        .into_iter()
                        .flat_map(Option::<_>::from)
                        .collect::<Vec<_>>()
                        .leak(),
                    actions: map_and_leak(rule.actions),
                }),
            }
        }
        Self {
            id: map.name,
            text_rules: text_rules.leak(),
            event_rules: event_rules.leak(),
        }
    }
}

impl From<ast::Action> for Action<Ident, Ident, Ident> {
    fn from(action: ast::Action) -> Self {
        match action {
            ast::Action::InsertText(s) => Self::InsertText(s.leak()),
            ast::Action::InsertChar(c) => Self::InsertChar(c),
            ast::Action::InsertVariable(id) => Self::InsertVariable(id),
            ast::Action::InsertCandidates(groups) => Self::InsertCandidates(map_and_leak(groups)),
            ast::Action::DeleteMarker(m) => Self::DeleteMarker(m),
            ast::Action::DeletePosition(n) => Self::DeletePosition(n),
            ast::Action::MoveMarker(m) => Self::MoveMarker(m),
            ast::Action::MovePosition(n) => Self::MovePosition(n),
            ast::Action::Mark(id) => Self::Mark(id),
            ast::Action::Show => Self::Show,
            ast::Action::Hide => Self::Hide,
            ast::Action::PushbackCount(n) => Self::PushbackCount(n),
            ast::Action::PushbackKeySeq(_) => Self::PushbackKeySeq,
            ast::Action::Pop => Self::Pop,
            ast::Action::Undo | ast::Action::UndoVariable(_) => Self::Undo,
            ast::Action::UndoCount(n) => Self::UndoCount(n),
            ast::Action::Commit => Self::Commit,
            ast::Action::Unhandle => Self::Unhandle,
            ast::Action::ShiftPrevious => Self::ShiftPrevious,
            ast::Action::Shift(id) => Self::Shift(id),
            ast::Action::Call { .. } => Self::Call,
            ast::Action::Set(cmd, id, expr) => Self::Set(cmd, id, expr.into()),
            ast::Action::If(cond, then, els) => {
                Self::If(cond.into(), map_and_leak(then), map_and_leak(els))
            }
            ast::Action::Cond(clauses) => Self::Cond(map_and_leak(clauses)),
            ast::Action::SelectMarker(m) => Self::SelectMarker(m),
            ast::Action::SelectIndex(n) => Self::SelectIndex(n),
            ast::Action::SelectVariable(id) => Self::SelectVariable(id),
            ast::Action::MacroCall(_) => unreachable!("macro calls are inlined before rendering"),
        }
    }
}

impl From<ast::CandidateGroup> for CandidateGroup {
    fn from(group: ast::CandidateGroup) -> Self {
        match group {
            ast::CandidateGroup::Text(s) => Self::Text(s.leak()),
            ast::CandidateGroup::List(items) => Self::List(
                items
                    .into_iter()
                    .map(|s| s.leak() as &'static _)
                    .collect::<Vec<_>>()
                    .leak(),
            ),
        }
    }
}

impl From<ast::Expr> for Expr<Ident> {
    fn from(expr: ast::Expr) -> Self {
        match expr {
            ast::Expr::Integer(i) => Self::Integer(i),
            ast::Expr::Variable(id) => Self::Variable(id),
            ast::Expr::Op(op, exprs) => Self::Op(op, map_and_leak(exprs)),
        }
    }
}

impl From<ast::Condition> for Condition<Ident> {
    fn from(cond: ast::Condition) -> Self {
        Self {
            op: cond.op,
            left: cond.left.into(),
            right: cond.right.map(Into::into),
        }
    }
}

impl From<ast::CondClause> for CondClause<Ident, Ident, Ident> {
    fn from(clause: ast::CondClause) -> Self {
        Self {
            test: clause.test.into(),
            actions: map_and_leak(clause.actions),
        }
    }
}

impl From<ast::KeyEvent> for Option<KeyEvent> {
    fn from(event: ast::KeyEvent) -> Self {
        match event {
            ast::KeyEvent::Code(_) => None,
            ast::KeyEvent::Symbol { modifiers, base } => Some(KeyEvent {
                modifiers: map_and_leak(modifiers),
                base: base.leak(),
            }),
        }
    }
}

impl From<ast::State> for State<Ident, Ident, Ident, Ident> {
    fn from(state: ast::State) -> Self {
        Self {
            id: state.name,
            branches: map_and_leak(state.branches),
        }
    }
}

impl From<ast::Branch> for Branch<Ident, Ident, Ident, Ident> {
    fn from(branch: ast::Branch) -> Self {
        match branch {
            ast::Branch::Map { map, actions } => Self::Map {
                map,
                actions: map_and_leak(actions),
            },
            ast::Branch::Nil { actions } => Self::Nil {
                actions: map_and_leak(actions),
            },
            ast::Branch::Init { actions } => Self::Init {
                actions: map_and_leak(actions),
            },
        }
    }
}

fn map_and_leak<T, U: From<T>>(from: Vec<T>) -> &'static [U] {
    from.into_iter().map(Into::into).collect::<Vec<_>>().leak()
}
