// SPDX-License-Identifier: MIT

//! Translates a parsed [`InputMethodFile`] into a Rust module.
//!
//! For more details see the [`InputMethodFile::generate`] method.
//!
//! [`InputMethodFile`]: crate::ast::InputMethodFile
//! [`InputMethodFile::generate`]: crate::ast::InputMethodFile::generate

use std::collections::{HashMap, HashSet};
use std::io;

use crate::ident::{Ident, Interner};
use crate::ir::{self, CandidateGroup, KeyEvent};
use crate::{ParsedInputMethod, preprocess::Preprocessor};

type Action = ir::Action<Ident, Ident, Ident>;
type Branch = ir::Branch<Ident, Ident, Ident, Ident>;
type Condition = ir::Condition<Ident>;
type CondClause = ir::CondClause<Ident, Ident, Ident>;
type EventRule = ir::EventRule<Ident, Ident, Ident>;
type Expr = ir::Expr<Ident>;
type Map = ir::Map<Ident, Ident, Ident, Ident>;
type Marker = ir::Marker<Ident>;
type State = ir::State<Ident, Ident, Ident, Ident>;
type TextRule = ir::TextRule<Ident, Ident, Ident>;

#[derive(Default, Clone)]
struct IdRegistry {
    order: Vec<Ident>,
    variant_name: HashMap<Ident, String>,
}

impl IdRegistry {
    fn add(&mut self, id: Ident, interner: &Interner) {
        if self.variant_name.contains_key(&id) {
            return;
        }
        let base = to_pascal_case(interner.resolve(id));
        let mut name = base.clone();
        let mut n = 2;
        let used: HashSet<&String> = self.variant_name.values().collect();
        while used.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        self.variant_name.insert(id, name);
        self.order.push(id);
    }

    fn variant(&self, id: Ident) -> &str {
        &self.variant_name[&id]
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if capitalize_next {
                out.extend(ch.to_uppercase());
                capitalize_next = false;
            } else {
                out.push(ch);
            }
        } else {
            capitalize_next = true;
        }
    }
    if out.is_empty() {
        out.push('X');
    }
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    out
}

struct Generator<'w, W> {
    interner: Interner,

    map_ids: IdRegistry,
    marker_ids: IdRegistry,
    state_ids: IdRegistry,
    var_ids: IdRegistry,

    writer: &'w mut W,
    carriage_returened: bool,
    indent: u8,
}

