//! History related commands tests
use super::assert_history;
use crate::config::EditMode;
use crate::keys::{KeyCode as K, Modifiers as M};

#[test]
fn down_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &["line1"],
            &[(K::Down, M::NONE), (K::Enter, M::NONE)],
            "",
            ("", ""),
        );
        assert_history(
            *mode,
            &["line1", "line2"],
            &[(K::Up, M::NONE), (K::Up, M::NONE), (K::Down, M::NONE), (K::Enter, M::NONE)],
            "",
            ("line2", ""),
        );
        assert_history(
            *mode,
            &["line1"],
            &[
                (K::Char('a'), M::NONE),
                (K::Up, M::NONE),
                (K::Down, M::NONE), // restore original line
                (K::Enter, M::NONE),
            ],
            "",
            ("a", ""),
        );
        assert_history(
            *mode,
            &["line1"],
            &[
                (K::Char('a'), M::NONE),
                (K::Down, M::NONE), // noop
                (K::Enter, M::NONE),
            ],
            "",
            ("a", ""),
        );
    }
}

#[test]
fn up_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(*mode, &[], &[(K::Up, M::NONE), (K::Enter, M::NONE)], "", ("", ""));
        assert_history(
            *mode,
            &["line1"],
            &[(K::Up, M::NONE), (K::Enter, M::NONE)],
            "",
            ("line1", ""),
        );
        assert_history(
            *mode,
            &["line1", "line2"],
            &[(K::Up, M::NONE), (K::Up, M::NONE), (K::Enter, M::NONE)],
            "",
            ("line1", ""),
        );
    }
}

#[test]
fn ctrl_r() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &[],
            &[(K::Char('R'), M::CTRL), (K::Char('o'), M::NONE), (K::Enter, M::NONE)],
            "",
            ("o", ""),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                (K::Char('R'), M::CTRL),
                (K::Char('o'), M::NONE),
                (K::Right, M::NONE), // just to assert cursor pos
                (K::Enter, M::NONE),
            ],
            "",
            ("cargo", ""),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                (K::Char('R'), M::CTRL),
                (K::Char('u'), M::NONE),
                (K::Right, M::NONE), // just to assert cursor pos
                (K::Enter, M::NONE),
            ],
            "",
            ("ru", "stc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                (K::Char('R'), M::CTRL),
                (K::Char('r'), M::NONE),
                (K::Char('u'), M::NONE),
                (K::Right, M::NONE), // just to assert cursor pos
                (K::Enter, M::NONE),
            ],
            "",
            ("r", "ustc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                (K::Char('R'), M::CTRL),
                (K::Char('r'), M::NONE),
                (K::Char('R'), M::CTRL),
                (K::Right, M::NONE), // just to assert cursor pos
                (K::Enter, M::NONE),
            ],
            "",
            ("r", "ustc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                (K::Char('R'), M::CTRL),
                (K::Char('r'), M::NONE),
                (K::Char('z'), M::NONE), // no match
                (K::Right, M::NONE),     // just to assert cursor pos
                (K::Enter, M::NONE),
            ],
            "",
            ("car", "go"),
        );
        assert_history(
            EditMode::Emacs,
            &["rustc", "cargo"],
            &[
                (K::Char('a'), M::NONE),
                (K::Char('R'), M::CTRL),
                (K::Char('r'), M::NONE),
                (K::Char('G'), M::CTRL), // abort (FIXME: doesn't work with vi mode)
                (K::Enter, M::NONE),
            ],
            "",
            ("a", ""),
        );
    }
}

#[test]
fn ctrl_r_with_long_prompt() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[(K::Char('R'), M::CTRL), (K::Char('o'), M::NONE), (K::Enter, M::NONE)],
            ">>>>>>>>>>>>>>>>>>>>>>>>>>> ",
            ("cargo", ""),
        );
    }
}

#[test]
fn ctrl_s() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                (K::Char('R'), M::CTRL),
                (K::Char('r'), M::NONE),
                (K::Char('R'), M::CTRL),
                (K::Char('S'), M::CTRL),
                (K::Right, M::NONE), // just to assert cursor pos
                (K::Enter, M::NONE),
            ],
            "",
            ("car", "go"),
        );
    }
}

#[test]
fn meta_lt() {
    assert_history(
        EditMode::Emacs,
        &[""],
        &[(K::Char('<'), M::ALT), (K::Enter, M::NONE)],
        "",
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[(K::Char('<'), M::ALT), (K::Enter, M::NONE)],
        "",
        ("rustc", ""),
    );
}

#[test]
fn meta_gt() {
    assert_history(
        EditMode::Emacs,
        &[""],
        &[(K::Char('>'), M::ALT), (K::Enter, M::NONE)],
        "",
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[(K::Char('<'), M::ALT), (K::Char('>'), M::ALT), (K::Enter, M::NONE)],
        "",
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[
            (K::Char('a'), M::NONE),
            (K::Char('<'), M::ALT),
            (K::Char('>'), M::ALT), // restore original line
            (K::Enter, M::NONE),
        ],
        "",
        ("a", ""),
    );
}
