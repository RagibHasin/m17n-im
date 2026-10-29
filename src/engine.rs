// SPDX-License-Identifier: MIT

//! A generic execution engine that runs directly against an [`InputMethod`].

use std::borrow::Cow;
use std::collections::HashMap;

use crate::ir::*;

#[derive(Debug, Clone)]
struct Snapshot<MarkerId, StateId, VarId> {
    state: StateId,
    prev_state: Option<StateId>,
    vars: HashMap<VarId, i32>,
    committed: String,
    preedit: Vec<char>,
    current_pos: usize,
    marks: HashMap<MarkerId, usize>,
}

/// The entry point for executing a m17n [`InputMethod`].
#[derive(Debug)]
pub struct Engine<IM: InputMethod + 'static> {
    input_method: IM,

    state: IM::StateId,
    prev_state: Option<IM::StateId>,
    vars: HashMap<IM::VarId, i32>,
    committed: String,
    preedit: Vec<char>,
    current_position: usize,
    marks: HashMap<IM::MarkerId, usize>,
    last_snapshot: Option<Snapshot<IM::MarkerId, IM::StateId, IM::VarId>>,
    /// Characters pushed back by `pushback` that still need (re-)matching, in the order
    /// they should be consumed. See module docs for why this can't just be a slice
    /// of the original input.
    requeue: String,
    unhandled: bool,
}

impl<IM: InputMethod> Engine<IM> {
    /// Instantiates a new [`Engine`] wrapping the given [`InputMethod`].
    pub fn new(input_method: IM) -> Self {
        let initial_state = input_method.initial_state();
        let mut engine = Self {
            input_method,
            state: initial_state,
            prev_state: None,
            vars: HashMap::new(),
            committed: String::new(),
            preedit: Vec::new(),
            current_position: 0,
            marks: HashMap::new(),
            last_snapshot: None,
            requeue: String::new(),
            unhandled: false,
        };
        engine.set_state(initial_state);
        engine
    }

    fn set_state(&mut self, state: IM::StateId) {
        self.state = state;
        if let Some(state) = self.input_method.get_state(state) {
            for b in state.branches {
                if let Branch::Init { actions } = b {
                    self.exec_all(actions);
                }
            }
        }
    }

    fn reset(&mut self) -> String {
        let committed = std::mem::take(&mut self.committed);

        self.prev_state = None;
        self.vars.clear();
        self.preedit.clear();
        self.current_position = 0;
        self.last_snapshot = None;
        self.requeue.clear();
        self.unhandled = false;

        self.set_state(self.input_method.initial_state());

        committed
    }

    /// Convert a full input string to output text, as if it were typed key by key,
    /// then flush any remaining pre-edit text.
    ///
    /// Resets the internal state of the engine to initial.
    ///
    /// It is intended to be used word-by-word, not with whole sentences, or longer texts.
    pub fn convert(&mut self, input: &str) -> String {
        let mut pos = 0;
        while !self.requeue.is_empty() || pos < input.len() {
            self.step(input, &mut pos);
        }
        self.commit_preedit();
        self.reset()
    }

