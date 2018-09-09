//! Vi command mode specific key bindings
use super::{assert_cursor, assert_history};
use config::EditMode;
use keys::KeyPress;

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
fn semi_colon() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('f'),
            KeyPress::Char('o'),
            KeyPress::Char(';'),
            KeyPress::Enter,
        ],
        ("Hello, w", "orld!"),
    );
}

#[test]
fn comma() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, w", "orld!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('f'),
            KeyPress::Char('l'),
            KeyPress::Char(','),
            KeyPress::Enter,
        ],
        ("Hel", "lo, world!"),
    );
}

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
fn uppercase_c() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, w", "orld!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('C'),
            KeyPress::Char('i'),
            KeyPress::Enter,
        ],
        ("Hello, i", ""),
    );
}

#[test]
fn ctrl_k() {
    for key in &[KeyPress::Char('D'), KeyPress::Ctrl('K')] {
        assert_cursor(
            EditMode::Vi,
            ("Hi", ""),
            &[KeyPress::Esc, *key, KeyPress::Enter],
            ("H", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[KeyPress::Esc, *key, KeyPress::Enter],
            ("", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("By", "e"),
            &[KeyPress::Esc, *key, KeyPress::Enter],
            ("B", ""),
        );
    }
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
fn f() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('f'),
            KeyPress::Char('r'),
            KeyPress::Enter,
        ],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('3'),
            KeyPress::Char('f'),
            KeyPress::Char('l'),
            KeyPress::Enter,
        ],
        ("Hello, wor", "ld!"),
    );
}

#[test]
fn uppercase_f() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('F'),
            KeyPress::Char('r'),
            KeyPress::Enter,
        ],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('3'),
            KeyPress::Char('F'),
            KeyPress::Char('l'),
            KeyPress::Enter,
        ],
        ("He", "llo, world!"),
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

#[test]
fn r() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ", world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('r'),
            KeyPress::Char('o'),
            KeyPress::Enter,
        ],
        ("H", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('4'),
            KeyPress::Char('r'),
            KeyPress::Char('i'),
            KeyPress::Enter,
        ],
        ("Hiii", "i, world!"),
    );
}

#[test]
fn s() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ", world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('s'),
            KeyPress::Char('o'),
            KeyPress::Enter,
        ],
        ("Ho", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('4'),
            KeyPress::Char('s'),
            KeyPress::Char('i'),
            KeyPress::Enter,
        ],
        ("Hi", ", world!"),
    );
}

#[test]
fn uppercase_s() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[KeyPress::Esc, KeyPress::Char('S'), KeyPress::Enter],
        ("", ""),
    );
}

#[test]
fn t() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('t'),
            KeyPress::Char('r'),
            KeyPress::Enter,
        ],
        ("Hello, w", "orld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::Esc,
            KeyPress::Char('3'),
            KeyPress::Char('t'),
            KeyPress::Char('l'),
            KeyPress::Enter,
        ],
        ("Hello, wo", "rld!"),
    );
}

#[test]
fn uppercase_t() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('T'),
            KeyPress::Char('r'),
            KeyPress::Enter,
        ],
        ("Hello, wor", "ld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::Esc,
            KeyPress::Char('3'),
            KeyPress::Char('T'),
            KeyPress::Char('l'),
            KeyPress::Enter,
        ],
        ("Hel", "lo, world!"),
    );
}
