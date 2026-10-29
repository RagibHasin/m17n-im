// SPDX-License-Identifier: MIT

//! Contains intermediate representation (IR) structures for [`InputMethod`]s.
//!
//! These are derived from the [AST](crate::ast) and used by [`Engine`](crate::Engine) when
//! executing an input method.

use std::fmt::Debug;

/// Identifier for a map, marker, state, or variable.
pub trait Id: Copy + Eq + Debug + 'static {}

/// Describes the executable set of data derived from a `.mim` file.
#[expect(clippy::type_complexity, reason = "false positive")]
pub trait InputMethod: Debug {
    /// Identifier for the maps.
    type MapId: Id;

    /// Identifier for the markers.
    type MarkerId: Id + std::hash::Hash + 'static;

    /// Identifier for the states.
    type StateId: Id;

    /// Identifier for the variables.
    type VarId: Id + std::hash::Hash + 'static;

    /// Retrieves the [`Map`] with the `id`.
    fn get_map(
        &self,
        id: Self::MapId,
    ) -> Option<Map<Self::MapId, Self::MarkerId, Self::StateId, Self::VarId>>;

    /// Retrieves the [`State`] with the `id`.
    fn get_state(
        &self,
        id: Self::StateId,
    ) -> Option<State<Self::MapId, Self::MarkerId, Self::StateId, Self::VarId>>;

    /// Describes the ID of the initial state.
    fn initial_state(&self) -> Self::StateId;
}

/// Markers refer to a position in a list/string often relative to an existing marked position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker<Id> {
    /// `@0` .. `@9` -- candidate-group markers.
    Candidate(u8),
    /// `@<` -- the beginning of the pre-edit text.
    Start,
    /// `@=` -- the start of the current match.
    MatchStart,
    /// `@>` -- the end of the pre-edit text.
    End,
    /// `@-` -- one before `@=`.
    Prev,
    /// `@+` -- one after `@=`.
    Next,
    /// `@[` -- the start of the previous candidate group.
    GroupStart,
    /// `@]` -- the end of the previous candidate group.
    GroupEnd,
    /// `@@` -- the currently selected candidate.
    Selected,
    /// `@-N` -- `N` positions before `@=`.
    RelPrev(usize),
    /// `@+N` -- `N` positions after `@=`.
    RelNext(usize),
    /// Custom marker
    Custom(Id),
}

/// See [`crate::ast::Expr`].
#[derive(Clone, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub enum Expr<VarId: Id> {
    Integer(i32),
    Variable(VarId),
    Op(Operator, &'static [Self]),
}

/// Operators.
///
/// These can operate on [`Expr`]essions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// +
    Add,
    /// -
    Sub,
    /// *, ×
    Mul,
    /// ÷, /
    Div,
    /// ∨, | (Logical OR)
    Or,
    /// ∧, & (Logical AND)
    And,
    /// ¬, ! (Logical NOT)
    Not,
    /// =
    Eq,
    /// <
    Lt,
    /// >
    Gt,
    /// ≤
    Le,
    /// ≥
    Ge,
}

/// See [`crate::ast::Condition`].
#[derive(Clone, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct Condition<VarId: Id> {
    pub op: Option<Operator>,
    pub left: Expr<VarId>,
    pub right: Option<Expr<VarId>>,
}

/// See [`crate::ast::CondClause`].
#[derive(Clone, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct CondClause<MarkerId: Id, StateId: Id, VarId: Id> {
    pub test: Expr<VarId>,
    pub actions: &'static [Action<MarkerId, StateId, VarId>],
}

/// Assignment with operation.
///
/// Sets or mutates a variable.
#[derive(Clone, Copy, Debug)]
pub enum SetCmd {
    /// =, Direct assignment.
    Set,
    /// +=
    Add,
    /// -=
    Sub,
    /// *=
    Mul,
    /// /=
    Div,
}

