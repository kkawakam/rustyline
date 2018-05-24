//! Emacs specific key bindings
use super::{assert_cursor, assert_history};
use consts::KeyPress;

#[test]
fn ctrl_a() {
    assert_cursor(
        ("Hi", ""),
        &[KeyPress::Ctrl('A'), KeyPress::Enter],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_e() {
    assert_cursor(
        ("", "Hi"),
        &[KeyPress::Ctrl('E'), KeyPress::Enter],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_b() {
    assert_cursor(
        ("Hi", ""),
        &[KeyPress::Ctrl('B'), KeyPress::Enter],
        ("H", "i"),
    );
}

#[test]
fn ctrl_f() {
    assert_cursor(
        ("", "Hi"),
        &[KeyPress::Ctrl('F'), KeyPress::Enter],
        ("H", "i"),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        ("Hi", ""),
        &[KeyPress::Ctrl('H'), KeyPress::Enter],
        ("H", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(("", ""), &[KeyPress::Backspace, KeyPress::Enter], ("", ""));
    assert_cursor(
        ("Hi", ""),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("H", ""),
    );
    assert_cursor(
        ("", "Hi"),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_k() {
    assert_cursor(
        ("Hi", ""),
        &[KeyPress::Ctrl('K'), KeyPress::Enter],
        ("Hi", ""),
    );
    assert_cursor(
        ("", "Hi"),
        &[KeyPress::Ctrl('K'), KeyPress::Enter],
        ("", ""),
    );
    assert_cursor(
        ("B", "ye"),
        &[KeyPress::Ctrl('K'), KeyPress::Enter],
        ("B", ""),
    );
}

#[test]
fn ctrl_n() {
    assert_history(
        &["line1", "line2"],
        &[
            KeyPress::Ctrl('P'),
            KeyPress::Ctrl('P'),
            KeyPress::Ctrl('N'),
            KeyPress::Enter,
        ],
        ("line2", ""),
    );
}

#[test]
fn ctrl_p() {
    assert_history(
        &["line1"],
        &[KeyPress::Ctrl('P'), KeyPress::Enter],
        ("line1", ""),
    );
}

#[test]
fn ctrl_x_ctrl_u() {
    assert_cursor(
        ("Hello, ", "world"),
        &[
            KeyPress::Ctrl('W'),
            KeyPress::Ctrl('X'),
            KeyPress::Ctrl('U'),
            KeyPress::Enter,
        ],
        ("Hello, ", "world"),
    );
}

#[test]
fn meta_b() {
    assert_cursor(
        ("Hello, world!", ""),
        &[KeyPress::Meta('B'), KeyPress::Enter],
        ("Hello, ", "world!"),
    );
}

#[test]
fn meta_f() {
    assert_cursor(
        ("", "Hello, world!"),
        &[KeyPress::Meta('F'), KeyPress::Enter],
        ("Hello", ", world!"),
    );
}

#[test]
fn meta_c() {
    assert_cursor(
        ("hi", ""),
        &[KeyPress::Meta('C'), KeyPress::Enter],
        ("hi", ""),
    );
    assert_cursor(
        ("", "hi"),
        &[KeyPress::Meta('C'), KeyPress::Enter],
        ("Hi", ""),
    );
}

#[test]
fn meta_l() {
    assert_cursor(
        ("Hi", ""),
        &[KeyPress::Meta('L'), KeyPress::Enter],
        ("Hi", ""),
    );
    assert_cursor(
        ("", "HI"),
        &[KeyPress::Meta('L'), KeyPress::Enter],
        ("hi", ""),
    );
}

#[test]
fn meta_u() {
    assert_cursor(
        ("hi", ""),
        &[KeyPress::Meta('U'), KeyPress::Enter],
        ("hi", ""),
    );
    assert_cursor(
        ("", "hi"),
        &[KeyPress::Meta('U'), KeyPress::Enter],
        ("HI", ""),
    );
}

#[test]
fn meta_d() {
    assert_cursor(
        ("Hello", ", world!"),
        &[KeyPress::Meta('D'), KeyPress::Enter],
        ("Hello", "!"),
    );
}

#[test]
fn meta_t() {
    assert_cursor(
        ("Hello", ", world!"),
        &[KeyPress::Meta('T'), KeyPress::Enter],
        ("world, Hello", "!"),
    );
}

#[test]
fn meta_y() {
    assert_cursor(
        ("Hello, world", "!"),
        &[
            KeyPress::Ctrl('W'),
            KeyPress::Left,
            KeyPress::Ctrl('W'),
            KeyPress::Ctrl('Y'),
            KeyPress::Meta('Y'),
            KeyPress::Enter,
        ],
        ("world", " !"),
    );
}
