use std::cmp::Ordering;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: usize, // The leftmost column is number 0.
    pub row: usize, // The highest row is number 0.
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.row.cmp(&other.row) {
            Ordering::Equal => self.col.cmp(&other.col),
            o => o,
        }
    }
}

/// Layout of a substring of the input buffer
#[derive(Debug)]
pub struct SpanLayout {
    /// Offset of the start of the substring in the input buffer
    pub offset: usize,

    /// Position on of the start of the span
    pub pos: Position,
}

/// All positions are relative to start of the prompt == origin (col: 0, row: 0)
#[derive(Debug, Default)]
pub struct Layout {
    pub default_prompt: bool,

    /// Cursor position (relative to the start of the prompt)
    /// - cursor.row >= spans[0].pos.row
    /// - if cursor.row > spans[0].pos.row then cursor.col >= spans[0].pos.col
    pub cursor: Position,

    /// Number of rows used so far (from start of prompt to end
    /// of input or hint)
    /// - cursor <= end
    pub end: Position,

    /// Layout of the input buffer, broken into spans.
    /// - non-empty,
    /// - first element has offset 0,
    pub spans: Vec<SpanLayout>,
}

impl Layout {
    pub fn find_span_by_row(&self, row: usize) -> Option<(usize, &SpanLayout)> {
        match self.spans.binary_search_by_key(&row, |span| span.pos.row) {
            Ok(i) => Some((i, &self.spans[i])),
            Err(_) => None,
        }
    }

    /// Find the span of an offset in input buffer.
    pub fn find_span_by_offset(&self, offset: usize) -> (usize, &SpanLayout) {
        match self.spans.binary_search_by_key(&offset, |span| span.offset) {
            Ok(i) => (i, &self.spans[i]),
            Err(mut i) => {
                if i == 0 {
                    unreachable!("first span must have offset 0")
                }
                i -= 1;
                (i, &self.spans[i])
            }
        }
    }

    /// Compute layout for rendering prompt + line + some info (either hint,
    /// validation msg, ...). on the screen. Depending on screen width, line
    /// wrapping may be applied.
    pub fn compute(
        renderer: &impl crate::tty::Renderer,
        prompt_size: Position,
        default_prompt: bool,
        line: &crate::line_buffer::LineBuffer,
        info: Option<&str>,
    ) -> Layout {
        let mut spans = Vec::with_capacity(line.len());

        let buf = line.as_str();
        let cursor_offset = line.pos();

        let mut cursor = None;
        let mut curr_position = prompt_size;

        // iterate over input buffer lines
        let mut line_start_offset = 0;
        let end = loop {
            spans.push(SpanLayout {
                offset: line_start_offset,
                pos: curr_position,
            });

            // find the end of input line
            let line_end_offset = buf[line_start_offset..]
                .find('\n')
                .map_or(buf.len(), |x| x + line_start_offset);

            // find cursor position
            if line_start_offset <= cursor_offset {
                cursor = Some(
                    renderer
                        .calculate_position(&line[line_start_offset..cursor_offset], curr_position),
                );
            }

            // find end of line position
            let line_end = if cursor_offset == line_end_offset {
                // optimization
                cursor.unwrap()
            } else {
                renderer
                    .calculate_position(&line[line_start_offset..line_end_offset], curr_position)
            };

            if line_end_offset == buf.len() {
                break line_end;
            } else {
                curr_position = Position {
                    row: line_end.row + 1,
                    col: 0,
                };
                line_start_offset = line_end_offset + 1;
            }
        };
        let cursor = cursor.unwrap_or(end);

        // layout info after the input
        let end = if let Some(info) = info {
            renderer.calculate_position(info, end)
        } else {
            end
        };

        let new_layout = Layout {
            default_prompt,
            cursor,
            end,
            spans,
        };
        debug_assert!(!new_layout.spans.is_empty());
        debug_assert!(new_layout.spans[0].offset == 0);
        debug_assert!(new_layout.cursor <= new_layout.end);
        new_layout
    }
}
