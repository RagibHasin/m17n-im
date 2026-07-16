// SPDX-License-Identifier: MIT

//! Parses `.mim` source into [`crate::ast::InputMethodFile`].
//!
//! Start with [`Parser::parse`] or [`Parser::parse_from_path`].

use winnow::Parser as _;

use crate::ast::*;
use crate::ident::{Ident, Interner};
use crate::sexpr::{SExpr, file};

#[derive(Debug)]
/// An error originated from parsing or executing an m17n iniput method.
pub enum MimError {
    /// A syntax error.
    Syntax(String),
    /// A semantic error.
    Semantic(String),
}
impl std::error::Error for MimError {}
impl std::fmt::Display for MimError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(e) => write!(fmt, "syntax error:\n{e}"),
            Self::Semantic(e) => write!(fmt, "{e}"),
        }
    }
}

type MimResult<T> = Result<T, MimError>;

fn err<T>(msg: impl Into<String>) -> MimResult<T> {
    Err(MimError::Semantic(msg.into()))
}

fn is_action_keyword(s: &str) -> bool {
    [
        "insert", "delete", "select", "move", "mark", "show", "hide", "pushback", "pop", "undo",
        "commit", "unhandle", "shift", "call", "set", "add", "sub", "mul", "div", "if", "cond",
    ]
    .contains(&s)
}

fn parse_operator(s: &str) -> Option<Operator> {
    use Operator::*;
    Some(match s {
        "+" => Add,
        "-" => Sub,
        "*" => Mul,
        "/" => Div,
        "|" => Or,
        "&" => And,
        "!" => Not,
        "=" => Eq,
        "<" => Lt,
        ">" => Gt,
        "<=" => Le,
        ">=" => Ge,
        _ => return None,
    })
}

fn parse_rel_operator(s: &str) -> Option<Operator> {
    use Operator::*;
    match parse_operator(s)? {
        op @ (Eq | Lt | Gt | Le | Ge) => Some(op),
        _ => None,
    }
}

// MARK: Generic SExpr helpers

/// `crate::sexpr` tokenizes every bare atom, names, operators, and integer literals alike,
/// as [`SExpr::Symbol`], matching the grammar's own ambiguity between `SYMBOL` and `INTEGER` tokens.
/// This helper detects whether a given symbol's text is actually a (possibly negative) decimal
/// integer.
fn symbol_num(s: &str) -> Option<i32> {
    if s.is_empty() {
        return None;
    }
    s.parse().ok()
}

impl<'s> SExpr<'s> {
    /// The raw text of a `Symbol` node, or `None` for `Str`/`List`.
    fn symbol(&self) -> Option<&'s str> {
        match self {
            SExpr::Symbol(s) => Some(s),
            SExpr::Str(_) | SExpr::List(_) => None,
        }
    }

    fn str<'b>(&'b self) -> MimResult<&'s str>
    where
        'b: 's,
    {
        self.try_into()
    }

    fn children<'b: 's>(&'b self) -> MimResult<&'b [Self]> {
        self.try_into()
    }

    fn num(&self) -> MimResult<i32> {
        self.try_into()
    }

    fn head_symbol(&self) -> Option<&'s str> {
        match self {
            SExpr::List(items) => items.first().and_then(Self::symbol),
            _ => None,
        }
    }

    fn is_form_named(&self, name: &str) -> bool {
        self.head_symbol() == Some(name)
    }

    fn kind(&self) -> &'static str {
        match self {
            SExpr::Symbol(_) => "symbol",
            SExpr::Str(_) => "string",
            SExpr::List(_) => "list",
        }
    }

    fn head_is_nil(&self) -> bool {
        matches!(self, Self::Symbol(s) if *s == "nil")
    }

    /// A CANDIDATES group node is `MTEXT` or `(MTEXT MTEXT ...)`.
    fn looks_like_candidate_group(&self) -> bool {
        match self {
            SExpr::Str(_) => true,
            SExpr::List(items) => items.iter().all(|c| matches!(c, SExpr::Str(_))),
            _ => false,
        }
    }
}

impl<'a: 'i, 'i> TryFrom<&'a SExpr<'i>> for &'i str {
    type Error = MimError;

