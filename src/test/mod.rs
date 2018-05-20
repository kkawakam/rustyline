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
fn assert_cursor(initial: (&str, &str), keys: &[KeyPress], expected_cursor: usize) {
    let mut editor = init_editor(keys);
    editor.readline_with_initial("", initial).unwrap();
    assert_eq!(expected_cursor, editor.term.cursor.get());
}

#[test]
fn down_key() {
    assert_line(&[KeyPress::Down, KeyPress::Enter], "");
}

#[test]
fn meta_backspace_key() {
    assert_line(&[KeyPress::Meta('\x08'), KeyPress::Enter], "");
}

#[test]
fn page_down_key() {
    assert_line(&[KeyPress::PageDown, KeyPress::Enter], "");
}

#[test]
fn page_up_key() {
    assert_line(&[KeyPress::PageUp, KeyPress::Enter], "");
}

#[test]
fn up_key() {
    assert_line(&[KeyPress::Up, KeyPress::Enter], "");
}

#[test]
fn unknown_esc_key() {
    assert_line(&[KeyPress::UnknownEscSeq, KeyPress::Enter], "");
}
