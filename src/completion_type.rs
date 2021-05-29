pub use crate::binding::{ConditionalEventHandler, Event, EventContext, EventHandler};
use crate::completion::{longest_common_prefix, Candidate, Completer};
pub use crate::config::{
    ColorMode, CompletionType, Config, EditMode, HistoryDuplicates, OutputStreamType,
};
use crate::edit::State;
use crate::error;
pub use crate::keymap::{Anchor, At, CharSearch, Cmd, InputMode, Movement, RepeatCount, Word};
use crate::keymap::{InputState, Refresher};
pub use crate::keys::{KeyCode, KeyEvent, Modifiers};
use crate::tty::{Renderer, Term, Terminal};
use crate::Helper;
use std::cmp;
use std::result;
use unicode_width::UnicodeWidthStr;

/// The error type for I/O and Linux Syscalls (Errno)
type Result<T> = result::Result<T, error::ReadlineError>;

struct CircularListHelper {
    circular_list: String,
    num_rows_including_linewraps: usize,
    term_cols: usize,
    term_rows: usize,
    prompt_rows: usize,
    circular_list_rows: usize,
}

impl CircularListHelper {
    fn exists(&self) -> bool {
        !self.circular_list.is_empty()
    }

    /*
     * Show completion_candidates. Example:
     *   |> test
     *   test0 >test1 test2
     */
    fn build_and_render_block<H: Helper, C: Candidate>(
        &mut self,
        s: &mut State<'_, '_, H>,
        candidates: &[C],
        idx: usize,
        start: usize,
    ) -> Result<()> {
        let candidate = candidates[idx].replacement();
        s.line.replace(start..s.line.pos(), &candidate);
        s.refresh_line()?;

        let rows_in_prompt = s.layout.end.row + 1;

        // Problems occur through changing layouts
        // (resizing, change of number of rows_in_prompt)
        // Fix through clearing the block
        if self.term_cols != s.out.get_columns()
            || self.term_rows != s.out.get_rows()
            || self.prompt_rows != rows_in_prompt
            || self.num_rows_including_linewraps > self.circular_list_rows
        {
            self.term_cols = s.out.get_columns();
            self.term_rows = s.out.get_rows();
            self.clear_block(s)?;
        }

        let circular_list_rows = s.out.get_rows().saturating_sub(rows_in_prompt);
        // Build the CircularList
        let (mut circular_list, num_rows_including_linewraps) = circular_list(
            &ListLayout {
                columns: s.out.get_columns(),
                rows: circular_list_rows,
                index: idx,
            },
            &candidates,
        );

        // The list should always start in a new line
        circular_list.insert(0, '\n');
        self.prompt_rows = rows_in_prompt;
        self.circular_list_rows = circular_list_rows;
        self.circular_list = circular_list;
        self.num_rows_including_linewraps = num_rows_including_linewraps;
        self.render_block(s)?;
        Ok(())
    }

    fn render_block<H: Helper>(&mut self, s: &mut State<'_, '_, H>) -> Result<()> {
        if !self.exists() {
            return Ok(());
        }

        // Display the CircularList after the prompt + line
        let pos_end_of_line = s
            .out
            .calculate_position(&s.line[s.line.pos()..], s.layout.cursor);
        s.out.move_cursor(s.layout.cursor, pos_end_of_line)?;
        s.out.write_and_flush(self.circular_list.as_bytes())?;
        let pos_end_of_block = s
            .out
            .calculate_position(&self.circular_list, pos_end_of_line);
        s.out.move_cursor(pos_end_of_block, pos_end_of_line)?;
        s.out.move_cursor(pos_end_of_line, s.layout.cursor)?;
        Ok(())
    }

