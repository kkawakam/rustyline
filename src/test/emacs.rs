//! Emacs specific key bindings
use super::{assert_cursor, assert_history};
use crate::config::EditMode;
use crate::keys::KeyPress;

#[test]
fn ctrl_a() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::ctrl('A'), KeyPress::ENTER],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("test test\n123", "foo"),
        &[KeyPress::ctrl('A'), KeyPress::ENTER],
        ("test test\n", "123foo"),
    );
}

#[test]
fn ctrl_e() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::ctrl('E'), KeyPress::ENTER],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo", "test test\n123"),
        &[KeyPress::ctrl('E'), KeyPress::ENTER],
        ("footest test", "\n123"),
    );
}

#[test]
fn ctrl_b() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::ctrl('B'), KeyPress::ENTER],
        ("H", "i"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::meta('2'), KeyPress::ctrl('B'), KeyPress::ENTER],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            KeyPress::meta('-'),
            KeyPress::meta('2'),
            KeyPress::ctrl('B'),
            KeyPress::ENTER,
        ],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_f() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::ctrl('F'), KeyPress::ENTER],
        ("H", "i"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::meta('2'), KeyPress::ctrl('F'), KeyPress::ENTER],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[
            KeyPress::meta('-'),
            KeyPress::meta('2'),
            KeyPress::ctrl('F'),
            KeyPress::ENTER,
        ],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::ctrl('H'), KeyPress::ENTER],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::meta('2'), KeyPress::ctrl('H'), KeyPress::ENTER],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            KeyPress::meta('-'),
            KeyPress::meta('2'),
            KeyPress::ctrl('H'),
            KeyPress::ENTER,
        ],
        ("", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(
        EditMode::Emacs,
        ("", ""),
        &[KeyPress::BACKSPACE, KeyPress::ENTER],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::BACKSPACE, KeyPress::ENTER],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::BACKSPACE, KeyPress::ENTER],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_k() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::ctrl('K'), KeyPress::ENTER],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::ctrl('K'), KeyPress::ENTER],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("B", "ye"),
        &[KeyPress::ctrl('K'), KeyPress::ENTER],
        ("B", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", "foo\nbar"),
        &[KeyPress::ctrl('K'), KeyPress::ENTER],
        ("Hi", "\nbar"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", "\nbar"),
        &[KeyPress::ctrl('K'), KeyPress::ENTER],
        ("Hi", "bar"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", "bar"),
        &[KeyPress::ctrl('K'), KeyPress::ENTER],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_u() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::ctrl('U'), KeyPress::ENTER],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::ctrl('U'), KeyPress::ENTER],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("B", "ye"),
        &[KeyPress::ctrl('U'), KeyPress::ENTER],
        ("", "ye"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo\nbar", "Hi"),
        &[KeyPress::ctrl('U'), KeyPress::ENTER],
        ("foo\n", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo\n", "Hi"),
        &[KeyPress::ctrl('U'), KeyPress::ENTER],
        ("foo", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo", "Hi"),
        &[KeyPress::ctrl('U'), KeyPress::ENTER],
        ("", "Hi"),
    );
}
#[test]
fn ctrl_n() {
    assert_history(
        EditMode::Emacs,
        &["line1", "line2"],
        &[
            KeyPress::ctrl('P'),
            KeyPress::ctrl('P'),
            KeyPress::ctrl('N'),
            KeyPress::ENTER,
        ],
        "",
        ("line2", ""),
    );
}

#[test]
fn ctrl_p() {
    assert_history(
        EditMode::Emacs,
        &["line1"],
        &[KeyPress::ctrl('P'), KeyPress::ENTER],
        "",
        ("line1", ""),
    );
}

#[test]
fn ctrl_t() {
    /* FIXME
    assert_cursor(
        ("ab", "cd"),
        &[KeyPress::meta('2'), KeyPress::ctrl('T'), KeyPress::ENTER],
        ("acdb", ""),
    );*/
}

#[test]
fn ctrl_x_ctrl_u() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, ", "world"),
        &[
            KeyPress::ctrl('W'),
            KeyPress::ctrl('X'),
            KeyPress::ctrl('U'),
            KeyPress::ENTER,
        ],
        ("Hello, ", "world"),
    );
}

#[test]
fn meta_b() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[KeyPress::meta('B'), KeyPress::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[KeyPress::meta('2'), KeyPress::meta('B'), KeyPress::ENTER],
        ("", "Hello, world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[KeyPress::meta('-'), KeyPress::meta('B'), KeyPress::ENTER],
        ("Hello", ", world!"),
    );
}

#[test]
fn meta_f() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[KeyPress::meta('F'), KeyPress::ENTER],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[KeyPress::meta('2'), KeyPress::meta('F'), KeyPress::ENTER],
        ("Hello, world", "!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[KeyPress::meta('-'), KeyPress::meta('F'), KeyPress::ENTER],
        ("Hello, ", "world!"),
    );
}

#[test]
fn meta_c() {
    assert_cursor(
        EditMode::Emacs,
        ("hi", ""),
        &[KeyPress::meta('C'), KeyPress::ENTER],
        ("hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "hi"),
        &[KeyPress::meta('C'), KeyPress::ENTER],
        ("Hi", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "hi test"),
        &[KeyPress::meta('2'), KeyPress::meta('C'), KeyPress::ENTER],
        ("Hi Test", ""),
    );*/
}

#[test]
fn meta_l() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[KeyPress::meta('L'), KeyPress::ENTER],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "HI"),
        &[KeyPress::meta('L'), KeyPress::ENTER],
        ("hi", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "HI TEST"),
        &[KeyPress::meta('2'), KeyPress::meta('L'), KeyPress::ENTER],
        ("hi test", ""),
    );*/
}

#[test]
fn meta_u() {
    assert_cursor(
        EditMode::Emacs,
        ("hi", ""),
        &[KeyPress::meta('U'), KeyPress::ENTER],
        ("hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "hi"),
        &[KeyPress::meta('U'), KeyPress::ENTER],
        ("HI", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "hi test"),
        &[KeyPress::meta('2'), KeyPress::meta('U'), KeyPress::ENTER],
        ("HI TEST", ""),
    );*/
}

#[test]
fn meta_d() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[KeyPress::meta('D'), KeyPress::ENTER],
        ("Hello", "!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[KeyPress::meta('2'), KeyPress::meta('D'), KeyPress::ENTER],
        ("Hello", ""),
    );
}

#[test]
fn meta_t() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[KeyPress::meta('T'), KeyPress::ENTER],
        ("world, Hello", "!"),
    );
    /* FIXME
    assert_cursor(
        ("One Two", " Three Four"),
        &[KeyPress::meta('T'), KeyPress::ENTER],
        ("One Four Three Two", ""),
    );*/
}

#[test]
fn meta_y() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world", "!"),
        &[
            KeyPress::ctrl('W'),
            KeyPress::LEFT,
            KeyPress::ctrl('W'),
            KeyPress::ctrl('Y'),
            KeyPress::meta('Y'),
            KeyPress::ENTER,
        ],
        ("world", " !"),
    );
}

#[test]
fn meta_backspace() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, wor", "ld!"),
        &[KeyPress::meta('\x08'), KeyPress::ENTER],
        ("Hello, ", "ld!"),
    );
}

#[test]
fn meta_digit() {
    assert_cursor(
        EditMode::Emacs,
        ("", ""),
        &[KeyPress::meta('3'), KeyPress::normal('h'), KeyPress::ENTER],
        ("hhh", ""),
    );
}
