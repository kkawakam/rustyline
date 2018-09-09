//! Emacs specific key bindings
use super::{assert_cursor, assert_history};
use config::EditMode;
use keys::KeyPress;

#[test]
fn ctrl_a() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Ctrl('A'), KeyPress::Enter],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_e() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::Ctrl('E'), KeyPress::Enter],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_b() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Ctrl('B'), KeyPress::Enter],
        ("H", "i"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Meta('2'), KeyPress::Ctrl('B'), KeyPress::Enter],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            KeyPress::Meta('-'),
            KeyPress::Meta('2'),
            KeyPress::Ctrl('B'),
            KeyPress::Enter,
        ],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_f() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::Ctrl('F'), KeyPress::Enter],
        ("H", "i"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::Meta('2'), KeyPress::Ctrl('F'), KeyPress::Enter],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[
            KeyPress::Meta('-'),
            KeyPress::Meta('2'),
            KeyPress::Ctrl('F'),
            KeyPress::Enter,
        ],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Ctrl('H'), KeyPress::Enter],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Meta('2'), KeyPress::Ctrl('H'), KeyPress::Enter],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            KeyPress::Meta('-'),
            KeyPress::Meta('2'),
            KeyPress::Ctrl('H'),
            KeyPress::Enter,
        ],
        ("", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(
        EditMode::Emacs,
        ("", ""),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::Backspace, KeyPress::Enter],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_k() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Ctrl('K'), KeyPress::Enter],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::Ctrl('K'), KeyPress::Enter],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("B", "ye"),
        &[KeyPress::Ctrl('K'), KeyPress::Enter],
        ("B", ""),
    );
}

#[test]
fn ctrl_n() {
    assert_history(
        EditMode::Emacs,
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
        EditMode::Emacs,
        &["line1"],
        &[KeyPress::Ctrl('P'), KeyPress::Enter],
        ("line1", ""),
    );
}

#[test]
fn ctrl_t() {
    /* FIXME
    assert_cursor(
        ("ab", "cd"),
        &[KeyPress::Meta('2'), KeyPress::Ctrl('T'), KeyPress::Enter],
        ("acdb", ""),
    );*/
}

#[test]
fn ctrl_x_ctrl_u() {
    assert_cursor(
        EditMode::Emacs,
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
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[KeyPress::Meta('B'), KeyPress::Enter],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[KeyPress::Meta('2'), KeyPress::Meta('B'), KeyPress::Enter],
        ("", "Hello, world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[KeyPress::Meta('-'), KeyPress::Meta('B'), KeyPress::Enter],
        ("Hello", ", world!"),
    );
}

#[test]
fn meta_f() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[KeyPress::Meta('F'), KeyPress::Enter],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[KeyPress::Meta('2'), KeyPress::Meta('F'), KeyPress::Enter],
        ("Hello, world", "!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[KeyPress::Meta('-'), KeyPress::Meta('F'), KeyPress::Enter],
        ("Hello, ", "world!"),
    );
}

#[test]
fn meta_c() {
    assert_cursor(
        EditMode::Emacs,
        ("hi", ""),
        &[KeyPress::Meta('C'), KeyPress::Enter],
        ("hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "hi"),
        &[KeyPress::Meta('C'), KeyPress::Enter],
        ("Hi", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "hi test"),
        &[KeyPress::Meta('2'), KeyPress::Meta('C'), KeyPress::Enter],
        ("Hi Test", ""),
    );*/
}

#[test]
fn meta_l() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::Meta('L'), KeyPress::Enter],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "HI"),
        &[KeyPress::Meta('L'), KeyPress::Enter],
        ("hi", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "HI TEST"),
        &[KeyPress::Meta('2'), KeyPress::Meta('L'), KeyPress::Enter],
        ("hi test", ""),
    );*/
}

#[test]
fn meta_u() {
    assert_cursor(
        EditMode::Emacs,
        ("hi", ""),
        &[KeyPress::Meta('U'), KeyPress::Enter],
        ("hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "hi"),
        &[KeyPress::Meta('U'), KeyPress::Enter],
        ("HI", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "hi test"),
        &[KeyPress::Meta('2'), KeyPress::Meta('U'), KeyPress::Enter],
        ("HI TEST", ""),
    );*/
}

#[test]
fn meta_d() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[KeyPress::Meta('D'), KeyPress::Enter],
        ("Hello", "!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[KeyPress::Meta('2'), KeyPress::Meta('D'), KeyPress::Enter],
        ("Hello", ""),
    );
}

#[test]
fn meta_t() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[KeyPress::Meta('T'), KeyPress::Enter],
        ("world, Hello", "!"),
    );
    /* FIXME
    assert_cursor(
        ("One Two", " Three Four"),
        &[KeyPress::Meta('T'), KeyPress::Enter],
        ("One Four Three Two", ""),
    );*/
}

#[test]
fn meta_y() {
    assert_cursor(
        EditMode::Emacs,
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

#[test]
fn meta_backspace() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, wor", "ld!"),
        &[KeyPress::Meta('\x08'), KeyPress::Enter],
        ("Hello, ", "ld!"),
    );
}

#[test]
fn meta_digit() {
    assert_cursor(
        EditMode::Emacs,
        ("", ""),
        &[KeyPress::Meta('3'), KeyPress::Char('h'), KeyPress::Enter],
        ("hhh", ""),
    );
}
