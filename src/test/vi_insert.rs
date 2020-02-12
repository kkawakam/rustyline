//! Vi insert mode specific key bindings
use super::assert_cursor;
use crate::config::EditMode;
use crate::keys::KeyPress;

#[test]
fn insert_mode_by_default() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[KeyPress::from('a'), KeyPress::ENTER],
        ("a", ""),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::ctrl('H'), KeyPress::ENTER],
        ("H", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[KeyPress::BACKSPACE, KeyPress::ENTER],
        ("", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::BACKSPACE, KeyPress::ENTER],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[KeyPress::BACKSPACE, KeyPress::ENTER],
        ("", "Hi"),
    );
}

#[test]
fn esc() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[KeyPress::from('a'), KeyPress::ESC, KeyPress::ENTER],
        ("", "a"),
    );
}
