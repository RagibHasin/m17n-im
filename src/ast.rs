// SPDX-License-Identifier: MIT

//! AST type definitions for the m17n input method (`.mim`) database format.
//!
//! Every identifier that the *grammar itself* defines, from a fixed, bounded vocabulary,
//! is an `enum`: [`SetCmd`], [`Operator`], [`KeyModifier`], [`Marker`].
//!
//! Every identifier that names something the *source file* defines (state names, map names,
//! variable names, macro names, marker names, module/function names, command names) is
//! an [`Ident`] rather than a string.

use crate::ir;

pub use crate::ident::{Ident, Interner};

/// Entry point of the AST.
///
/// Describes a `.mim` file in its entirety (sans comments).
#[derive(Debug, Default)]
pub struct InputMethodFile {
    /// Interner for the user-defined identifiers.
    pub interner: Interner,

    /// Declaration of the input method.
    pub declaration: Option<Declaration>,
    /// Description of the input method.
    pub description: Option<Description>,
    /// Title of the input method.
    pub title: Option<String>,

    /// Variables defined in this input method.
    pub variables: Vec<Variable>,
    /// Commands defined in this input method.
    pub commands: Vec<Command>,
    /// Modules defined in this input method.
    pub modules: Vec<Module>,
    /// Macros defined in this input method.
    pub macros: Vec<Macro>,
    /// Macro includes for this input method.
    pub macro_includes: Vec<Inclusion>,
    /// Maps defined in this input method.
    pub maps: Vec<Map>,
    /// Map includes for this input method.
    pub map_includes: Vec<Inclusion>,
    /// States defined in this input method.
    pub states: Vec<State>,
    /// State includes for this input method.
    pub state_includes: Vec<Inclusion>,

    /// Top-level forms that didn't match a known production, kept verbatim.
    pub unknown_forms: Vec<String>,
}

/// Input method declaration.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// Language the input method supports.
    pub language: Ident,
    /// Name of the input method.
    pub name: Ident,
    /// Extra identifier of the input method.
    pub extra_id: Option<Ident>,
    /// Version of the input method.
    pub version: Option<String>,
}

/// Input method declaration.
#[derive(Debug, Default, Clone)]
pub struct Description {
    /// `None` represents an explicit `nil` description.
    pub text: Option<String>,
    /// `gettext` internationalization.
    pub gettext: bool,
}

/// Inclusion link to other input methods (`.mim` files).
#[derive(Debug, Clone)]
pub struct Inclusion {
    /// Tags
    pub tags: Tags,
    /// The specific macro/map/state name to include, if given (otherwise: all).
    pub name: Option<Ident>,
}

/// Include directive tags.
#[derive(Debug, Clone)]
pub struct Tags {
    /// Language the input method supports.
    pub language: Ident,
    /// Name of the input method.
    pub name: Ident,
    /// Extra identifier of the input method.
    pub extra_id: Option<Ident>,
}

/// Variable declaration.
#[derive(Debug, Clone)]
pub struct Variable {
    /// Name of the variable.
    pub name: Ident,
    /// Description of the variable.
    pub description: Option<Description>,
    /// Initial value of the variable.
    pub value: Option<Value>,
    /// Possible value candidates (like a enum/refinement type) of the variable.
    pub candidates: Vec<ValueCandidate>,
}

/// Value of a variable.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Value of the variable defined by the symbol.
    Symbol(Ident),
    /// Integer value.
    Integer(i32),
    /// Text value.
    Text(String),
}

/// Possible candidates (like a enum/refinement type) of a variable.
#[derive(Debug, Clone)]
pub enum ValueCandidate {
    /// A single value.
    Value(Value),
    /// A range of values.
    Range(i32, i32),
}

/// Command declaration.
#[derive(Debug, Clone)]
pub struct Command {
    /// Name of the command.
    pub name: Ident,
    /// Description of the command.
    pub description: Option<Description>,
    /// Key sequences that invoke this command.
    pub key_seqs: Vec<KeySeq>,
}

/// Module declaration
#[derive(Debug, Clone)]
pub struct Module {
    /// Name of the module.
    pub name: Ident,
    /// Functions contained in this module.
    pub functions: Vec<Ident>,
}

/// Macro declaration.
#[derive(Debug, Clone)]
pub struct Macro {
    /// Name of the macro.
    pub name: Ident,
    /// List of actions that the macro executes.
    pub actions: Vec<Action>,
}

/// Map declaration.
#[derive(Debug, Clone)]
pub struct Map {
    /// Name of the map.
    pub name: Ident,
    /// Rules of this map.
    pub rules: Vec<Rule>,
}

/// State declaration.
#[derive(Debug, Clone)]
pub struct State {
    /// Name of the state.
    pub name: Ident,
    /// Title of the state, for display.
    pub title_text: Option<String>,
    /// Possible branches in this state.
    pub branches: Vec<Branch>,
}

/// Action, that is executed by an [`Engine`](crate::Engine).
#[derive(Debug, Clone)]
pub enum Action {
    /// Insert a string at the marked pre-edit position.
    InsertText(String),
    /// Insert a single character at the marked pre-edit position.
    InsertChar(char),
    /// Insert a single character from this variable at the marked pre-edit position.
    InsertVariable(Ident),
    /// Insert a list of candidates at the marked pre-edit position.
    InsertCandidates(Vec<CandidateGroup>),