    fn clear_block<H: Helper>(&mut self, s: &mut State<'_, '_, H>) -> Result<()> {
        if !self.exists() {
            return Ok(());
        }

        let pos_end_of_line = s
            .out
            .calculate_position(&s.line[s.line.pos()..], s.layout.cursor);
        s.out.move_cursor(s.layout.cursor, pos_end_of_line)?;
        s.out.clear_screen_from_cursor_down()?;
        s.out.move_cursor(pos_end_of_line, s.layout.cursor)?;

        self.circular_list = String::new();
        Ok(())
    }
}

struct ListLayout {
    columns: usize,
    rows: usize,
    index: usize,
}

fn circular_list<C: Candidate>(list_layout: &ListLayout, candidates: &[C]) -> (String, usize) {
    let min_col_pad = 2;
    let cols = list_layout.columns;
    let available_rows = list_layout.rows;
    if available_rows == 0 || cols == 0 {
        return (String::new(), 0);
    }
    let max_candidate_width = candidates
        .iter()
        .map(|s| s.display().width())
        .max()
        .unwrap_or(cols)
        + min_col_pad;
    let max_width = cmp::min(cols, max_candidate_width);
    let num_cols = cols / max_width;
    let num_rows = (candidates.len() + num_cols - 1) / num_cols;
    let mut circ_list = String::with_capacity(num_rows * cols + 16);
    let mut row_off_outside_index = 0;
    let mut num_rows_including_linewraps = 0;

    // Prevent us from doing to much work by calculating begin and end
    // -1 to ensure that the list_layout.index can reach the end of the available_rows
    let begin = (list_layout.index % num_rows).saturating_sub(available_rows.saturating_sub(1));
    let end = cmp::min(begin.saturating_add(available_rows), num_rows);

    for row in begin..end {
        let mut _fill_line = cols;
        num_rows_including_linewraps += 1;
        // Print the appropriate member of each column into our current row
        for col in 0..num_cols {
            let idx = (col * num_rows) + row;
            if idx < candidates.len() {
                let candidate = &candidates[idx];
                let width = candidate.display().width() + 1;
                let candidate_width = width + max_width.saturating_sub(width);
                num_rows_including_linewraps += candidate_width.saturating_sub(1) / cols;
                if list_layout.index == idx {
                    circ_list.push('>');
                    row_off_outside_index = num_rows_including_linewraps;
                } else {
                    circ_list.push(' ');
                }
                circ_list.push_str(candidate.display());
                (width..max_width).for_each(|_| circ_list.push(' '));
            }
        }
        circ_list.push('\n');
    }

    // CircularList is finished
    // At this point it might still have to many rows to be displayed in the terminal without glitches
    if num_rows_including_linewraps > available_rows {
        // If highlighted member is outside of the displayable list
        // delete the appropriate amount at the front
        if row_off_outside_index >= available_rows {
            let distance_beyond_window = row_off_outside_index.saturating_sub(available_rows);

            if let Some(line_break_num) = circ_list
                .lines()
                .enumerate()
                .scan(0, |rows_including_overlength, (line_num, line)| {
                    if *rows_including_overlength >= distance_beyond_window {
                        *rows_including_overlength += 1 + (line.width().saturating_sub(1) / cols);
                        None
                    } else {
                        *rows_including_overlength += 1 + (line.width().saturating_sub(1) / cols);
                        Some(line_num)
                    }
                })
                .last()
            {
                if let Some((delete_end_point, _)) =
                    circ_list.match_indices('\n').nth(line_break_num)
                {
                    circ_list.replace_range(0..delete_end_point.saturating_add(1), "");
                }
            }
        }
        // Ensure that the CircularList isn't to long to be displayed
        // -> delete the rest at the end (take linewrapping into account)
        if let Some(line_break_num) = circ_list
            .lines()
            .enumerate()
            .scan(0, |rows_including_overlength, (line_num, line)| {
                *rows_including_overlength += 1 + (line.width().saturating_sub(1) / cols);
                if *rows_including_overlength > available_rows {
                    None
                } else {
                    Some(line_num)
                }
            })
            .last()
        {
            if let Some((delete_start_point, _)) = circ_list.match_indices('\n').nth(line_break_num)
            {
                circ_list.replace_range(delete_start_point + 1.., "");
            }
        } else {
            circ_list.clear();
        }
    }
    // At this point CircularList is filled and has the right length
    // now remove the last \n to not give the user a mandatory newline
    circ_list.pop();
    (circ_list, num_rows_including_linewraps)
}