    fn try_from(x: &'a SExpr<'i>) -> Result<Self, Self::Error> {
        match x {
            SExpr::Str(s) => Ok(s),
            _ => err(format!("expected a string, got {}", x.kind())),
        }
    }
}

impl<'a: 'i, 'i> TryFrom<&'a SExpr<'i>> for &'a [SExpr<'i>] {
    type Error = MimError;

    fn try_from(x: &'a SExpr<'i>) -> Result<Self, Self::Error> {
        match x {
            SExpr::List(list) => Ok(list.as_slice()),
            _ => err(format!("expected a string, got {}", x.kind())),
        }
    }
}

impl<'a: 'i, 'i> TryFrom<&'a SExpr<'i>> for i32 {
    type Error = MimError;

    fn try_from(x: &'a SExpr<'i>) -> Result<Self, Self::Error> {
        match x {
            SExpr::Symbol(s) => s
                .parse()
                .map_err(|_| MimError::Semantic(format!("expected an integer, got symbol '{s}'"))),
            _ => err(format!("expected an integer, got {}", x.kind())),
        }
    }
}

// MARK: Parser

/// Entry point for `.mim` file parsing.
#[derive(Debug)]
pub struct Parser {
    interner: Interner,
}

impl Parser {
    /// Parse a `.mim` source strnig into an AST [`InputMethodFile`].
    pub fn parse(src: &str) -> MimResult<InputMethodFile> {
        let forms = file
            .parse(src)
            .map_err(|e| MimError::Syntax(e.to_string()))?;
        let mut parser = Self {
            interner: Interner::new(),
        };
        let mut file = InputMethodFile::default();
        for form in forms {
            parser.parse_top_level_form(&form, &mut file)?;
        }
        file.interner = parser.interner;
        Ok(file)
    }

    /// Parse `.mim` source from a file into an AST [`InputMethodFile`].
    pub fn parse_from_path(path: impl AsRef<std::path::Path>) -> MimResult<InputMethodFile> {
        let src = std::fs::read_to_string(path).map_err(|e| MimError::Semantic(e.to_string()))?;
        Self::parse(&src)
    }

    fn intern(&mut self, s: &str) -> Ident {
        self.interner.intern(s)
    }

    fn sym_ident(&mut self, e: &SExpr<'_>) -> MimResult<Ident> {
        e.symbol()
            .map(|s| self.intern(s))
            .ok_or_else(|| MimError::Semantic(format!("expected a symbol, got {}", e.kind())))
    }

    fn parse_top_level_form(
        &mut self,
        form: &SExpr<'_>,
        file: &mut InputMethodFile,
    ) -> MimResult<()> {
        match form.head_symbol() {
            Some("input-method") => file.declaration = Some(self.parse_im_declaration(form)?),
            Some("description") => file.description = Some(self.parse_im_description(form)?),
            Some("title") => file.title = Some(form.children()?[1].str()?.into()),
            Some("variable") => file.variables.extend(self.parse_variable_list(form)?),
            Some("command") => file.commands.extend(self.parse_command_list(form)?),
            Some("module") => file.modules.extend(self.parse_module_list(form)?),
            Some("macro") => file.macros.extend(self.parse_macro_list(form)?),
            Some("map") => file.maps.extend(self.parse_map_list(form)?),
            Some("state") => file.states.extend(self.parse_state_list(form)?),
            Some("include") => self.parse_top_level_include(form, file)?,
            _ => file.unknown_forms.push(form.to_string()),
        }
        Ok(())
    }

    fn parse_im_declaration(&mut self, form: &SExpr<'_>) -> MimResult<Declaration> {
        let [_, language, name, others @ ..] = form.children()? else {
            return err("(input-method ...) requires at least LANGUAGE and NAME");
        };
        let language = self.sym_ident(language)?;
        let name = self.sym_ident(name)?;
        let mut extra_id = None;
        let mut version = None;
        for k in others {
            match k {
                SExpr::Symbol(id) => extra_id = Some(self.intern(id)),
                SExpr::Str(_) => {}
                SExpr::List(list) => {
                    if let [head, v, ..] = list.as_slice()
                        && head.symbol() == Some("version")
                    {
                        version = Some(v.to_string());
                    }
                }
            }
            if extra_id.is_some() && version.is_some() {
                break;
            }
        }
        Ok(Declaration {
            language,
            name,
            extra_id,
            version,
        })
    }

