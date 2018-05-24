use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::{Editor, Result};
use completion::Completer;
use config::Config;
use consts::KeyPress;
use edit::init_state;
use keymap::{Cmd, InputState};
use tty::Sink;

mod common;
mod emacs;
mod history;

fn init_editor(keys: &[KeyPress]) -> Editor<()> {
    let mut editor = Editor::<()>::new();
    editor.term.keys.extend(keys.iter().cloned());
    editor
}

struct SimpleCompleter;
impl Completer for SimpleCompleter {
    fn complete(&self, line: &str, _pos: usize) -> Result<(usize, Vec<String>)> {
        Ok((0, vec![line.to_owned() + "t"]))
    }
}

#[test]
fn complete_line() {
    let mut out = Sink::new();
    let mut s = init_state(&mut out, "rus", 3);
    let config = Config::default();
    let mut input_state = InputState::new(&config, Rc::new(RefCell::new(HashMap::new())));
    let keys = &[KeyPress::Enter];
    let mut rdr = keys.iter();
    let completer = SimpleCompleter;
    let cmd = super::complete_line(
        &mut rdr,
        &mut s,
        &mut input_state,
        &completer,
        &Config::default(),
    ).unwrap();
    assert_eq!(Some(Cmd::AcceptLine), cmd);
    assert_eq!("rust", s.line.as_str());
    assert_eq!(4, s.line.pos());
}

fn assert_line(keys: &[KeyPress], expected_line: &str) {
    let mut editor = init_editor(keys);
    let actual_line = editor.readline(">>").unwrap();
    assert_eq!(expected_line, actual_line);
}
fn assert_line_with_initial(initial: (&str, &str), keys: &[KeyPress], expected_line: &str) {
    let mut editor = init_editor(keys);
    let actual_line = editor.readline_with_initial(">>", initial).unwrap();
    assert_eq!(expected_line, actual_line);
}
fn assert_cursor(initial: (&str, &str), keys: &[KeyPress], expected: (&str, &str)) {
    let mut editor = init_editor(keys);
    let actual_line = editor.readline_with_initial("", initial).unwrap();
    assert_eq!(expected.0.to_owned() + expected.1, actual_line);
    assert_eq!(expected.0.len(), editor.term.cursor);
}

fn assert_history(entries: &[&str], keys: &[KeyPress], expected: (&str, &str)) {
    let mut editor = init_editor(keys);
    for entry in entries {
        editor.history.add(*entry);
    }
    let actual_line = editor.readline("").unwrap();
    assert_eq!(expected.0.to_owned() + expected.1, actual_line);
    assert_eq!(expected.0.len(), editor.term.cursor);
}

#[test]
fn unknown_esc_key() {
    assert_line(&[KeyPress::UnknownEscSeq, KeyPress::Enter], "");
}
