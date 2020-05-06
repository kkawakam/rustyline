use std::borrow::Cow;

use crate::layout::{Meter, Position};
use crate::highlight::split_highlight;


pub struct Screen<'a> {
    buffer: &'a mut String,
    meter: Meter,
    rows: usize,
    scroll_top: usize,
}

impl<'a> Screen<'a> {
    pub fn new(buffer: &'a mut String,
        cols: usize,
        rows: usize,
        tab_stop: usize,
        scroll_top: usize,
    ) -> Screen {
        Screen {
            buffer,
            meter: Meter::new(cols, tab_stop),
            rows,
            scroll_top,
        }
    }
    fn skip_lines(&mut self, text: &str) -> usize {
        let mut bytes = 0;
        while self.meter.get_row() < self.scroll_top {
            bytes += self.meter.update_line(&text[bytes..])
                .unwrap_or(text[bytes..].len());
            if bytes >= text.len() {
                break;
            }
            self.meter.update_newline();
            if text[bytes..].starts_with('\n') {
                bytes += 1;
            }
        }
        return bytes;
    }
    pub fn get_position(&self) -> Position {
        return self.meter.get_position();
    }
    pub fn add_text(&mut self, text: &str) {
        let max_row = self.scroll_top + self.rows;
        if self.meter.get_row() >= self.scroll_top + self.rows {
            return;
        }
        let mut text = Cow::from(text);
        if self.meter.get_row() < self.scroll_top {
            let skip_bytes = self.skip_lines(&text[..]);
            let (_prefix, suffix) = split_highlight(&text[..], skip_bytes);
            text = suffix.into_owned().into();
        }
        let mut written = 0;
        while written < text.len() && self.meter.get_row() < max_row {
            if let Some(line) = self.meter.update_line(&text[written..]) {
                self.buffer.push_str(&text[written..][..line]);
                written += line;
                self.meter.update_newline();
                if text[written..].starts_with('\n') {
                    written += 1;
                }
                if self.meter.get_row() >= max_row {
                    // break before inserting a newline
                    break;
                }
                self.buffer.push('\n');
            } else {
                self.buffer.push_str(&text[written..]);
                written = text.len();
                break;
            }
        }
        if written > 0 {
            let (with_colors, _) = split_highlight(&text[..], written);
            if with_colors.len() > written {
                // Reset highlight zero-length bytes
                self.buffer.push_str(&with_colors[written..]);
            }
        }
    }
}


#[test]
fn test_scroll() {
    const TEXT: &str = "11111\n22222\n33333\n44444\n55555\n66666\n77777\n";
    const RESULTS:&[&str] = &[
        "11111\n22222\n33333\n44444",
        "22222\n33333\n44444\n55555",
        "33333\n44444\n55555\n66666",
        "44444\n55555\n66666\n77777",
        "55555\n66666\n77777\n",
    ];
    for (scroll_top, result) in RESULTS.iter().enumerate() {
        for idx in 0..TEXT.len() {
            let mut buf = String::new();
            let mut scr = Screen::new(&mut buf, 10, 4, 2, scroll_top);
            scr.add_text(&TEXT[..idx]);
            scr.add_text(&TEXT[idx..]);
            assert_eq!(&buf, result,
                "scroll: {}, iteration: {}", scroll_top, idx);
        }
    }
}

#[test]
fn test_scroll_prompt() {
    const PROMPT: &str = "\x1b[1;32m   > \x1b[0m";
    const ITEMS: &[&str] = &[
        "11111\n",
        "22222\n",
        "33333\n",
        "44444\n",
        "55555\n",
        "66666\n",
        "77777\n",
        "",
    ];
    let results = &[
        format!("{0}11111\n{0}22222\n{0}33333\n{0}44444", PROMPT),
        format!("{0}22222\n{0}33333\n{0}44444\n{0}55555", PROMPT),
        format!("{0}33333\n{0}44444\n{0}55555\n{0}66666", PROMPT),
        format!("{0}44444\n{0}55555\n{0}66666\n{0}77777", PROMPT),
        format!("{0}55555\n{0}66666\n{0}77777\n{0}", PROMPT),
    ];
    for (scroll_top, result) in results.iter().enumerate() {
        let mut buf = String::new();
        let mut scr = Screen::new(&mut buf, 10, 4, 2, scroll_top);
        for (_, text) in ITEMS.iter().enumerate() {
            scr.add_text(&PROMPT);
            scr.add_text(text);
        }
        assert_eq!(&buf, result, "scroll: {}", scroll_top);
    }
}
