use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::vec::IntoIter;

use crate::completion::Completer;
use crate::config::{Config, EditMode};
use crate::edit::init_state;
use crate::highlight::Highlighter;
use crate::hint::Hinter;
use crate::keymap::{Cmd, InputState};
use crate::keys::KeyPress;
use crate::tty::Sink;
use crate::validate::Validator;
use crate::{Context, Editor, Helper, Result};

mod common;
mod emacs;
mod history;
mod vi_cmd;
mod vi_insert;

fn init_editor(mode: EditMode, keys: &[KeyPress]) -> Editor<()> {
    let config = Config::builder().edit_mode(mode).build();
    let mut editor = Editor::<()>::with_config(config);
    editor.term.keys.extend(keys.iter().cloned());
    editor
}

struct SimpleCompleter;
impl Completer for SimpleCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<String>)> {
        Ok((0, vec![line.to_owned() + "t"]))
    }
}

impl Helper for SimpleCompleter {}
impl Hinter for SimpleCompleter {}
impl Highlighter for SimpleCompleter {}
impl Validator for SimpleCompleter {}

#[test]
fn complete_line() {
    let mut out = Sink::new();
    let history = crate::history::History::new();
    let helper = Some(SimpleCompleter);
    let mut s = init_state(&mut out, "rus", 3, helper.as_ref(), &history);
    let config = Config::default();
    let mut input_state = InputState::new(&config, Arc::new(RwLock::new(HashMap::new())));
    let keys = vec![KeyPress::Enter];
    let mut rdr: IntoIter<KeyPress> = keys.into_iter();
    let cmd = super::complete_line(&mut rdr, &mut s, &mut input_state, &Config::default()).unwrap();
    assert_eq!(Some(Cmd::AcceptLine), cmd);
    assert_eq!("rust", s.line.as_str());
    assert_eq!(4, s.line.pos());
}

// `keys`: keys to press
// `expected_line`: line after enter key
fn assert_line(mode: EditMode, keys: &[KeyPress], expected_line: &str) {
    let mut editor = init_editor(mode, keys);
    let actual_line = editor.readline(">>").unwrap();
    assert_eq!(expected_line, actual_line);
}

// `initial`: line status before `keys` pressed: strings before and after cursor
// `keys`: keys to press
// `expected_line`: line after enter key
fn assert_line_with_initial(
    mode: EditMode,
    initial: (&str, &str),
    keys: &[KeyPress],
    expected_line: &str,
) {
    let mut editor = init_editor(mode, keys);
    let actual_line = editor.readline_with_initial(">>", initial).unwrap();
    assert_eq!(expected_line, actual_line);
}

// `initial`: line status before `keys` pressed: strings before and after cursor
// `keys`: keys to press
// `expected`: line status before enter key: strings before and after cursor
fn assert_cursor(mode: EditMode, initial: (&str, &str), keys: &[KeyPress], expected: (&str, &str)) {
    let mut editor = init_editor(mode, keys);
    let actual_line = editor.readline_with_initial("", initial).unwrap();
    assert_eq!(expected.0.to_owned() + expected.1, actual_line);
    assert_eq!(expected.0.len(), editor.term.cursor);
}

// `entries`: history entries before `keys` pressed
// `keys`: keys to press
// `expected`: line status before enter key: strings before and after cursor
fn assert_history(
    mode: EditMode,
    entries: &[&str],
    keys: &[KeyPress],
    prompt: &str,
    expected: (&str, &str),
) {
    let mut editor = init_editor(mode, keys);
    for entry in entries {
        editor.history.add(*entry);
    }
    let actual_line = editor.readline(prompt).unwrap();
    assert_eq!(expected.0.to_owned() + expected.1, actual_line);
    if prompt.is_empty() {
        assert_eq!(expected.0.len(), editor.term.cursor);
    }
}

#[test]
fn unknown_esc_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[KeyPress::UnknownEscSeq, KeyPress::Enter], "");
    }
}

#[test]
fn test_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Editor<()>>();
}

#[test]
fn test_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Editor<()>>();
}