/// See [`crate::ast::CandidateGroup`].
#[derive(Debug, Clone)]
pub enum CandidateGroup {
    /// Each character of the text is one candidate.
    Text(&'static str),
    /// An explicit list of candidates.
    List(&'static [&'static str]),
}

/// See [`crate::ast::Action`].
#[derive(Clone, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub enum Action<MarkerId: Id, StateId: Id, VarId: Id> {
    InsertText(&'static str),
    InsertChar(char),
    InsertVariable(VarId),
    /// First candidate of each group is inserted; see module docs.
    InsertCandidates(&'static [CandidateGroup]),

    DeleteMarker(Marker<MarkerId>),
    DeletePosition(i32),

    SelectMarker(Marker<MarkerId>),
    SelectIndex(i32),
    SelectVariable(VarId),

    MoveMarker(Marker<MarkerId>),
    MovePosition(i32),

    /// `(mark NAME)` -- named markers collapse to the single tracked mark
    /// position, same as `@-`; see module docs.
    Mark(MarkerId),

    Show,
    Hide,

    PushbackCount(i32),
    PushbackKeySeq,

    Pop,

    Undo,
    UndoCount(i32),
    UndoVariable(VarId),

    Commit,
    Unhandle,

    /// `(shift STATE-NAME)` to a specific named state.
    Shift(StateId),
    /// `(shift t)` -- return to the previously active state.
    ShiftPrevious,

    /// `(call ...)` -- no host binding; always reports "unhandled".
    Call,

    Set(SetCmd, VarId, Expr<VarId>),

    If(Condition<VarId>, &'static [Self], &'static [Self]),
    Cond(&'static [CondClause<MarkerId, StateId, VarId>]),
}

/// Key modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyModifier {
    /// Shift (⇧)
    Shift,
    /// Control (^)
    Control,
    /// Meta
    Meta,
    /// Alt (⎇)
    Alt,
    /// Super
    Super,
    /// Hyper
    Hyper,
}

/// See [`crate::ast::KeyEvent`].
#[derive(Clone, Copy, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct KeyEvent {
    pub modifiers: &'static [KeyModifier],
    pub base: &'static str,
}

/// See [`crate::ast::Rule`] and [`crate::ast::KeySeq::Text`].
#[derive(Clone, Copy, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct TextRule<MarkerId: Id, StateId: Id, VarId: Id> {
    pub keyseq: &'static str,
    pub actions: &'static [Action<MarkerId, StateId, VarId>],
}

/// See [`crate::ast::Rule`] and [`crate::ast::KeySeq::Events`].
#[derive(Clone, Copy, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct EventRule<MarkerId: Id, StateId: Id, VarId: Id> {
    pub events: &'static [KeyEvent],
    pub actions: &'static [Action<MarkerId, StateId, VarId>],
}

/// See [`crate::ast::Map`].
#[derive(Clone, Copy, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct Map<MapId, MarkerId: Id, StateId: Id, VarId: Id> {
    pub id: MapId,
    pub text_rules: &'static [TextRule<MarkerId, StateId, VarId>],
    pub event_rules: &'static [EventRule<MarkerId, StateId, VarId>],
}

/// See [`crate::ast::Branch`].
#[derive(Clone, Copy, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub enum Branch<MapId, MarkerId: Id, StateId: Id, VarId: Id> {
    Map {
        map: MapId,
        actions: &'static [Action<MarkerId, StateId, VarId>],
    },
    Nil {
        actions: &'static [Action<MarkerId, StateId, VarId>],
    },
    Init {
        actions: &'static [Action<MarkerId, StateId, VarId>],
    },
}

/// See [`crate::ast::State`].
#[derive(Clone, Copy, Debug)]
#[expect(missing_docs, reason = "derivative type")]
pub struct State<MapId: Id, MarkerId: Id, StateId: Id, VarId: Id> {
    pub id: StateId,
    pub branches: &'static [Branch<MapId, MarkerId, StateId, VarId>],
}