impl<W: io::Write> Generator<'_, W> {
    // MARK: Collectors

    fn collect_expr(&mut self, e: &Expr) {
        match e {
            Expr::Variable(id) => self.var_ids.add(*id, &self.interner),
            Expr::Op(_, args) => args.iter().for_each(|a| self.collect_expr(a)),
            Expr::Integer(_) => {}
        }
    }

    fn collect_action(&mut self, action: &Action) {
        match action {
            Action::InsertVariable(id) | Action::SelectVariable(id) | Action::UndoVariable(id) => {
                self.var_ids.add(*id, &self.interner);
            }
            Action::DeleteMarker(Marker::Custom(id))
            | Action::SelectMarker(Marker::Custom(id))
            | Action::MoveMarker(Marker::Custom(id)) => self.marker_ids.add(*id, &self.interner),
            Action::Set(_, id, e) => {
                self.var_ids.add(*id, &self.interner);
                self.collect_expr(e);
            }
            Action::If(cond, then, els) => {
                self.collect_expr(&cond.left);
                if let Some(r) = &cond.right {
                    self.collect_expr(r);
                }
                self.collect_actions(then);
                self.collect_actions(els);
            }
            Action::Cond(clauses) => {
                for clause in *clauses {
                    self.collect_expr(&clause.test);
                    self.collect_actions(clause.actions);
                }
            }

            Action::Shift(id) => self.state_ids.add(*id, &self.interner),
            _ => {}
        }
    }

    fn collect_actions(&mut self, actions: &[Action]) {
        for action in actions {
            self.collect_action(action);
        }
    }

    fn collect_state(&mut self, state: &State) {
        self.state_ids.add(state.id, &self.interner);
        for b in state.branches {
            match b {
                Branch::Map { actions, .. }
                | Branch::Nil { actions }
                | Branch::Init { actions } => {
                    self.collect_actions(actions);
                }
            }
            if let Branch::Map { map, .. } = b {
                self.map_ids.add(*map, &self.interner);
            }
        }
    }

    fn collect_map(&mut self, map: &Map) {
        self.map_ids.add(map.id, &self.interner);
        for actions in map
            .text_rules
            .iter()
            .map(|r| r.actions)
            .chain(map.event_rules.iter().map(|r| r.actions))
        {
            self.collect_actions(actions);
        }
    }

    // MARK: Writers
    // - All `write_{item}` methods end with `self.write`
    // - Parent is responsible for trailing comma
    // - Single item slice is written inlined

    fn write(&mut self, x: impl std::fmt::Display) -> io::Result<()> {
        if std::mem::take(&mut self.carriage_returened) {
            for _ in 0..self.indent {
                write!(self.writer, "    ")?;
            }
        }
        write!(self.writer, "{x}")
    }

    fn writeln(&mut self, l: impl std::fmt::Display) -> io::Result<()> {
        if self.carriage_returened {
            for _ in 0..self.indent {
                write!(self.writer, "    ")?;
            }
        }
        self.carriage_returened = true;
        writeln!(self.writer, "{l}")
    }

    /// Do NOT writeln at the end of `f`.
    fn write_opt<T>(
        &mut self,
        value: Option<&T>,
        f: impl FnOnce(&mut Self, &T) -> io::Result<()>,
    ) -> io::Result<()> {
        match value {
            Some(value) => {
                self.write("Some(")?;
                f(self, value)?;
                self.writeln(")")
            }
            None => self.writeln("None"),
        }
    }

    /// If `small` is `false`:
    ///   - DO writeln before calling this.
    ///   - DO writeln at the end of `f`.
    fn write_slice<T>(
        &mut self,
        slice: &[T],
        f: impl Fn(&mut Self, &T) -> io::Result<()>,
    ) -> io::Result<()> {
        self.write("&[")?;
        match slice.len() {
            0 => {}
            1 => f(self, &slice[0])?,
            _ => {
                self.writeln("")?;
                self.indent += 1;
                for item in slice {
                    f(self, item)?;
                    self.writeln(',')?;
                }
                self.indent -= 1;
            }
        }
        self.write(']')
    }

    fn write_enum(&mut self, reg: IdRegistry, name: &str) -> io::Result<()> {
        self.writeln("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")?;
        self.write(format_args!("pub enum {name} {{"))?;
        if !reg.order.is_empty() {
            self.writeln("")?;
        }
        self.indent += 1;
        for item in &reg.order {
            self.writeln(format_args!("{},", reg.variant(*item)))?;
        }
        self.indent -= 1;
        self.writeln('}')?;
        self.writeln(format_args!("impl Id for {name} {{}}"))
    }

    fn write_expr(&mut self, expr: &Expr) -> io::Result<()> {
        match expr {
            Expr::Integer(i) => self.write(format_args!("int({i})")),
            Expr::Variable(id) => self.write(format_args!(
                "var(VarId::{})",
                self.var_ids.variant(*id).to_owned()
            )),
            Expr::Op(op, args) => {
                self.write("op(")?;
                if is_simple(expr, 0) {
                    self.write(format_args!("{op:?}, &["))?;
                    match args.len() {
                        0 => {}
                        1 => self.write_expr(&args[0])?,
                        2 => {
                            self.write_expr(&args[0])?;
                            self.write(", ")?;
                            self.write_expr(&args[1])?;
                        }
                        _ => unreachable!(),
                    }
                    self.write(']')?;
                } else {
                    self.writeln("")?;
                    self.indent += 1;
                    self.writeln(format_args!("{op:?},"))?;
                    self.write_slice(args, Self::write_expr)?;
                    self.writeln(',')?;
                    self.indent -= 1;
                }
                self.write(')')
            }
        }
    }

    fn write_marker(&mut self, marker: &Marker) -> io::Result<()> {
        self.write(match marker {
            Marker::Candidate(i) => return self.write(format_args!("Marker::Candidate({i})")),
            Marker::Start => "Marker::Start",
            Marker::MatchStart => "Marker::MatchStart",
            Marker::End => "Marker::End",
            Marker::Prev => "Marker::Prev",
            Marker::Next => "Marker::Next",
            Marker::GroupStart => "Marker::GroupStart",
            Marker::GroupEnd => "Marker::GroupEnd",
            Marker::Selected => "Marker::Selected",
            Marker::RelPrev(n) => return self.write(format_args!("Marker::RelPrev({n})")),
            Marker::RelNext(n) => return self.write(format_args!("Marker::RelNext({n})")),
            Marker::Custom(id) => {
                return self.write(format_args!(
                    "Marker::Custom(MarkerId::{})",
                    self.marker_ids.variant(*id).to_owned()
                ));
            }
        })
    }

    fn write_action(&mut self, action: &Action) -> io::Result<()> {
        match action {
            Action::InsertText(s) => self.write(format_args!("Action::InsertText({s:?})")),
            Action::InsertChar(c) => self.write(format_args!("Action::InsertChar({c:?})")),
            Action::InsertVariable(id) => self.write(format_args!(
                "Action::InsertVariable(VarId::{})",
                self.var_ids.variant(*id).to_owned()
            )),
            Action::InsertCandidates(groups) => {
                self.write("Action::InsertCandidates(")?;
                self.write_slice(groups, Self::write_candidate_group)?;
                self.write(')')
            }
            Action::DeleteMarker(m) => {
                self.write("Action::DeleteMarker(")?;
                self.write_marker(m)?;
                self.write(')')
            }
            Action::DeletePosition(n) => self.write(format_args!("Action::DeletePosition({n})")),
            Action::MoveMarker(m) => {
                self.write("Action::MoveMarker(")?;
                self.write_marker(m)?;
                self.write(')')
            }
            Action::MovePosition(n) => self.write(format_args!("Action::MovePosition({n})")),
            Action::Mark(id) => self.write(format_args!(
                "Action::Mark(MarkerId::{})",
                self.marker_ids.variant(*id).to_owned()
            )),
            Action::Show => self.write("Action::Show"),
            Action::Hide => self.write("Action::Hide"),
            Action::PushbackCount(n) => self.write(format_args!("Action::PushbackCount({n})")),
            Action::PushbackKeySeq => self.write("Action::PushbackKeySeq"),
            Action::Pop => self.write("Action::Pop"),
            Action::Undo => self.write("Action::Undo"),
            Action::UndoVariable(id) => self.write(format_args!(
                "Action::UndoVariable(VarId::{})",
                self.var_ids.variant(*id).to_owned()
            )),
            Action::UndoCount(n) => self.write(format_args!("Action::UndoCount({n})")),
            Action::Commit => self.write("Action::Commit"),
            Action::Unhandle => self.write("Action::Unhandle"),
            Action::ShiftPrevious => self.write("Action::ShiftPrevious"),
            Action::Shift(id) => self.write(format_args!(
                "Action::Shift(StateId::{})",
                self.state_ids.variant(*id).to_owned()
            )),
            Action::Call => self.write("Action::Call"),
            Action::Set(cmd, id, expr) => {
                self.write(format_args!(
                    "Action::Set(SetCmd::{cmd:?}, VarId::{}, ",
                    self.var_ids.variant(*id).to_owned()
                ))?;
                self.write_expr(expr)?;
                self.write(')')
            }
            Action::If(condition, then, els) => {
                self.writeln("Action::If(")?;
                self.indent += 1;
                self.write_condition(condition)?;
                self.writeln(',')?;
                self.write_slice(then, Self::write_action)?;
                self.writeln(',')?;
                self.write_slice(els, Self::write_action)?;
                self.writeln(',')?;
                self.indent -= 1;
                self.write(')')
            }
            Action::Cond(clauses) => {
                self.write("Action::Cond(")?;
                self.write_slice(clauses, Self::write_clause)?;
                self.write(')')
            }
            Action::SelectMarker(m) => {
                self.write("Action::SelectMarker(")?;
                self.write_marker(m)?;
                self.write(')')
            }
            Action::SelectIndex(n) => self.write(format_args!("Action::SelectIndex({n})")),
            Action::SelectVariable(id) => self.write(format_args!(
                "Action::SelectVariable(VarId::{})",
                self.var_ids.variant(*id).to_owned()
            )),
        }
    }

    fn write_candidate_group(&mut self, group: &CandidateGroup) -> io::Result<()> {
        match group {
            CandidateGroup::Text(s) => self.writeln(format_args!("CandidateGroup::MText({s:?})")),
            CandidateGroup::List(items) => {
                self.writeln("CandidateGroup::List(")?;
                self.write_slice(items, |g, item| g.write(format_args!("{item:?}")))?;
                self.indent -= 1;
                self.write(')')
            }
        }
    }

    fn write_condition(&mut self, condition: &Condition) -> io::Result<()> {
        self.writeln("Condition {")?;
        self.indent += 1;
        self.writeln(format_args!("op: {:?},", condition.op))?;
        self.write("left: ")?;
        self.write_expr(&condition.left)?;
        self.writeln(',')?;
        self.write("right: ")?;
        self.write_opt(condition.right.as_ref(), Self::write_expr)?;
        self.writeln(',')?;
        self.indent -= 1;
        self.write('}')
    }

    fn write_clause(&mut self, clause: &CondClause) -> io::Result<()> {
        self.writeln("CondClause {")?;
        self.indent += 1;
        self.write("test: ")?;
        self.write_expr(&clause.test)?;
        self.writeln(',')?;
        self.write("actions: ")?;
        self.write_slice(clause.actions, Self::write_action)?;
        self.writeln(',')?;
        self.indent -= 1;
        self.write('}')
    }

    fn write_text_rule(&mut self, rule: &TextRule) -> io::Result<()> {
        self.write("text_rule(")?;
        if rule.actions.len() > 1 {
            self.writeln("")?;
            self.indent += 1;
            self.writeln(format_args!("{:?},", rule.keyseq))?;
            self.write_slice(rule.actions, Self::write_action)?;
            self.writeln(',')?;
            self.indent -= 1;
        } else {
            self.write(format_args!("{:?}, ", rule.keyseq))?;
            self.write_slice(rule.actions, Self::write_action)?;
        }
        self.write(')')
    }

    fn write_event_rule(&mut self, rule: &EventRule) -> io::Result<()> {
        self.writeln("event_rule(")?;
        self.indent += 1;
        self.write_slice(rule.events, Self::write_key_event)?;
        self.writeln(',')?;
        self.write_slice(rule.actions, Self::write_action)?;
        self.writeln(',')?;
        self.indent -= 1;
        self.write(')')
    }

    fn write_key_event(&mut self, event: &KeyEvent) -> io::Result<()> {
        self.write("key_event(")?;
        self.write_slice(event.modifiers, |g, m| g.write(format_args!("{m:?}")))?;
        self.write(format_args!(", {:?})", event.base))
    }

    fn write_map(&mut self, map: &Map) -> io::Result<()> {
        self.writeln("Map {")?;
        self.indent += 1;
        self.writeln(format_args!(
            "id: MapId::{},",
            self.map_ids.variant(map.id).to_owned()
        ))?;
        self.write("text_rules: ")?;
        self.write_slice(map.text_rules, Self::write_text_rule)?;
        self.writeln(',')?;
        self.write("event_rules: ")?;
        self.write_slice(map.event_rules, Self::write_event_rule)?;
        self.writeln(',')?;
        self.indent -= 1;
        self.write('}')
    }

    fn write_branch(&mut self, branch: &Branch) -> io::Result<()> {
        match branch {
            Branch::Map { map, actions } => {
                self.writeln("Branch::Map {")?;
                self.indent += 1;
                self.writeln(format_args!(
                    "map: MapId::{},",
                    self.map_ids.variant(*map).to_owned()
                ))?;
                self.write("actions: ")?;
                self.write_slice(actions, Self::write_action)?;
                self.writeln(',')?;
                self.indent -= 1;
            }
            Branch::Nil { actions } => {
                self.writeln("Branch::Nil {")?;
                self.indent += 1;
                self.write("actions: ")?;
                self.write_slice(actions, Self::write_action)?;
                self.writeln(',')?;
                self.indent -= 1;
            }
            Branch::Init { actions } => {
                self.writeln("Branch::Init {")?;
                self.indent += 1;
                self.write("actions: ")?;
                self.write_slice(actions, Self::write_action)?;
                self.writeln(',')?;
                self.indent -= 1;
            }
        }
        self.write('}')
    }

    fn write_state(&mut self, state: &State) -> io::Result<()> {
        self.writeln("State {")?;
        self.indent += 1;
        self.writeln(format_args!(
            "id: StateId::{},",
            self.state_ids.variant(state.id).to_owned()
        ))?;
        self.write("branches: ")?;
        self.write_slice(state.branches, Self::write_branch)?;
        self.writeln(',')?;
        self.indent -= 1;
        self.write('}')
    }
}

