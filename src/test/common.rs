///! Basic commands tests.
use super::{assert_cursor, assert_line, assert_line_with_initial, init_editor};
use crate::config::EditMode;
use crate::error::ReadlineError;
use crate::keys::{KeyCode as K, Modifiers as M};

#[test]
fn home_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[(K::Home, M::NONE), (K::Enter, M::NONE)],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[(K::Home, M::NONE), (K::Enter, M::NONE)],
            ("", "Hi"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hi", ""),
                &[(K::Esc, M::NONE), (K::Home, M::NONE), (K::Enter, M::NONE)],
                ("", "Hi"),
            );
        }
    }
}

#[test]
fn end_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[(K::End, M::NONE), (K::Enter, M::NONE)],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[(K::End, M::NONE), (K::Enter, M::NONE)],
            ("Hi", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[(K::End, M::NONE), (K::Enter, M::NONE)],
            ("Hi", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "Hi"),
                &[(K::Esc, M::NONE), (K::End, M::NONE), (K::Enter, M::NONE)],
                ("Hi", ""),
            );
        }
    }
}

#[test]
fn left_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[(K::Left, M::NONE), (K::Enter, M::NONE)],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[(K::Left, M::NONE), (K::Enter, M::NONE)],
            ("", "Hi"),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[(K::Left, M::NONE), (K::Enter, M::NONE)],
            ("", "Hi"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Bye", ""),
                &[(K::Esc, M::NONE), (K::Left, M::NONE), (K::Enter, M::NONE)],
                ("B", "ye"),
            );
        }
    }
}

#[test]
fn right_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[(K::Right, M::NONE), (K::Enter, M::NONE)],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[(K::Right, M::NONE), (K::Enter, M::NONE)],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("B", "ye"),
            &[(K::Right, M::NONE), (K::Enter, M::NONE)],
            ("By", "e"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[(K::Right, M::NONE), (K::Enter, M::NONE)],
            ("Hi", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "Hi"),
                &[(K::Esc, M::NONE), (K::Right, M::NONE), (K::Enter, M::NONE)],
                ("H", "i"),
            );
        }
    }
}

#[test]
fn enter_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[(K::Enter, M::NONE)], "");
        assert_line(*mode, &[(K::Char('a'), M::NONE), (K::Enter, M::NONE)], "a");
        assert_line_with_initial(*mode, ("Hi", ""), &[(K::Enter, M::NONE)], "Hi");
        assert_line_with_initial(*mode, ("", "Hi"), &[(K::Enter, M::NONE)], "Hi");
        assert_line_with_initial(*mode, ("H", "i"), &[(K::Enter, M::NONE)], "Hi");
        if *mode == EditMode::Vi {
            // vi command mode
            assert_line(*mode, &[(K::Esc, M::NONE), (K::Enter, M::NONE)], "");
            assert_line(
                *mode,
                &[
                    (K::Char('a'), M::NONE),
                    (K::Esc, M::NONE),
                    (K::Enter, M::NONE),
                ],
                "a",
            );
            assert_line_with_initial(
                *mode,
                ("Hi", ""),
                &[(K::Esc, M::NONE), (K::Enter, M::NONE)],
                "Hi",
            );
            assert_line_with_initial(
                *mode,
                ("", "Hi"),
                &[(K::Esc, M::NONE), (K::Enter, M::NONE)],
                "Hi",
            );
            assert_line_with_initial(
                *mode,
                ("H", "i"),
                &[(K::Esc, M::NONE), (K::Enter, M::NONE)],
                "Hi",
            );
        }
    }
}

#[test]
fn newline_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[(K::Char('J'), M::CTRL)], "");
        assert_line(
            *mode,
            &[(K::Char('a'), M::NONE), (K::Char('J'), M::CTRL)],
            "a",
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_line(*mode, &[(K::Esc, M::NONE), (K::Char('J'), M::CTRL)], "");
            assert_line(
                *mode,
                &[
                    (K::Char('a'), M::NONE),
                    (K::Esc, M::NONE),
                    (K::Char('J'), M::CTRL),
                ],
                "a",
            );
        }
    }
}