pub(crate) fn circular_completion_list_loop<H: Helper, He: Helper>(
    rdr: &mut <Terminal as Term>::Reader,
    s: &mut State<'_, '_, H>,
    input_state: &mut InputState,
    completer: He,
    start: usize,
    candidates: Vec<<He as Completer>::Candidate>,
) -> Result<Option<Cmd>> {
    let mark = s.changes.borrow_mut().begin();
    // Save the current edited line before overwriting it
    // We need to insert a ' ' to prevent a glitch, in the case that the cursor is at
    // exactly the end of the line.
    // Remove the extra ' ' before returning
    let prev_pos = s.line.pos();
    s.line.insert(' ', 1);
    s.line.set_pos(prev_pos);
    let mut backup = s.line.as_str().to_owned();
    let mut backup_pos = s.line.pos();
    let mut cmd;
    let mut idx: usize = 0;
    let mut block_at_the_end = CircularListHelper {
        circular_list: String::new(),
        num_rows_including_linewraps: 0,
        circular_list_rows: 0,
        term_cols: s.out.get_columns(),
        term_rows: s.out.get_rows(),
        prompt_rows: s.layout.end.row + 1,
    };

    // Show longest common prefix immediatly
    if let Some(lcp) = longest_common_prefix(&candidates) {
        // if we can extend the item, extend it
        if lcp.len() > s.line.pos() - start {
            completer.update(&mut s.line, start, lcp);
            s.refresh_line()?;
        }
    }

    let mut candidates = candidates;

    'reload_completer: loop {
        // We can't complete any further, wait for second tab
        cmd = s.next_cmd(input_state, rdr, true)?;

        // If any character other than TAB, pass it to the main loop
        if !matches!(cmd, Cmd::Complete) {
            break 'reload_completer;
        }

        // If TAB was pressed (or SPACE in the match block below) update the completion_candidates
        // Only update candidates if the start hasn't changed. A larger start may lead to a panic
        let (sta, cand) = completer.complete(&s.line, s.line.pos(), &s.ctx)?;
        if start == sta {
            candidates = cand;
            idx = 0;
        }

        // Circular behavior loop
        loop {
            // Show completion or original (backup) buffer
            if idx < candidates.len() {
                block_at_the_end.build_and_render_block(s, &candidates, idx, start)?;
            } else {
                // Restore current edited line
                s.line.update(&backup, backup_pos);
                s.refresh_line()?;
                block_at_the_end.clear_block(s)?;
            }

            cmd = s.next_cmd(input_state, rdr, true)?;

            match cmd {
                Cmd::Complete => {
                    idx = (idx + 1) % (candidates.len() + 1); // Circular
                    if idx == candidates.len() {
                        s.out.beep()?;
                    }
                }
                Cmd::CompleteBackward => {
                    if idx == 0 {
                        idx = candidates.len(); // Circular
                        s.out.beep()?;
                    } else {
                        idx = (idx - 1) % (candidates.len() + 1); // Circular
                    }
                }
                Cmd::Abort => {
                    // Re-show original buffer
                    if idx < candidates.len() {
                        s.line.update(&backup, backup_pos);
                        s.refresh_line()?;
                    }
                    block_at_the_end.clear_block(s)?;

                    // Prevent glitch if multiline and the cursor at the edge of the screen in vi mode
                    if matches!(s.line.as_bytes().get(s.line.pos()), Some(b' ')) {
                        s.line.delete_range(s.line.pos()..s.line.pos() + 1);
                        s.refresh_line()?;
                    }
                    s.changes.borrow_mut().truncate(mark);
                    return Ok(None);
                }
                Cmd::SelfInsert(1, ' ') => {
                    // SPACE confirms an element from the displayed CircularList
                    block_at_the_end.clear_block(s)?;

                    match s
                        .line
                        .as_bytes()
                        .get(s.line.pos().saturating_sub(1))
                        .map(|b| *b as char)
                    {
                        Some('/') | Some('\\') => {
                            backup = s.line.as_str().to_owned();
                            backup_pos = s.line.pos();
                            continue 'reload_completer;
                        }
                        _ => {
                            break 'reload_completer;
                        }
                    }
                }
                _ => {
                    block_at_the_end.clear_block(s)?;
                    break 'reload_completer;
                }
            }
        }
    }
    // Prevent glitch if multiline and the cursor at the edge of the screen in vi mode
    if !matches!(cmd, Cmd::Move(Movement::BackwardChar(1)))
        && matches!(s.line.as_bytes().get(s.line.pos()), Some(b' '))
    {
        s.line.delete_range(s.line.pos()..s.line.pos() + 1);
        s.refresh_line()?;
    }
    s.changes.borrow_mut().end();
    Ok(Some(cmd))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circular_list_test() {
        macro_rules! circular_list_tester {
            ($cols:expr, $rows:expr, $index:expr, $candidates:expr => $result:expr) => {
                let (circ_list, _num_rows_including_linewraps) = circular_list(
                    &ListLayout {
                        columns: $cols,
                        rows: $rows,
                        index: $index,
                    },
                    &$candidates,
                );
                assert!(circ_list == $result);
            };
        }

        let candidates = [
            "test-0".to_string(),
            "test-1".to_string(),
            "test-2".to_string(),
            "test-3".to_string(),
            "test-4".to_string(),
            "test-5".to_string(),
            "test-6".to_string(),
            "test-7".to_string(),
        ];
        circular_list_tester!(8,8,0,candidates => ">test-0 \n test-1 \n test-2 \n test-3 \n test-4 \n test-5 \n test-6 \n test-7 ");
        circular_list_tester!(8,8,1,candidates => " test-0 \n>test-1 \n test-2 \n test-3 \n test-4 \n test-5 \n test-6 \n test-7 ");
        circular_list_tester!(8,8,6,candidates => " test-0 \n test-1 \n test-2 \n test-3 \n test-4 \n test-5 \n>test-6 \n test-7 ");
        circular_list_tester!(8,8,7,candidates => " test-0 \n test-1 \n test-2 \n test-3 \n test-4 \n test-5 \n test-6 \n>test-7 ");
        circular_list_tester!(8,4,0,candidates => ">test-0 \n test-1 \n test-2 \n test-3 ");
        circular_list_tester!(8,4,6,candidates => " test-3 \n test-4 \n test-5 \n>test-6 ");
        circular_list_tester!(8,4,7,candidates => " test-4 \n test-5 \n test-6 \n>test-7 ");
        circular_list_tester!(4,8,0,candidates => ">test-0\n test-1\n test-2\n test-3");
        circular_list_tester!(4,8,7,candidates => " test-4\n test-5\n test-6\n>test-7");
        circular_list_tester!(8,1,0,candidates => ">test-0 ");
        circular_list_tester!(8,1,7,candidates => ">test-7 ");
        circular_list_tester!(7,1,0,candidates => ">test-0");
        circular_list_tester!(7,1,7,candidates => ">test-7");
        circular_list_tester!(1,7,0,candidates => ">test-0");
        circular_list_tester!(1,8,7,candidates => ">test-7");
        circular_list_tester!(1,8,0,candidates => ">test-0");
        circular_list_tester!(1,7,7,candidates => ">test-7");
        circular_list_tester!(1,6,0,candidates => "");
        circular_list_tester!(6,1,0,candidates => "");
        circular_list_tester!(8,0,0,candidates => "");
        circular_list_tester!(0,8,0,candidates => "");
    }
}