fn is_simple(expr: &Expr, depth: u8) -> bool {
    depth < 2
        && match expr {
            Expr::Integer(_) | Expr::Variable(_) => true,
            Expr::Op(_, exprs) => exprs.len() < 3 && exprs.iter().all(|e| is_simple(e, depth + 1)),
        }
}

// MARK: Template

impl crate::ast::InputMethodFile {
    /// Generates a Rust module and writes the content in the `writer`.
    ///
    /// The generated module would contain four enums: `MapId`, `MarkerId`, `StateId`, and `VarId`.
    /// These enums then would be used in two static slices `MAPS` and `STATES` describing
    /// the input method. Finally there would be a constant `INITIAL_STATE` of type `StateId` indicating
    /// the initial state of the constructed engine.
    ///
    /// Contents of the generated module is intended to be used with an [`EmbeddableInputMethod`].
    /// For more details, see the "examples/bn-khipro" crate.
    ///
    /// [`EmbeddableInputMethod`]: crate::EmbeddableInputMethod
    pub fn generate(
        mut self,
        preprocessor: &mut Preprocessor,
        writer: &mut impl io::Write,
    ) -> io::Result<ParsedInputMethod> {
        self.inline_macros(preprocessor);
        self.check_unsupported_actions(preprocessor);

        let interner = self.interner.clone();
        let input_method = self.into_input_method(preprocessor).unwrap();

        let mut generator = Generator {
            interner,
            map_ids: IdRegistry::default(),
            marker_ids: IdRegistry::default(),
            state_ids: IdRegistry::default(),
            var_ids: IdRegistry::default(),
            writer,
            carriage_returened: true,
            indent: 0,
        };
        for state in &input_method.states {
            generator.collect_state(state);
        }
        for map in &input_method.maps {
            generator.collect_map(map);
        }

        generator
            .writeln("// AUTO-GENERATED by `m17n-im`'s codegen module. Do not edit manually.")?;
        generator.writeln("")?;
        generator.writeln("#![cfg_attr(rustfmt, rustfmt_skip)]")?;
        generator.writeln("#![allow(clippy::upper_case_acronyms)]")?;
        generator.writeln("")?;
        generator.writeln("use m17n_im::__embed::*;")?;
        generator.writeln("")?;
        generator.write_enum(generator.map_ids.clone(), "MapId")?;
        generator.writeln("")?;
        generator.write_enum(generator.marker_ids.clone(), "MarkerId")?;
        generator.writeln("")?;
        generator.write_enum(generator.state_ids.clone(), "StateId")?;
        generator.writeln("")?;
        generator.write_enum(generator.var_ids.clone(), "VarId")?;
        generator.writeln("")?;
        generator.write("pub static MAPS: &[Map<MapId, MarkerId, StateId, VarId>] = ")?;
        generator.write_slice(&input_method.maps, Generator::write_map)?;
        generator.writeln(';')?;
        generator.writeln("")?;
        generator.write("pub static STATES: &[State<MapId, MarkerId, StateId, VarId>] = ")?;
        generator.write_slice(&input_method.states, Generator::write_state)?;
        generator.writeln(';')?;
        generator.writeln("")?;
        generator.writeln(format_args!(
            "pub const INITIAL_STATE: StateId = StateId::{};",
            generator
                .state_ids
                .variant(input_method.initial_state)
                .to_owned()
        ))?;
        generator.writeln("")?;
        generator.writeln(
            "pub const INPUT_METHOD: EmbeddableInputMethod<MapId, MarkerId, StateId, VarId> =",
        )?;
        generator.indent += 1;
        generator.writeln("EmbeddableInputMethod {")?;
        generator.indent += 1;
        generator.writeln("maps: MAPS,")?;
        generator.writeln("states: STATES,")?;
        generator.writeln("initial_state: INITIAL_STATE,")?;
        generator.indent -= 1;
        generator.writeln("};")?;
        generator.indent -= 1;
        generator.writeln("")?;
        generator.writeln(
            "pub type Engine = m17n_im::Engine<&'static EmbeddableInputMethod<MapId, MarkerId, StateId, VarId>>;",
        )?;
        generator.writeln("")?;
        generator.writeln("pub fn instantiate_engine() -> Engine {")?;
        generator.indent += 1;
        generator.writeln("Engine::new(&INPUT_METHOD)")?;
        generator.indent -= 1;
        generator.writeln('}')?;

        Ok(input_method)
    }
}
