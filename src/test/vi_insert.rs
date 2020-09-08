//! Vi insert mode specific key bindings
use super::assert_cursor;
use crate::config::EditMode;
use crate::keys::{KeyCode as K, Modifiers as M};

#[test]
fn insert_mode_by_default() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[(K::Char('a'), M::NONE), (K::Enter, M::NONE)],
        ("a", ""),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[(K::Char('H'), M::CTRL), (K::Enter, M::NONE)],
        ("H", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[(K::Backspace, M::NONE), (K::Enter, M::NONE)],
        ("", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[(K::Backspace, M::NONE), (K::Enter, M::NONE)],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[(K::Backspace, M::NONE), (K::Enter, M::NONE)],
        ("", "Hi"),
    );
}

#[test]
fn esc() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[
            (K::Char('a'), M::NONE),
            (K::Esc, M::NONE),
            (K::Enter, M::NONE),
        ],
        ("", "a"),
    );
}