    fn parse_im_description<'i>(&mut self, form: &'i SExpr<'i>) -> MimResult<Description> {
        self.read_description(&form.children()?[1..])
    }

    fn read_description<'i>(&mut self, items: &'i [SExpr<'i>]) -> MimResult<Description> {
        if items.is_empty() || items[0].head_is_nil() {
            return Ok(Description::default());
        }
        let first = &items[0];
        Ok(if first.is_form_named("_") {
            Description {
                text: Some(first.children()?[1].str()?.into()),
                gettext: true,
            }
        } else {
            Description {
                text: Some(first.str()?.into()),
                gettext: false,
            }
        })
    }

    fn parse_variable_list(&mut self, form: &SExpr<'_>) -> MimResult<Vec<Variable>> {
        form.children()?[1..]
            .iter()
            .map(|d| self.parse_variable_decl(d))
            .collect()
    }

    fn parse_variable_decl(&mut self, node: &SExpr<'_>) -> MimResult<Variable> {
        let [name, rest @ ..] = node.children()? else {
            return err("invalid variable declaration");
        };
        let name = self.sym_ident(name)?;
        if rest.is_empty() {
            return Ok(Variable {
                name,
                description: None,
                value: None,
                candidates: vec![],
            });
        }
        let description = Some(self.read_description(&rest[..1])?);
        let value = rest.get(1).map(|v| self.parse_value(v)).transpose()?;
        let candidates = rest[2..]
            .iter()
            .map(|c| self.parse_value_candidate(c))
            .collect::<MimResult<Vec<_>>>()?;
        Ok(Variable {
            name,
            description,
            value,
            candidates,
        })
    }

    fn parse_value_candidate(&mut self, n: &SExpr<'_>) -> MimResult<ValueCandidate> {
        Ok(
            if let SExpr::List(kids) = n
                && !n.head_symbol().map(is_action_keyword).unwrap_or(false)
                && kids.len() == 2
                && matches!(kids[0], SExpr::Symbol(_))
                && let (Ok(from), Ok(to)) = (kids[0].num(), kids[1].num())
            {
                ValueCandidate::Range(from, to)
            } else {
                ValueCandidate::Value(self.parse_value(n)?)
            },
        )
    }

    fn parse_value(&mut self, n: &SExpr<'_>) -> MimResult<Value> {
        match n {
            SExpr::Str(s) => Ok(Value::Text(s.to_string())),
            SExpr::Symbol(s) => Ok(s
                .parse()
                .map_or_else(|_| Value::Symbol(self.intern(s)), Value::Integer)),
            other => err(format!("cannot parse VALUE from {}", other.kind())),
        }
    }

    fn parse_command_list(&mut self, form: &SExpr<'_>) -> MimResult<Vec<Command>> {
        form.children()?[1..]
            .iter()
            .map(|d| self.parse_command_decl(d))
            .collect()
    }

    fn parse_command_decl(&mut self, node: &SExpr<'_>) -> MimResult<Command> {
        let kids = node.children()?;
        let name = self.sym_ident(&kids[0])?;
        let rest = &kids[1..];
        if rest.is_empty() {
            return Ok(Command {
                name,
                description: None,
                key_seqs: vec![],
            });
        }
        let description = Some(self.read_description(&rest[..1])?);
        let key_seqs = rest[1..]
            .iter()
            .map(|k| self.parse_keyseq(k))
            .collect::<MimResult<Vec<_>>>()?;
        Ok(Command {
            name,
            description,
            key_seqs,
        })
    }

    fn parse_module_list(&mut self, form: &SExpr<'_>) -> MimResult<Vec<Module>> {
        form.children()?[1..]
            .iter()
            .map(|m| -> MimResult<Module> {
                let kids = m.children()?;
                let name = self.sym_ident(&kids[0])?;
                let functions = kids[1..]
                    .iter()
                    .map(|f| self.sym_ident(f))
                    .collect::<MimResult<Vec<_>>>()?;
                Ok(Module { name, functions })
            })
            .collect()
    }

    fn parse_tags(&mut self, node: &SExpr<'_>) -> MimResult<Tags> {
        let kids = node.children()?;
        Ok(Tags {
            language: self.sym_ident(&kids[0])?,
            name: self.sym_ident(&kids[1])?,
            extra_id: match kids.get(2) {
                Some(k) => Some(self.sym_ident(k)?),
                None => None,
            },
        })
    }

    fn parse_macro_list(&mut self, form: &SExpr<'_>) -> MimResult<Vec<Macro>> {
        form.children()?[1..]
            .iter()
            .map(|m| -> MimResult<Macro> {
                let kids = m.children()?;
                let name = self.sym_ident(&kids[0])?;
                let actions = kids[1..]
                    .iter()
                    .map(|a| self.parse_action(a))
                    .collect::<MimResult<Vec<_>>>()?;
                Ok(Macro { name, actions })
            })
            .collect()
    }

    fn parse_map_list(&mut self, form: &SExpr<'_>) -> MimResult<Vec<Map>> {
        form.children()?[1..]
            .iter()
            .map(|m| -> MimResult<Map> {
                let kids = m.children()?;
                let name = self.sym_ident(&kids[0])?;
                let rules = kids[1..]
                    .iter()
                    .map(|r| self.parse_rule(r))
                    .collect::<MimResult<Vec<_>>>()?;
                Ok(Map { name, rules })
            })
            .collect()
    }

    fn parse_rule(&mut self, node: &SExpr<'_>) -> MimResult<Rule> {
        let kids = node.children()?;
        let key_seq = self.parse_keyseq(&kids[0])?;
        let actions = kids[1..]
            .iter()
            .map(|a| self.parse_action(a))
            .collect::<MimResult<Vec<_>>>()?;
        Ok(Rule { key_seq, actions })
    }

    fn parse_keyseq(&mut self, n: &SExpr<'_>) -> MimResult<KeySeq> {
        match n {
            SExpr::Str(_) => Ok(KeySeq::Text(n.str()?.into())),
            SExpr::List(items) => Ok(KeySeq::Events(
                items
                    .iter()
                    .map(|e| self.parse_key_event(e))
                    .collect::<MimResult<Vec<_>>>()?,
            )),
            other => err(format!("invalid KEYSEQ: {}", other.kind())),
        }
    }

    fn parse_key_event(&mut self, n: &SExpr<'_>) -> MimResult<KeyEvent> {
        match n {
            SExpr::Symbol(s) => Ok(
                if let Some(rest) = s.strip_prefix('?')
                    && let Some(c) = rest.chars().next()
                {
                    KeyEvent::Code(c as u32)
                } else if let Some(v) = symbol_num(s) {
                    KeyEvent::Code(v.try_into().map_err(|_| {
                        MimError::Semantic(format!("invalid code in key event: {v}\nmust be a u32"))
                    })?)
                } else {
                    self.parse_key_symbol(s)
                },
            ),
            other => err(format!("invalid key event: {}", other.kind())),
        }
    }

    fn parse_key_symbol(&self, raw: &str) -> KeyEvent {
        let mut modifiers = Vec::new();
        let mut rest = raw;
        loop {
            let m = ["S-", "C-", "M-", "A-", "s-", "H-"]
                .iter()
                .find(|p| rest.starts_with(**p));
            match m {
                Some(prefix) => {
                    modifiers.push(match *prefix {
                        "S-" => KeyModifier::Shift,
                        "C-" => KeyModifier::Control,
                        "M-" => KeyModifier::Meta,
                        "A-" => KeyModifier::Alt,
                        "s-" => KeyModifier::Super,
                        "H-" => KeyModifier::Hyper,
                        _ => unreachable!(),
                    });
                    rest = &rest[prefix.len()..];
                }
                None => break,
            }
        }
        KeyEvent::Symbol {
            modifiers,
            base: rest.to_string(),
        }
    }

    fn parse_state_list(&mut self, form: &SExpr<'_>) -> MimResult<Vec<State>> {
        form.children()?[1..]
            .iter()
            .map(|s| self.parse_state(s))
            .collect()
    }

    fn parse_state(&mut self, node: &SExpr<'_>) -> MimResult<State> {
        let kids = node.children()?;
        let name = self.sym_ident(&kids[0])?;
        let mut idx = 1;
        let mut title_text = None;
        if let Some(SExpr::Str(text)) = kids.get(idx) {
            title_text = Some(text.to_string());
            idx += 1;
        }
        let branches = kids[idx..]
            .iter()
            .map(|b| self.parse_branch(b))
            .collect::<MimResult<_>>()?;
        Ok(State {
            name,
            title_text,
            branches,
        })
    }

    fn parse_branch(&mut self, node: &SExpr<'_>) -> MimResult<Branch> {
        let kids = node.children()?;
        let head_text = kids[0]
            .symbol()
            .ok_or_else(|| MimError::Semantic("expected branch head symbol".into()))?;
        let actions = kids[1..]
            .iter()
            .map(|a| self.parse_action(a))
            .collect::<MimResult<Vec<_>>>()?;
        Ok(match head_text {
            "nil" => Branch::Nil { actions },
            "t" => Branch::Init { actions },
            _ => Branch::Map {
                map: self.intern(head_text),
                actions,
            },
        })
    }

    fn parse_top_level_include(
        &mut self,
        form: &SExpr<'_>,
        file: &mut InputMethodFile,
    ) -> MimResult<()> {
        let kids = &form.children()?[1..];
        let tags = self.parse_tags(&kids[0])?;
        let kind = kids[1].symbol().unwrap_or_default();
        let name = match kids.get(2) {
            Some(k) => Some(self.sym_ident(k)?),
            None => None,
        };
        let inclusion = Inclusion { tags, name };
        match kind {
            "macro" => file.macro_includes.push(inclusion),
            "map" => file.map_includes.push(inclusion),
            "state" => file.state_includes.push(inclusion),
            _ => file.unknown_forms.push(form.to_string()),
        }
        Ok(())
    }

    // -- actions --------------------------------------------------------

    fn parse_action(&mut self, n: &SExpr<'_>) -> MimResult<Action> {
        match n {
            SExpr::Str(_) => return Ok(Action::InsertText(n.str()?.into())),
            SExpr::Symbol(s) => {
                if let Some(v) = symbol_num(s) {
                    let c = v.try_into().map_err(|_| {
                        MimError::Semantic(format!("invalid character code in action: {v:X}"))
                    })?;
                    let c = char::from_u32(c).ok_or_else(|| {
                        MimError::Semantic(format!("invalid character code in action: {c:X}"))
                    })?;
                    return Ok(Action::InsertChar(c));
                }
                if let Some(rest) = s.strip_prefix('?')
                    && let Some(c) = rest.chars().next()
                {
                    return Ok(Action::InsertChar(c));
                }
                return Ok(Action::InsertVariable(self.intern(s)));
            }
            SExpr::List(_) => {}
        }

        let kids = n.children()?;
        if kids.is_empty() {
            return err("empty action list");
        }
        let head_text = kids[0].symbol();

        let is_keyword = head_text.map(is_action_keyword).unwrap_or(false);
        if !is_keyword {
            if let Some(h) = head_text
                && !kids[0].looks_like_candidate_group()
                && kids.len() == 1
            {
                return Ok(Action::MacroCall(self.intern(h)));
            }
            let groups = kids
                .iter()
                .map(|g| self.parse_candidate_group(g))
                .collect::<MimResult<Vec<_>>>()?;
            return Ok(Action::InsertCandidates(groups));
        }

        let head_text = head_text.unwrap();
        Ok(match head_text {
            "insert" => self.parse_insert_form(kids)?,
            "delete" => self.parse_delete_form(kids)?,
            "select" => self.parse_select_form(kids)?,
            "move" => self.parse_move_form(kids)?,
            "mark" => Action::Mark(self.sym_ident(&kids[1])?),
            "show" => Action::Show,
            "hide" => Action::Hide,
            "pushback" => self.parse_pushback_form(kids)?,
            "pop" => Action::Pop,
            "undo" => self.parse_undo_form(kids)?,
            "commit" => Action::Commit,
            "unhandle" => Action::Unhandle,
            "shift" => match kids[1].symbol().unwrap_or_default() {
                "t" => Action::ShiftPrevious,
                name => Action::Shift(self.intern(name)),
            },
            "call" => self.parse_call_form(kids)?,
            "set" | "add" | "sub" | "mul" | "div" => {
                let cmd = match head_text {
                    "set" => SetCmd::Set,
                    "add" => SetCmd::Add,
                    "sub" => SetCmd::Sub,
                    "mul" => SetCmd::Mul,
                    _ => SetCmd::Div,
                };
                let var = self.sym_ident(&kids[1])?;
                let expr = self.parse_expr(&kids[2])?;
                Action::Set(cmd, var, expr)
            }
            "if" => self.parse_if_form(kids)?,
            "cond" => self.parse_cond_form(kids)?,
            _ => unreachable!("unreachable action keyword: {head_text}"),
        })
    }

    fn parse_candidate_group(&mut self, n: &SExpr<'_>) -> MimResult<CandidateGroup> {
        match n {
            SExpr::Str(_) => Ok(CandidateGroup::Text(n.str()?.into())),
            SExpr::List(items) => Ok(CandidateGroup::List(
                items
                    .iter()
                    .map(|c| c.str().map(Into::into))
                    .collect::<MimResult<Vec<_>>>()?,
            )),
            other => err(format!("invalid CANDIDATES: {}", other.kind())),
        }
    }

    fn parse_insert_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let arg = &kids[1];
        Ok(match arg {
            SExpr::Str(_) => Action::InsertText(arg.str()?.into()),
            SExpr::Symbol(s) => {
                if let Some(v) = symbol_num(s) {
                    let c = v.try_into().map_err(|_| {
                        MimError::Semantic(format!("invalid character code in action: {v:X}"))
                    })?;
                    let c = char::from_u32(c).ok_or_else(|| {
                        MimError::Semantic(format!("invalid character code in action: {c:X}"))
                    })?;
                    Action::InsertChar(c)
                } else if let Some(rest) = s.strip_prefix('?') {
                    match rest.chars().next() {
                        Some(c) => Action::InsertChar(c),
                        None => Action::InsertVariable(self.intern(s)),
                    }
                } else {
                    Action::InsertVariable(self.intern(s))
                }
            }
            SExpr::List(items) => {
                let groups = items
                    .iter()
                    .map(|g| self.parse_candidate_group(g))
                    .collect::<MimResult<Vec<_>>>()?;
                Action::InsertCandidates(groups)
            }
        })
    }

    fn parse_marker(&mut self, x: &SExpr<'_>) -> MimResult<Marker> {
        let text = x.symbol().ok_or_else(|| {
            MimError::Semantic(format!("expected marker symbol, got {}", x.kind()))
        })?;
        let predefined = text.strip_prefix('@').and_then(|rest| {
            Some(if rest.len() == 1 {
                match rest {
                    "<" => Marker::Start,
                    "=" => Marker::MatchStart,
                    ">" => Marker::End,
                    "-" => Marker::Prev,
                    "+" => Marker::Next,
                    "[" => Marker::GroupStart,
                    "]" => Marker::GroupEnd,
                    "@" => Marker::Selected,
                    _ => return rest.parse().map(Marker::Candidate).ok(),
                }
            } else if let Some(Ok(v)) = rest.strip_prefix('-').map(str::parse) {
                Marker::RelPrev(v)
            } else if let Some(Ok(v)) = rest.strip_prefix('+').map(str::parse) {
                Marker::RelNext(v)
            } else {
                return None;
            })
        });

        Ok(predefined.unwrap_or_else(|| Marker::Custom(self.intern(text))))
    }

    fn parse_delete_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let arg = &kids[1];
        if let Ok(v) = arg.num() {
            return Ok(Action::DeletePosition(v));
        }
        Ok(Action::DeleteMarker(self.parse_marker(arg)?))
    }

    fn parse_select_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let arg = &kids[1];
        Ok(if let Ok(v) = arg.num() {
            Action::SelectIndex(v)
        } else if let Ok(m) = self.parse_marker(arg) {
            Action::SelectMarker(m)
        } else {
            Action::SelectVariable(self.sym_ident(arg)?)
        })
    }

    fn parse_move_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let arg = &kids[1];
        if let Ok(v) = arg.num() {
            return Ok(Action::MovePosition(v));
        }
        Ok(Action::MoveMarker(self.parse_marker(arg)?))
    }

    fn parse_pushback_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let arg = &kids[1];
        if let Ok(v) = arg.num() {
            return Ok(Action::PushbackCount(v));
        }
        Ok(Action::PushbackKeySeq(self.parse_keyseq(arg)?))
    }

    fn parse_undo_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        if kids.len() < 2 {
            return Ok(Action::Undo);
        }
        let arg = &kids[1];
        if let Ok(v) = arg.num() {
            return Ok(Action::UndoCount(v));
        }
        Ok(Action::UndoVariable(self.sym_ident(arg)?))
    }

    fn parse_call_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let module = self.sym_ident(&kids[1])?;
        let function = self.sym_ident(&kids[2])?;
        let args = kids[3..]
            .iter()
            .map(|a| self.parse_call_arg(a))
            .collect::<MimResult<Vec<_>>>()?;
        Ok(Action::Call {
            module,
            function,
            args,
        })
    }

    fn parse_call_arg(&mut self, n: &SExpr<'_>) -> MimResult<CallArg> {
        if let SExpr::List(_) = n {
            return Ok(CallArg::PList(n.to_string()));
        }
        Ok(CallArg::Value(self.parse_value(n)?))
    }

    fn parse_if_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let condition = self.parse_condition(&kids[1])?;
        let then = kids[2]
            .children()?
            .iter()
            .map(|a| self.parse_action(a))
            .collect::<MimResult<Vec<_>>>()?;
        let else_ = match kids.get(3) {
            Some(e) => e
                .children()?
                .iter()
                .map(|a| self.parse_action(a))
                .collect::<MimResult<Vec<_>>>()?,
            None => vec![],
        };
        Ok(Action::If(condition, then, else_))
    }

    fn parse_condition(&mut self, x: &SExpr<'_>) -> MimResult<Condition> {
        Ok(
            if let SExpr::List(args) = x
                && let [op, left, right] = args.as_slice()
            {
                Condition {
                    op: Some(op.symbol().and_then(parse_rel_operator).ok_or_else(|| {
                        MimError::Semantic(
                            "condition head must be an operator or expression".into(),
                        )
                    })?),
                    left: self.parse_expr(left)?,
                    right: Some(self.parse_expr(right)?),
                }
            } else {
                Condition {
                    op: None,
                    left: self.parse_expr(x)?,
                    right: None,
                }
            },
        )
    }

    fn parse_cond_form(&mut self, kids: &[SExpr<'_>]) -> MimResult<Action> {
        let clauses = kids[1..]
            .iter()
            .map(|c| -> MimResult<CondClause> {
                let ckids = c.children()?;
                let test = self.parse_expr(&ckids[0])?;
                let actions = ckids[1..]
                    .iter()
                    .map(|a| self.parse_action(a))
                    .collect::<MimResult<Vec<_>>>()?;
                Ok(CondClause { test, actions })
            })
            .collect::<MimResult<Vec<_>>>()?;
        Ok(Action::Cond(clauses))
    }

    fn parse_expr(&mut self, x: &SExpr<'_>) -> MimResult<Expr> {
        Ok(match x {
            SExpr::Symbol(s) => {
                if let Some(rest) = s.strip_prefix('?')
                    && let Some(c) = rest.chars().next()
                {
                    Expr::Integer(c as i32)
                } else if let Some(v) = symbol_num(s) {
                    Expr::Integer(v)
                } else {
                    Expr::Variable(self.intern(s))
                }
            }
            SExpr::List(kids) => {
                if kids.is_empty() {
                    return err("invalid EXPRESSION: ()");
                }
                let head = kids[0]
                    .symbol()
                    .ok_or_else(|| MimError::Semantic("invalid EXPRESSION operator".into()))?;
                let op = parse_operator(head).ok_or_else(|| {
                    MimError::Semantic(format!("invalid EXPRESSION operator: {head}"))
                })?;
                let args = kids[1..]
                    .iter()
                    .map(|a| self.parse_expr(a))
                    .collect::<MimResult<Vec<_>>>()?;
                Expr::Op(op, args)
            }
            other => return err(format!("invalid EXPRESSION: {}", other.kind())),
        })
    }
}
