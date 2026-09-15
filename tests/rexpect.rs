#![cfg(all(unix, not(feature = "signal-hook")))]

use rexpect::{
    error::Error,
    process::Signal,
    session::{spawn_command, PtySession},
};
use std::process::Command;

fn wrap(f: fn(&mut PtySession) -> Result<(), Error>, styled: bool, eof: &str) -> Result<(), Error> {
    let bin = env!("CARGO_BIN_EXE_re");
    let mut cmd = Command::new(bin);
    if styled {
        cmd.arg("-s");
    }
    let mut p = spawn_command(cmd, Some(2_000))?;
    p.exp_string("\x1b[?2004h")?; // bracketed paste on
    prompt(&mut p, styled)?;
    f(&mut p)?;
    p.exp_string("\x1b[?2004l")?; // bracketed paste off
    p.exp_string("\r\n")?; // writeln
    assert_eq!(p.exp_eof()?, eof);
    Ok(())
}

fn prompt(p: &mut PtySession, styled: bool) -> Result<(), Error> {
    p.exp_string("\x1b[?2026h")?; // synchronized
    if styled {
        p.exp_string("\x1b[1;32m> \x1b[0m")?; // prompt
    } else {
        p.exp_string("> ")?; // prompt
    }
    assert_eq!(p.exp_string("\r\x1b[2C")?, ""); // move cursor
    assert_eq!(p.exp_string("\x1b[?2026l")?, ""); // synchronized
    Ok(())
}

fn enter(p: &mut PtySession) -> Result<(), Error> {
    p.send_control('m')
}
fn backspace(p: &mut PtySession) -> Result<(), Error> {
    send_control_exp(p, 'h', "\x1b[D\x1b[K")
}
fn send_control_exp(p: &mut PtySession, c: char, exp: &str) -> Result<(), Error> {
    p.send_control(c)?;
    assert_eq!(p.exp_string(exp)?, "");
    Ok(())
}
fn send(p: &mut PtySession, s: &str) -> Result<(), Error> {
    p.send(s)?;
    p.flush()
}
fn send_exp(p: &mut PtySession, s: &str, exp: &str) -> Result<(), Error> {
    send(p, s)?;
    assert_eq!(p.exp_string(exp)?, "");
    Ok(())
}

#[test]
fn hello() -> Result<(), Error> {
    wrap(
        |p| {
            p.send_line("hello")?;
            p.exp_string("hello").map(|_| ())
        },
        false,
        "",
    )
}

#[test]
fn styled() -> Result<(), Error> {
    wrap(|p| enter(p), true, "")
}

#[test]
fn eof() -> Result<(), Error> {
    wrap(|p| p.send_control('d'), false, "Error: Eof\r\n")
}

#[test]
fn interrupt() -> Result<(), Error> {
    wrap(|p| p.send_control('c'), false, "Error: Interrupted\r\n")
}

#[test]
fn sigint() -> Result<(), Error> {
    wrap(
        |p| p.process_mut().signal(Signal::SIGINT),
        false,
        "Error: Interrupted\r\n",
    )
}

#[test]
fn sigwinch() -> Result<(), Error> {
    wrap(
        |p| {
            p.process_mut().signal(Signal::SIGWINCH)?;
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn clear_screen() -> Result<(), Error> {
    wrap(
        |p| {
            p.send_control('l')?;
            p.exp_string("\x1b[H\x1b[J")?;
            prompt(p, false)?;
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn control() -> Result<(), Error> {
    wrap(
        |p| {
            send_exp(p, "he", "he")?;
            //p.send_control('@')?;
            send_control_exp(p, 'a', "\x1b[2D")?; // home
            send_control_exp(p, 'e', "\x1b[2C")?; // end
            send_control_exp(p, 'u', "\x1b[2D\x1b[K")?;
            send_exp(p, "he", "he")?;
            send_control_exp(p, 'b', "\x1b[D")?; // left
            send_control_exp(p, 'k', "\x1b[K")?; // kill
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn rxvt() -> Result<(), Error> {
    wrap(
        |p| {
            send_exp(p, "he", "he")?;
            send_exp(p, "\x1b[1~", "\x1b[2D")?; // home
            send_exp(p, "\x1b[4~", "\x1b[2C")?; // end
            send_exp(p, "\x1b[7~", "\x1b[2D")?; // home
            send_exp(p, "\x1b[8~", "\x1b[2C")?; // end
            backspace(p)?;
            send_exp(p, "\x1b[7~", "\x1b[D")?; // home
            send_exp(p, "\x1b[3~", "\x1b[K")?; // delete
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn esc_esc() -> Result<(), Error> {
    wrap(
        |p| {
            send(p, "\x1b\x1b[D")?; // Alt-left
            send(p, "\x1b\x1bOD")?; // Alt-left
            send(p, "\x1b\x1b\x1b")?; // Esc
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn linux_console() -> Result<(), Error> {
    wrap(
        |p| {
            send(p, "\x1b[[A")?; // F1
            send(p, "\x1b[[B")?; // F2
            send(p, "\x1b[[C")?; // F3
            send(p, "\x1b[[D")?; // F4
            send(p, "\x1b[[E")?; // F5
            send(p, "\x1b[[F")?; // unknown
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn ansi() -> Result<(), Error> {
    wrap(
        |p| {
            send_exp(p, "h", "h")?;
            send_exp(p, "\x1b[D", "\x1b[D")?; // left
            send(p, "\x1b[D")?; // noop
            send_exp(p, "\x1b[C", "\x1b[C")?; // right
            send(p, "\x1b[C")?; // noop

            send_exp(p, "e", "e")?;
            send_exp(p, "\x1b[H", "\x1b[2D")?; // home
            send(p, "\x1b[H")?; // noop
            send_exp(p, "\x1b[F", "\x1b[2C")?; // end
            send(p, "\x1b[F")?; // noop

            send(p, "\x1b[A")?; // up
            send(p, "\x1b[B")?; // down
            send(p, "\x1b[Z")?; // backtab
            send(p, "\x1b[E")?; // unknown
            enter(p)
        },
        false,
        "",
    )
}

#[test]
fn esc_o() -> Result<(), Error> {
    wrap(
        |p| {
            //send(p, "\x1bO")?; // Alt-O
            send_exp(p, "h", "h")?;
            send_exp(p, "\x1bOD", "\x1b[D")?; // left
            send(p, "\x1bOD")?; // noop
            send_exp(p, "\x1bOC", "\x1b[C")?; // right
            send(p, "\x1bOC")?; // noop

            send_exp(p, "e", "e")?;
            send_exp(p, "\x1bOH", "\x1b[2D")?; // home
            send(p, "\x1bOH")?; // noop
            send_exp(p, "\x1bOF", "\x1b[2C")?; // end
            send(p, "\x1bOF")?; // noop

            send(p, "\x1bOA")?; // up
            send(p, "\x1bOB")?; // down
            send(p, "\x1bOZ")?; // unknown
            send(p, "\x1bOM") // enter
        },
        false,
        "",
    )
}
