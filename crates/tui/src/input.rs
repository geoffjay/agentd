use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub const PREFIX: &str = "> ";
pub const PREFIX_LEN: usize = 2;
pub const MAX_INPUT_ROWS: u16 = 20;

// ── Geometry ─────────────────────────────────────────────────────────────────

/// Total visual rows the input content occupies given `inner_width` (inside borders).
///
/// The first logical line is prefixed with "> " so its usable content width is
/// `inner_width - PREFIX_LEN`. All subsequent logical lines use the full `inner_width`.
/// Wrapping is character-level, matching `build_input_display_lines`.
pub fn compute_input_rows(buffer: &str, inner_width: usize) -> u16 {
    let first_w = inner_width.saturating_sub(PREFIX_LEN).max(1);
    buffer
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            let w = if i == 0 { first_w } else { inner_width.max(1) };
            line.chars().count().max(1).div_ceil(w) as u16
        })
        .fold(0u16, |a, r| a.saturating_add(r))
        .max(1)
}

/// Returns `(visual_row, visual_col)` for the byte cursor within the buffer.
///
/// For the very first display row of the first logical line, `visual_col` includes
/// the `PREFIX_LEN` offset (col 2 = right after "> "). All other rows report col
/// relative to the leftmost character on that display row (col 0 = first char).
pub fn cursor_visual_pos(buffer: &str, cursor: usize, inner_width: usize) -> (u16, u16) {
    let first_w = inner_width.saturating_sub(PREFIX_LEN).max(1);
    let before = &buffer[..cursor];
    let parts_before: Vec<&str> = before.split('\n').collect();
    let logical_idx = parts_before.len().saturating_sub(1);
    let col_in_logical = parts_before.last().map(|l| l.chars().count()).unwrap_or(0);

    let all_logical: Vec<&str> = buffer.split('\n').collect();
    let mut visual_row = 0u16;
    for (i, line) in all_logical.iter().enumerate().take(logical_idx) {
        let w = if i == 0 { first_w } else { inner_width.max(1) };
        let n = line.chars().count().max(1);
        visual_row = visual_row.saturating_add(n.div_ceil(w) as u16);
    }

    let line_w = if logical_idx == 0 { first_w } else { inner_width.max(1) };
    let row_within = (col_in_logical / line_w) as u16;
    visual_row = visual_row.saturating_add(row_within);
    let col_in_row = col_in_logical % line_w;
    let col =
        if logical_idx == 0 && row_within == 0 { col_in_row + PREFIX_LEN } else { col_in_row };

    (visual_row, col as u16)
}

/// Converts a visual `(target_row, target_col)` back to a byte offset in the buffer.
pub fn visual_pos_to_byte(
    buffer: &str,
    target_row: u16,
    target_col: u16,
    inner_width: usize,
) -> usize {
    let first_w = inner_width.saturating_sub(PREFIX_LEN).max(1);
    let mut current_row = 0u16;
    let mut byte_start = 0usize;

    for (li, lline) in buffer.split('\n').enumerate() {
        let w = if li == 0 { first_w } else { inner_width.max(1) };
        let char_count = lline.chars().count();
        let rows_in_line = char_count.max(1).div_ceil(w) as u16;

        if target_row >= current_row && target_row < current_row + rows_in_line {
            let row_offset = (target_row - current_row) as usize;
            let col_logical_start = row_offset * w;
            let prefix_here = if li == 0 && row_offset == 0 { PREFIX_LEN } else { 0 };
            let col_in_logical =
                col_logical_start + (target_col as usize).saturating_sub(prefix_here);
            let char_idx = col_in_logical.min(char_count);
            let byte_in_line =
                lline.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(lline.len());
            return byte_start + byte_in_line;
        }

        current_row += rows_in_line;
        byte_start += lline.len() + 1; // +1 for '\n'
    }

    buffer.len()
}

/// Moves the byte cursor up (`delta < 0`) or down (`delta > 0`) by visual rows.
/// Returns the new byte offset. If already at the first/last row, returns unchanged.
pub fn cursor_move_vertical(buffer: &str, cursor: usize, delta: i16, inner_width: usize) -> usize {
    let (row, col) = cursor_visual_pos(buffer, cursor, inner_width);
    let max_row = compute_input_rows(buffer, inner_width).saturating_sub(1);
    let target_row = ((row as i16 + delta).max(0) as u16).min(max_row);
    if target_row == row {
        return cursor;
    }
    visual_pos_to_byte(buffer, target_row, col, inner_width)
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Builds display lines for the input box with character-level wrapping.
///
/// When `cursor_byte` is `Some`, the character at the cursor position is
/// highlighted with a white background (acts as the visible cursor — no
/// terminal cursor via `set_cursor_position` is needed).
pub fn build_input_display_lines(
    buffer: &str,
    cursor_byte: Option<usize>,
    inner_width: usize,
) -> Vec<Line<'static>> {
    let cursor_pos = cursor_byte.map(|c| cursor_visual_pos(buffer, c, inner_width));
    let first_w = inner_width.saturating_sub(PREFIX_LEN).max(1);
    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut display_row = 0u16;

    for (li, lline) in buffer.split('\n').enumerate() {
        let (w, row_prefix) = if li == 0 { (first_w, PREFIX) } else { (inner_width.max(1), "") };
        let chars: Vec<char> = lline.chars().collect();
        let total = chars.len();
        let mut pos = 0usize;

        loop {
            let end = (pos + w).min(total);
            let chunk: &[char] = &chars[pos..end];
            let prefix_for_row = if pos == 0 { row_prefix } else { "" };
            let row_str: String = format!("{}{}", prefix_for_row, chunk.iter().collect::<String>());

            let line = match cursor_pos {
                Some((crow, ccol)) if crow == display_row => {
                    styled_row_with_cursor(row_str, ccol as usize, cursor_style)
                }
                _ => Line::from(row_str),
            };
            result.push(line);
            display_row += 1;
            pos = end;

            if pos >= total {
                break;
            }
        }
    }

    if result.is_empty() {
        // Shouldn't happen since split('\n') always yields at least one element,
        // but guard anyway.
        result.push(Line::from(PREFIX.to_string()));
    }

    result
}

/// Splits `row_str` at `ccol` and styles the character at that position.
fn styled_row_with_cursor(row_str: String, ccol: usize, cursor_style: Style) -> Line<'static> {
    let chars: Vec<char> = row_str.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::new();

    if ccol > 0 {
        let before: String = chars[..ccol.min(chars.len())].iter().collect();
        spans.push(Span::raw(before));
    }

    let cursor_ch = if ccol < chars.len() {
        chars[ccol].to_string()
    } else {
        " ".to_string() // cursor past end of content
    };
    spans.push(Span::styled(cursor_ch, cursor_style));

    if ccol + 1 < chars.len() {
        let after: String = chars[ccol + 1..].iter().collect();
        spans.push(Span::raw(after));
    }

    Line::from(spans)
}
