///! Basic commands tests.
use super::{assert_cursor, assert_line, assert_line_with_initial, init_editor};
use consts::KeyPress;
use error::ReadlineError;

#[test]
fn home_key() {
    assert_cursor(("", ""), &[KeyPress::Home, KeyPress::Enter], ("", ""));
    assert_cursor(("Hi", ""), &[KeyPress::Home, KeyPress::Enter], ("", "Hi"));
}

#[test]
fn end_key() {
    assert_cursor(("", ""), &[KeyPress::End, KeyPress::Enter], ("", ""));
    assert_cursor(("H", "i"), &[KeyPress::End, KeyPress::Enter], ("Hi", ""));
    assert_cursor(("", "Hi"), &[KeyPress::End, KeyPress::Enter], ("Hi", ""));
}

#[test]
fn left_key() {
    assert_cursor(("Hi", ""), &[KeyPress::Left, KeyPress::Enter], ("H", "i"));
    assert_cursor(("H", "i"), &[KeyPress::Left, KeyPress::Enter], ("", "Hi"));
    assert_cursor(("", "Hi"), &[KeyPress::Left, KeyPress::Enter], ("", "Hi"));
}

#[test]
fn right_key() {
    assert_cursor(("", ""), &[KeyPress::Right, KeyPress::Enter], ("", ""));
    assert_cursor(("", "Hi"), &[KeyPress::Right, KeyPress::Enter], ("H", "i"));
    assert_cursor(
        ("B", "ye"),
        &[KeyPress::Right, KeyPress::Enter],
        ("By", "e"),
    );
    assert_cursor(("H", "i"), &[KeyPress::Right, KeyPress::Enter], ("Hi", ""));
}

#[test]
fn enter_key() {
    assert_line(&[KeyPress::Enter], "");
    assert_line(&[KeyPress::Char('a'), KeyPress::Enter], "a");
    assert_line_with_initial(("Hi", ""), &[KeyPress::Enter], "Hi");
    assert_line_with_initial(("", "Hi"), &[KeyPress::Enter], "Hi");
    assert_line_with_initial(("H", "i"), &[KeyPress::Enter], "Hi");
}

#[test]
fn newline_key() {
    assert_line(&[KeyPress::Ctrl('J')], "");
    assert_line(&[KeyPress::Char('a'), KeyPress::Ctrl('J')], "a");
}

#[test]
fn eof_key() {
    let mut editor = init_editor(&[KeyPress::Ctrl('D')]);
    let err = editor.readline(">>");
    assert_matches!(err, Err(ReadlineError::Eof));

    assert_line(
        &[KeyPress::Char('a'), KeyPress::Ctrl('D'), KeyPress::Enter],
        "a",
    );
    assert_line_with_initial(("", "Hi"), &[KeyPress::Ctrl('D'), KeyPress::Enter], "i");
}

#[test]
fn interrupt_key() {
    let mut editor = init_editor(&[KeyPress::Ctrl('C')]);
    let err = editor.readline(">>");
    assert_matches!(err, Err(ReadlineError::Interrupted));

    let mut editor = init_editor(&[KeyPress::Ctrl('C')]);
    let err = editor.readline_with_initial(">>", ("Hi", ""));
    assert_matches!(err, Err(ReadlineError::Interrupted));
}

#[test]
fn delete_key() {
    assert_cursor(("a", ""), &[KeyPress::Delete, KeyPress::Enter], ("a", ""));
    assert_cursor(("", "a"), &[KeyPress::Delete, KeyPress::Enter], ("", ""));
}

#[test]
fn ctrl_t() {
    assert_cursor(
        ("a", "b"),
        &[KeyPress::Ctrl('T'), KeyPress::Enter],
        ("ba", ""),
    );
    assert_cursor(
        ("ab", "cd"),
        &[KeyPress::Ctrl('T'), KeyPress::Enter],
        ("acb", "d"),
    );
}

#[test]
fn ctrl_u() {
    assert_cursor(
        ("start of line ", "end"),
        &[KeyPress::Ctrl('U'), KeyPress::Enter],
        ("", "end"),
    );
    assert_cursor(
        ("", "end"),
        &[KeyPress::Ctrl('U'), KeyPress::Enter],
        ("", "end"),
    );
}

#[test]
fn ctrl_v() {
    assert_cursor(
        ("", ""),
        &[KeyPress::Ctrl('V'), KeyPress::Char('\t'), KeyPress::Enter],
        ("\t", ""),
    );
}

#[test]
fn ctrl_w() {
    assert_cursor(
        ("Hello, ", "world"),
        &[KeyPress::Ctrl('W'), KeyPress::Enter],
        ("", "world"),
    );
    assert_cursor(
        ("Hello, world.", ""),
        &[KeyPress::Ctrl('W'), KeyPress::Enter],
        ("Hello, ", ""),
    );
}

#[test]
fn ctrl_y() {
    assert_cursor(
        ("Hello, ", "world"),
        &[KeyPress::Ctrl('W'), KeyPress::Ctrl('Y'), KeyPress::Enter],
        ("Hello, ", "world"),
    );
}

#[test]
fn ctrl__() {
    assert_cursor(
        ("Hello, ", "world"),
        &[KeyPress::Ctrl('W'), KeyPress::Ctrl('_'), KeyPress::Enter],
        ("Hello, ", "world"),
    );
}
