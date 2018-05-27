//! Vi command mode specific key bindings
use super::{assert_cursor, assert_history};
use config::EditMode;
use consts::KeyPress;

#[test]
fn dollar() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[KeyPress::Esc, KeyPress::Char('$'), KeyPress::Enter],
        ("Hi", ""), // FIXME
    );
}

/*#[test]
fn dot() {
    // TODO
}*/

#[test]
fn zero() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::Esc, KeyPress::Char('0'), KeyPress::Enter],
        ("", "Hi"),
    );
}

#[test]
fn caret() {
    assert_cursor(
        EditMode::Vi,
        (" Hi", ""),
        &[KeyPress::Esc, KeyPress::Char('^'), KeyPress::Enter],
        (" ", "Hi"),
    );
}

#[test]
fn a() {
    assert_cursor(
        EditMode::Vi,
        ("B", "e"),
        &[
            KeyPress::Esc,
            KeyPress::Char('a'),
            KeyPress::Char('y'),
            KeyPress::Enter,
        ],
        ("By", "e"),
    );
}

#[test]
fn uppercase_a() {
    assert_cursor(
        EditMode::Vi,
        ("", "By"),
        &[
            KeyPress::Esc,
            KeyPress::Char('A'),
            KeyPress::Char('e'),
            KeyPress::Enter,
        ],
        ("Bye", ""),
    );
}

#[test]
fn b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[KeyPress::Esc, KeyPress::Char('b'), KeyPress::Enter],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('2'),
            KeyPress::Char('b'),
            KeyPress::Enter,
        ],
        ("Hello", ", world!"),
    );
}

#[test]
fn uppercase_b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[KeyPress::Esc, KeyPress::Char('B'), KeyPress::Enter],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('2'),
            KeyPress::Char('B'),
            KeyPress::Enter,
        ],
        ("", "Hello, world!"),
    );
}

#[test]
fn ctrl_k() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::Esc, KeyPress::Ctrl('K'), KeyPress::Enter],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[KeyPress::Esc, KeyPress::Ctrl('K'), KeyPress::Enter],
        ("", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("By", "e"),
        &[KeyPress::Esc, KeyPress::Ctrl('K'), KeyPress::Enter],
        ("B", ""),
    );
}

#[test]
fn e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::Esc, KeyPress::Char('e'), KeyPress::Enter],
        ("Hell", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('2'),
            KeyPress::Char('e'),
            KeyPress::Enter,
        ],
        ("Hello, worl", "d!"),
    );
}

#[test]
fn uppercase_e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::Esc, KeyPress::Char('E'), KeyPress::Enter],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('2'),
            KeyPress::Char('E'),
            KeyPress::Enter,
        ],
        ("Hello, world", "!"),
    );
}

#[test]
fn i() {
    assert_cursor(
        EditMode::Vi,
        ("Be", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('i'),
            KeyPress::Char('y'),
            KeyPress::Enter,
        ],
        ("By", "e"),
    );
}

#[test]
fn uppercase_i() {
    assert_cursor(
        EditMode::Vi,
        ("Be", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('I'),
            KeyPress::Char('y'),
            KeyPress::Enter,
        ],
        ("y", "Be"),
    );
}

#[test]
fn u() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[
            KeyPress::Esc,
            KeyPress::Ctrl('W'),
            KeyPress::Char('u'),
            KeyPress::Enter,
        ],
        ("Hello,", " world"),
    );
}

#[test]
fn w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::Esc, KeyPress::Char('w'), KeyPress::Enter],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('2'),
            KeyPress::Char('w'),
            KeyPress::Enter,
        ],
        ("Hello, ", "world!"),
    );
}

#[test]
fn uppercase_w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::Esc, KeyPress::Char('W'), KeyPress::Enter],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('2'),
            KeyPress::Char('W'),
            KeyPress::Enter,
        ],
        ("Hello, world", "!"),
    );
}

#[test]
fn x() {
    assert_cursor(
        EditMode::Vi,
        ("", "a"),
        &[KeyPress::Esc, KeyPress::Char('x'), KeyPress::Enter],
        ("", ""),
    );
}

#[test]
fn uppercase_x() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::Esc, KeyPress::Char('X'), KeyPress::Enter],
        ("", "i"),
    );
}

#[test]
fn h() {
    for key in &[
        KeyPress::Char('h'),
        KeyPress::Ctrl('H'),
        KeyPress::Backspace,
    ] {
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[KeyPress::Esc, *key, KeyPress::Enter],
            ("B", "ye"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[KeyPress::Esc, KeyPress::Char('2'), *key, KeyPress::Enter],
            ("", "Bye"),
        );
    }
}

#[test]
fn l() {
    for key in &[KeyPress::Char('l'), KeyPress::Char(' ')] {
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[KeyPress::Esc, *key, KeyPress::Enter],
            ("H", "i"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[KeyPress::Esc, KeyPress::Char('2'), *key, KeyPress::Enter],
            ("Hi", ""),
        );
    }
}

#[test]
fn j() {
    for key in &[
        KeyPress::Char('j'),
        KeyPress::Char('+'),
        KeyPress::Ctrl('N'),
    ] {
        assert_history(
            EditMode::Vi,
            &["line1", "line2"],
            &[
                KeyPress::Esc,
                KeyPress::Ctrl('P'),
                KeyPress::Ctrl('P'),
                *key,
                KeyPress::Enter,
            ],
            ("line2", ""),
        );
    }
}

#[test]
fn k() {
    for key in &[
        KeyPress::Char('k'),
        KeyPress::Char('-'),
        KeyPress::Ctrl('P'),
    ] {
        assert_history(
            EditMode::Vi,
            &["line1"],
            &[KeyPress::Esc, *key, KeyPress::Enter],
            ("line1", ""),
        );
    }
}

#[test]
fn p() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[
            KeyPress::Esc,
            KeyPress::Ctrl('W'),
            KeyPress::Char('p'),
            KeyPress::Enter,
        ],
        (" Hello", ",world"),
    );
}

#[test]
fn uppercase_p() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[
            KeyPress::Esc,
            KeyPress::Ctrl('W'),
            KeyPress::Char('P'),
            KeyPress::Enter,
        ],
        ("Hello", ", world"),
    );
}
