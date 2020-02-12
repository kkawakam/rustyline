//! History related commands tests
use super::assert_history;
use crate::config::EditMode;
use crate::keys::KeyPress;

#[test]
fn down_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &["line1"],
            &[KeyPress::DOWN, KeyPress::ENTER],
            "",
            ("", ""),
        );
        assert_history(
            *mode,
            &["line1", "line2"],
            &[KeyPress::UP, KeyPress::UP, KeyPress::DOWN, KeyPress::ENTER],
            "",
            ("line2", ""),
        );
        assert_history(
            *mode,
            &["line1"],
            &[
                KeyPress::from('a'),
                KeyPress::UP,
                KeyPress::DOWN, // restore original line
                KeyPress::ENTER,
            ],
            "",
            ("a", ""),
        );
        assert_history(
            *mode,
            &["line1"],
            &[
                KeyPress::from('a'),
                KeyPress::DOWN, // noop
                KeyPress::ENTER,
            ],
            "",
            ("a", ""),
        );
    }
}

#[test]
fn up_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(*mode, &[], &[KeyPress::UP, KeyPress::ENTER], "", ("", ""));
        assert_history(
            *mode,
            &["line1"],
            &[KeyPress::UP, KeyPress::ENTER],
            "",
            ("line1", ""),
        );
        assert_history(
            *mode,
            &["line1", "line2"],
            &[KeyPress::UP, KeyPress::UP, KeyPress::ENTER],
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
            &[KeyPress::ctrl('R'), KeyPress::from('o'), KeyPress::ENTER],
            "",
            ("o", ""),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::ctrl('R'),
                KeyPress::from('o'),
                KeyPress::RIGHT, // just to assert cursor pos
                KeyPress::ENTER,
            ],
            "",
            ("cargo", ""),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::ctrl('R'),
                KeyPress::from('u'),
                KeyPress::RIGHT, // just to assert cursor pos
                KeyPress::ENTER,
            ],
            "",
            ("ru", "stc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::ctrl('R'),
                KeyPress::from('r'),
                KeyPress::from('u'),
                KeyPress::RIGHT, // just to assert cursor pos
                KeyPress::ENTER,
            ],
            "",
            ("r", "ustc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::ctrl('R'),
                KeyPress::from('r'),
                KeyPress::ctrl('R'),
                KeyPress::RIGHT, // just to assert cursor pos
                KeyPress::ENTER,
            ],
            "",
            ("r", "ustc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::ctrl('R'),
                KeyPress::from('r'),
                KeyPress::from('z'), // no match
                KeyPress::RIGHT,     // just to assert cursor pos
                KeyPress::ENTER,
            ],
            "",
            ("car", "go"),
        );
        assert_history(
            EditMode::Emacs,
            &["rustc", "cargo"],
            &[
                KeyPress::from('a'),
                KeyPress::ctrl('R'),
                KeyPress::from('r'),
                KeyPress::ctrl('G'), // abort (FIXME: doesn't work with vi mode)
                KeyPress::ENTER,
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
            &[KeyPress::ctrl('R'), KeyPress::from('o'), KeyPress::ENTER],
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
                KeyPress::ctrl('R'),
                KeyPress::from('r'),
                KeyPress::ctrl('R'),
                KeyPress::ctrl('S'),
                KeyPress::RIGHT, // just to assert cursor pos
                KeyPress::ENTER,
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
        &[KeyPress::meta('<'), KeyPress::ENTER],
        "",
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[KeyPress::meta('<'), KeyPress::ENTER],
        "",
        ("rustc", ""),
    );
}

#[test]
fn meta_gt() {
    assert_history(
        EditMode::Emacs,
        &[""],
        &[KeyPress::meta('>'), KeyPress::ENTER],
        "",
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[KeyPress::meta('<'), KeyPress::meta('>'), KeyPress::ENTER],
        "",
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[
            KeyPress::from('a'),
            KeyPress::meta('<'),
            KeyPress::meta('>'), // restore original line
            KeyPress::ENTER,
        ],
        "",
        ("a", ""),
    );
}