#[test]
fn eof_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[(K::Char('D'), M::CTRL)]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Eof));
    }
    assert_line(
        EditMode::Emacs,
        &[
            (K::Char('a'), M::NONE),
            (K::Char('D'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        "a",
    );
    assert_line(
        EditMode::Vi,
        &[(K::Char('a'), M::NONE), (K::Char('D'), M::CTRL)],
        "a",
    );
    assert_line(
        EditMode::Vi,
        &[
            (K::Char('a'), M::NONE),
            (K::Esc, M::NONE),
            (K::Char('D'), M::CTRL),
        ],
        "a",
    );
    assert_line_with_initial(
        EditMode::Emacs,
        ("", "Hi"),
        &[(K::Char('D'), M::CTRL), (K::Enter, M::NONE)],
        "i",
    );
    assert_line_with_initial(EditMode::Vi, ("", "Hi"), &[(K::Char('D'), M::CTRL)], "Hi");
    assert_line_with_initial(
        EditMode::Vi,
        ("", "Hi"),
        &[(K::Esc, M::NONE), (K::Char('D'), M::CTRL)],
        "Hi",
    );
}

#[test]
fn interrupt_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[(K::Char('C'), M::CTRL)]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Interrupted));

        let mut editor = init_editor(*mode, &[(K::Char('C'), M::CTRL)]);
        let err = editor.readline_with_initial(">>", ("Hi", ""));
        assert_matches!(err, Err(ReadlineError::Interrupted));
        if *mode == EditMode::Vi {
            // vi command mode
            let mut editor = init_editor(*mode, &[(K::Esc, M::NONE), (K::Char('C'), M::CTRL)]);
            let err = editor.readline_with_initial(">>", ("Hi", ""));
            assert_matches!(err, Err(ReadlineError::Interrupted));
        }
    }
}

#[test]
fn delete_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", ""),
            &[(K::Delete, M::NONE), (K::Enter, M::NONE)],
            ("a", ""),
        );
        assert_cursor(
            *mode,
            ("", "a"),
            &[(K::Delete, M::NONE), (K::Enter, M::NONE)],
            ("", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "a"),
                &[(K::Esc, M::NONE), (K::Delete, M::NONE), (K::Enter, M::NONE)],
                ("", ""),
            );
        }
    }
}

#[test]
fn ctrl_t() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", "b"),
            &[(K::Char('T'), M::CTRL), (K::Enter, M::NONE)],
            ("ba", ""),
        );
        assert_cursor(
            *mode,
            ("ab", "cd"),
            &[(K::Char('T'), M::CTRL), (K::Enter, M::NONE)],
            ("acb", "d"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("ab", ""),
                &[
                    (K::Esc, M::NONE),
                    (K::Char('T'), M::CTRL),
                    (K::Enter, M::NONE),
                ],
                ("ba", ""),
            );
        }
    }
}

#[test]
fn ctrl_u() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("start of line ", "end"),
            &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
            ("", "end"),
        );
        assert_cursor(
            *mode,
            ("", "end"),
            &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
            ("", "end"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("start of line ", "end"),
                &[
                    (K::Esc, M::NONE),
                    (K::Char('U'), M::CTRL),
                    (K::Enter, M::NONE),
                ],
                ("", " end"),
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn ctrl_v() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[
                (K::Char('V'), M::CTRL),
                (K::Char('\t'), M::NONE),
                (K::Enter, M::NONE),
            ],
            ("\t", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", ""),
                &[
                    (K::Esc, M::NONE),
                    (K::Char('V'), M::CTRL),
                    (K::Char('\t'), M::NONE),
                    (K::Enter, M::NONE),
                ],
                ("\t", ""),
            );
        }
    }
}

#[test]
fn ctrl_w() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[(K::Char('W'), M::CTRL), (K::Enter, M::NONE)],
            ("", "world"),
        );
        assert_cursor(
            *mode,
            ("Hello, world.", ""),
            &[(K::Char('W'), M::CTRL), (K::Enter, M::NONE)],
            ("Hello, ", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hello, world.", ""),
                &[
                    (K::Esc, M::NONE),
                    (K::Char('W'), M::CTRL),
                    (K::Enter, M::NONE),
                ],
                ("Hello, ", "."),
            );
        }
    }
}

#[test]
fn ctrl_y() {
    for mode in &[EditMode::Emacs /* FIXME, EditMode::Vi */] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[
                (K::Char('W'), M::CTRL),
                (K::Char('Y'), M::CTRL),
                (K::Enter, M::NONE),
            ],
            ("Hello, ", "world"),
        );
    }
}

#[test]
fn ctrl__() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[
                (K::Char('W'), M::CTRL),
                (K::Char('_'), M::CTRL),
                (K::Enter, M::NONE),
            ],
            ("Hello, ", "world"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hello, ", "world"),
                &[
                    (K::Esc, M::NONE),
                    (K::Char('W'), M::CTRL),
                    (K::Char('_'), M::CTRL),
                    (K::Enter, M::NONE),
                ],
                ("Hello,", " world"),
            );
        }
    }
}