    /// The upcoming text available for rule matching: `self.requeue` (almost always empty),
    /// followed by a bounded slice of `input[pos..]`.
    /// Zero-copy (`Cow::Borrowed`, a plain `&str` slice of `input`) whenever there's nothing queued,
    /// which is the common case.
    fn lookahead<'i>(&self, input: &'i str, pos: usize) -> Cow<'i, str> {
        let rest = &input[pos..];
        if self.requeue.is_empty() {
            Cow::Borrowed(rest)
        } else {
            let mut s = self.requeue.clone();
            s.push_str(rest);
            Cow::Owned(s)
        }
    }

    /// Consume `n` characters from the front of the upcoming text (draining `requeue` first,
    /// then advancing `pos` into `input`).
    fn advance(&mut self, pos: &mut usize, mut n: usize) {
        if !self.requeue.is_empty() && n > 0 {
            let take = self.requeue.len().min(n);
            self.requeue.drain(..take);
            n -= take;
        }
        *pos += n;
    }

    /// Consume and return exactly one character from the front of the
    /// upcoming text (`requeue` first, then `input`).
    fn next_char(&mut self, input: &str, pos: &mut usize) -> Option<char> {
        if let Some(c) = self.requeue.chars().next() {
            self.requeue.drain(..c.len_utf8());
            return Some(c);
        }
        let c = input[*pos..].chars().next()?;
        *pos += c.len_utf8();
        Some(c)
    }

    fn step(&mut self, input: &str, pos: &mut usize) {
        let Some(state) = self.input_method.get_state(self.state) else {
            if let Some(c) = self.next_char(input, pos) {
                self.preedit.push(c);
            }
            return;
        };

        let lookahead = self.lookahead(input, *pos);

        let mut best/*: Option<(&'a Rule, usize, &'a [Action])>*/ = None;
        for branch in state.branches {
            if let Branch::Map { map, actions } = branch
                && let Some(map) = self.input_method.get_map(*map)
            {
                for rule in map.text_rules {
                    if lookahead.starts_with(rule.keyseq) {
                        let len = rule.keyseq.len();
                        if best.is_none_or(|(_, best_len, _)| len > best_len) {
                            best = Some((rule, len, actions));
                        }
                    }
                }
            }
        }

        if let Some((rule, len, branch_actions)) = best {
            self.last_snapshot = Some(self.snapshot());
            self.current_position = self.preedit.len();
            // Consume the matched text *before* running its actions: a `pushback` inside those
            // actions mutates `self.requeue`, and must not be mistaken for text that was already
            // part of this match (that would either double-consume or -- as it happened in the past
            // -- never advance `pos` at all, looping forever on the same input).
            self.advance(pos, len);
            self.unhandled = false;
            self.exec_all(rule.actions);
            if !self.unhandled {
                self.exec_all(branch_actions);
            }
        } else {
            // Likewise: grab the one character being passed through before running
            // the `nil` branch's actions, so a `pushback` there can't get mistaken for it.
            let passthrough = self.next_char(input, pos);
            if let Some(Branch::Nil { actions }) = state
                .branches
                .iter()
                .find(|b| matches!(b, Branch::Nil { .. }))
            {
                self.last_snapshot = Some(self.snapshot());
                self.current_position = self.preedit.len();
                self.exec_all(actions);
            } else {
                self.last_snapshot = Some(self.snapshot());
                self.current_position = self.preedit.len();
                self.set_state(self.input_method.initial_state());
            }
            self.requeue.extend(passthrough);
        }
    }

    fn commit_preedit(&mut self) {
        self.committed.extend(self.preedit.iter());
        self.preedit.clear();
    }

    fn snapshot(&self) -> Snapshot<IM::MarkerId, IM::StateId, IM::VarId> {
        Snapshot {
            state: self.state,
            prev_state: self.prev_state,
            vars: self.vars.clone(),
            committed: self.committed.clone(),
            preedit: self.preedit.clone(),
            current_pos: self.current_position,
            marks: self.marks.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot<IM::MarkerId, IM::StateId, IM::VarId>) {
        let Snapshot {
            state,
            prev_state,
            vars,
            committed,
            preedit,
            current_pos: current_position,
            marks,
        } = s;
        self.state = state;
        self.prev_state = prev_state;
        self.vars = vars;
        self.committed = committed;
        self.preedit = preedit;
        self.current_position = current_position;
        self.marks = marks;
    }

    fn exec_all(&mut self, actions: &[Action<IM::MarkerId, IM::StateId, IM::VarId>]) {
        for a in actions {
            self.exec(a);
            if self.unhandled {
                break;
            }
        }
    }

    fn resolve_pos(&self, n: i32) -> usize {
        usize::try_from(n).unwrap_or_else(|_| self.preedit.len().saturating_add_signed(n as _))
    }

    fn delete(&mut self, other: usize) {
        let range = if other < self.current_position {
            other..self.current_position
        } else {
            self.current_position..other
        };
        self.marks.retain(|_, pos| !range.contains(pos));
        self.preedit.drain(range);
        self.current_position = self.current_position.min(other);
    }

    fn eval(&self, x: &Expr<IM::VarId>) -> i32 {
        match x {
            Expr::Integer(v) => *v,
            Expr::Variable(name) => self.vars.get(name).copied().unwrap_or(0),
            Expr::Op(op, args) => {
                let vals: Vec<i32> = args.iter().map(|a| self.eval(a)).collect();
                match op {
                    Operator::Add => vals.iter().sum(),
                    Operator::Sub => {
                        if vals.len() == 1 {
                            -vals[0]
                        } else {
                            vals.into_iter().reduce(|a, b| a - b).unwrap_or(0)
                        }
                    }
                    Operator::Mul => vals.iter().product(),
                    Operator::Div => vals
                        .into_iter()
                        .reduce(|a, b| if b != 0 { a / b } else { 0 })
                        .unwrap_or(0),
                    Operator::Or => vals.into_iter().reduce(|a, b| a | b).unwrap_or(0),
                    Operator::And => vals.into_iter().reduce(|a, b| a & b).unwrap_or(0),
                    Operator::Not => i32::from(vals.first().copied().unwrap_or(0) == 0),
                    Operator::Eq => i32::from(vals.windows(2).all(|w| w[0] == w[1])),
                    Operator::Lt => i32::from(vals.windows(2).all(|w| w[0] < w[1])),
                    Operator::Gt => i32::from(vals.windows(2).all(|w| w[0] > w[1])),
                    Operator::Le => i32::from(vals.windows(2).all(|w| w[0] <= w[1])),
                    Operator::Ge => i32::from(vals.windows(2).all(|w| w[0] >= w[1])),
                }
            }
        }
    }

    fn eval_condition(&self, cond: &Condition<IM::VarId>) -> bool {
        let left = self.eval(&cond.left);
        match (cond.op, &cond.right) {
            (Some(op), Some(right)) => {
                let r = self.eval(right);
                match op {
                    Operator::Eq => left == r,
                    Operator::Lt => left < r,
                    Operator::Gt => left > r,
                    Operator::Le => left <= r,
                    Operator::Ge => left >= r,
                    _ => false,
                }
            }
            _ => left != 0,
        }
    }

    fn marker_pos(&self, marker: &Marker<IM::MarkerId>) -> usize {
        match marker {
            Marker::Start => 0,
            Marker::MatchStart => self.current_position,
            Marker::End => self.preedit.len(),
            Marker::Prev => self.current_position.saturating_sub(1),
            Marker::Next => self.preedit.len().min(self.current_position + 1),
            Marker::RelPrev(r) => self.current_position.saturating_sub(*r),
            Marker::RelNext(r) => self.preedit.len().min(self.current_position + *r),
            Marker::Custom(id) => self.marks.get(id).copied().unwrap_or(self.current_position),
            Marker::Candidate(_) | Marker::GroupStart | Marker::GroupEnd | Marker::Selected => {
                unreachable!("{marker:?} is invalid for delete/move")
            }
        }
    }

    fn exec(&mut self, action: &Action<IM::MarkerId, IM::StateId, IM::VarId>) {
        match action {
            Action::InsertText(s) => {
                let old_len = self.preedit.len();
                self.preedit
                    .splice(self.current_position..self.current_position, s.chars());
                let n_added = self.preedit.len() - old_len;
                for pos in self.marks.values_mut() {
                    if *pos > self.current_position {
                        *pos += n_added;
                    }
                }
                self.current_position += n_added;
            }
            Action::InsertChar(c) => {
                self.preedit.insert(self.current_position, *c);
                for pos in self.marks.values_mut() {
                    if *pos > self.current_position {
                        *pos += 1;
                    }
                }
                self.current_position += 1;
            }
            Action::InsertVariable(id) => {
                let c = self.vars.get(id).copied().unwrap_or(0);
                if let Some(c) = char::from_u32(c as _) {
                    self.preedit.insert(self.current_position, c);
                    self.current_position += 1;
                }
            }
            Action::InsertCandidates(groups) => {
                // TODO: rethink this logic
                if let Some(g) = groups.first() {
                    let text = match g {
                        CandidateGroup::Text(s) => Some(s),
                        CandidateGroup::List(items) => items.first(),
                    };
                    self.preedit
                        .extend(text.into_iter().flat_map(|s| s.chars()));
                }
            }
            Action::DeleteMarker(m) => self.delete(self.marker_pos(m)),
            Action::DeletePosition(n) => self.delete(self.resolve_pos(*n)),
            Action::MoveMarker(marker) => self.current_position = self.marker_pos(marker),
            Action::MovePosition(n) => self.current_position = self.resolve_pos(*n),
            Action::Mark(id) => {
                self.marks.insert(*id, self.current_position);
            }
            Action::PushbackCount(n) => {
                let take = (*n).unsigned_abs() as usize;
                let start = self.preedit.len().saturating_sub(take);
                let moved: String = self.preedit.split_off(start).into_iter().collect();
                self.requeue.insert_str(0, &moved);
                self.current_position = self.current_position.min(self.preedit.len());
            }
            Action::Undo | Action::UndoCount(_) | Action::UndoVariable(_) => {
                if let Some(s) = self.last_snapshot.take() {
                    self.restore(s);
                }
            }
            Action::Commit => self.commit_preedit(),
            // TODO: rethink this logic
            Action::Unhandle | Action::Call => self.unhandled = true,
            Action::Shift(target) => {
                self.prev_state = Some(self.state);
                self.set_state(*target);
            }
            Action::ShiftPrevious => {
                if let Some(prev) = self.prev_state.take() {
                    self.prev_state = Some(self.state);
                    self.set_state(prev);
                }
            }
            Action::Set(cmd, name, expr) => {
                let x = self.eval(expr);
                let a = self.vars.get(name).copied().unwrap_or(0);
                let a = match cmd {
                    SetCmd::Set => x,
                    SetCmd::Add => a + x,
                    SetCmd::Sub => a - x,
                    SetCmd::Mul => a * x,
                    SetCmd::Div if x != 0 => a / x,
                    SetCmd::Div => a,
                };
                self.vars.insert(*name, a);
            }
            Action::If(cond, then_actions, else_actions) => {
                if self.eval_condition(cond) {
                    self.exec_all(then_actions);
                } else {
                    self.exec_all(else_actions);
                }
            }
            Action::Cond(clauses) => {
                for clause in *clauses {
                    if self.eval(&clause.test) != 0 {
                        self.exec_all(clause.actions);
                        break;
                    }
                }
            }
            // TODO: rethink this logic
            // no candidate window modeled; see module docs.
            Action::Show
            | Action::Hide
            | Action::PushbackKeySeq
            | Action::Pop
            | Action::SelectMarker(_)
            | Action::SelectIndex(_)
            | Action::SelectVariable(_) => {}
        }
    }
}
