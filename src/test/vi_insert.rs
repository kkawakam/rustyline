//! Vi insert mode specific key bindings
use super::assert_cursor;
use config::EditMode;
use keys::KeyPress;

#[test]
fn insert_mode_by_default() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[KeyPress::Char('a'), KeyPress::Enter],
        ("a", ""),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::Ctrl('H'), KeyPress::Enter],
        ("H", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("", "Hi"),
    );
}

#[test]
fn esc() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[KeyPress::Char('a'), KeyPress::Esc, KeyPress::Enter],
        ("", "a"),
    );
}