    /// Delete the characters between the marker and the currently marked position
    /// from the pre-edit buffer.
    DeleteMarker(Marker),
    /// Delete the given number of characters at the marked position from the pre-edit buffer.
    /// Positive for characters after the mark, negative for before.
    DeletePosition(i32),

    /// Select the candidate described by the marker.
    SelectMarker(Marker),
    /// Select the candidate at the index.
    SelectIndex(i32),
    /// Select the candidate at the index value from the variable.
    SelectVariable(Ident),

    /// Set the marked position of the pre-edit buffer to the marker's described position.
    MoveMarker(Marker),
    /// Set the marked position of the pre-edit buffer to the index.
    /// Negative value counts in the reverse direction.
    MovePosition(i32),

    /// Set the marked position of the pre-edit buffer to the index value from the variable.
    Mark(Ident),

    /// Show the candidate list.
    Show,
    /// Hide the candidate list.
    Hide,

    /// Push the given number of characters back into the input buffer.
    PushbackCount(i32),
    /// Push the key sequence into the input buffer.
    PushbackKeySeq(KeySeq),

    /// Pop the first unhandled key event from input buffer.
    Pop,

    /// Undo the last two (this undo invocation and the one before) actions.
    Undo,
    /// If the given number is positive, undo the event from the given index to the last one.
    /// If the given number is negative, undo events from the end upto the count.
    UndoCount(i32),
    /// Undo similarly to [`UndoCount`](Self::UndoCount) with the count value from the variable.
    UndoVariable(Ident),

    /// Commit the content of pre-edit buffer.
    Commit,
    /// Commit the content of pre-edit buffer, and mark this invocation as unhandled.
    Unhandle,

    /// Shift to a specific named state.
    Shift(Ident),
    /// Shift to the previously active state.
    ShiftPrevious,

    /// Call a function.
    Call {
        /// Module the called function resides in.
        module: Ident,
        /// Name of the function to be called.
        function: Ident,
        /// Arguments of the function call.
        args: Vec<CallArg>,
    },

    /// Set a variable to the value of the expression.
    Set(SetCmd, Ident, Expr),

    /// If-Else conditional action.
    If(Condition, Vec<Self>, Vec<Self>),
    /// Switch/match conditional action.
    ///
    /// Executes the first truthy clause.
    Cond(Vec<CondClause>),

    /// Macro invocation.
    ///
    /// It is inlined in preprocessing pass.
    MacroCall(Ident),
}

/// Rule of a map.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Key sequence to rule matches to.
    pub key_seq: KeySeq,
    /// Actions for executing when the rule matches.
    pub actions: Vec<Action>,
}

/// Key sequence.
#[derive(Debug, Clone)]
pub enum KeySeq {
    /// Text, the result of raw key events interpreted by the keyboard.
    Text(String),
    /// Raw key events.
    Events(Vec<KeyEvent>),
}

/// Key event.
#[derive(Debug, Clone)]
pub enum KeyEvent {
    /// Raw key code.
    Code(u32),
    /// Structured symbolic key.
    Symbol {
        /// Modifiers on the key.
        modifiers: Vec<KeyModifier>,
        /// Name of the key.
        base: String,
    },
}

#[doc(inline)]
pub use ir::KeyModifier;

/// Branch of a state.
#[derive(Debug, Clone)]
pub enum Branch {
    /// Tries matching the input to the rules of this map.
    Map {
        /// Name of the map.
        map: Ident,
        /// Actions to be executed.
        actions: Vec<Action>,
    },
    /// Executes when no other match is found.
    Nil {
        /// Actions to be executed.
        actions: Vec<Action>,
    },
    /// Executes when the state is entered.
    Init {
        /// Actions to be executed.
        actions: Vec<Action>,
    },
}

/// [`Marker`](ir::Marker) with custom ID set to [`Ident`].
pub type Marker = ir::Marker<Ident>;

/// Group of candidates.
#[derive(Debug, Clone)]
pub enum CandidateGroup {
    /// Each character of the text is one candidate.
    Text(String),
    /// An explicit list of candidates.
    List(Vec<String>),
}

/// Argument of a function call.
#[derive(Debug, Clone)]
pub enum CallArg {
    /// A value argument.
    Value(Value),
    /// Raw nested-plist argument; plist syntax belongs to the general
    /// database format, not the input-method grammar, so it's kept as-is.
    PList(String),
}

#[doc(inline)]
pub use ir::{Operator, SetCmd};

/// Mathematical expression involving variables, constants, and operators.
#[derive(Clone, Debug)]
pub enum Expr {
    /// A constant expression.
    Integer(i32),
    /// A variable expression.
    Variable(Ident),
    /// An operation on nested expressions.
    Op(Operator, Vec<Self>),
}

/// Conditional expression, evaluates into `true`/`false`.
#[derive(Clone, Debug)]
pub struct Condition {
    /// Relation between `left` and `right`.
    pub op: Option<Operator>,
    /// Left-hand-side expression.
    ///
    /// It evaluating to non-zero makes the condition `true` if `op` and `right` are `None`.
    pub left: Expr,
    /// Right-hand-side expression.
    pub right: Option<Expr>,
}

/// Conditional clause.
#[derive(Debug, Clone)]
pub struct CondClause {
    /// The test expression.
    ///
    /// If it evaluates to non-zero, the actions are executed.
    pub test: Expr,
    /// Actions to be executed.
    pub actions: Vec<Action>,
}
