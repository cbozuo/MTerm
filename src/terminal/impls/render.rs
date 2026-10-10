use unicode_width::UnicodeWidthChar;

use crate::terminal::{HistSpan, Line};

/// How much terminal byte history is retained for resize reflow.
pub(crate) const RAW_CAP: usize = 2 * 1024 * 1024;
/// Per-session rendered scrollback cap.
///
/// 2026-10-02: 100_000 → 20_000（内存）。用户实测单进程 311 MB。
/// 一行的代价不是"一段文本"——`Line = (String, Vec<HistSpan>, bool)`
/// （`struct/state.rs`），整行文本一份 + **每个彩色片段各自一份 String**。
/// 200 列、带几个颜色段 ≈ 700 B/行（含分配器开销），于是
///   100_000 行 ≈ **70 MB / 会话**；4 个会话就上 300 MB。
/// 20_000 行 ≈ 14 MB/会话，且仍能完整装下一次 `cargo build` 的日志（3–8k 行）。
/// 主流终端默认都在 1k–10k（Windows Terminal 9999 / Alacritty 10000），本值已是它们的2 倍。
///
/// 硬上限在 `term_buffer.rs` 的 `while history.len() > MAX_HISTORY { pop_front }`，
/// 内存因此有天花板、不会无限涨；连接关闭时 `release_scrollback()` 会把 history/raw 全清空。
/// 要放宽只需改这一个常量。
pub(crate) const MAX_HISTORY: usize = 20_000;

pub(crate) fn cell_prefix(chars: &[char]) -> Vec<usize> {
    let mut prefix = Vec::with_capacity(chars.len() + 1);
    let mut accumulated = 0usize;
    for &character in chars {
        prefix.push(accumulated);
        accumulated += character.width().unwrap_or(0);
    }
    prefix.push(accumulated);
    prefix
}

pub(crate) fn char_at_cell_start(prefix: &[usize], target: usize) -> usize {
    let char_count = prefix.len().saturating_sub(1);
    for index in 0..char_count {
        if prefix[index] <= target && target < prefix[index + 1] {
            return index;
        }
    }
    char_count
}

pub(crate) fn char_after_cell_end(prefix: &[usize], target: usize) -> usize {
    let char_count = prefix.len().saturating_sub(1);
    for index in 0..char_count {
        if prefix[index] > target {
            return index;
        }
    }
    char_count
}

fn cell_attrs(
    screen: &vt100::Screen,
    row: u16,
    column: u16,
) -> (String, vt100::Color, vt100::Color, bool, bool, bool) {
    match screen.cell(row, column) {
        Some(cell) => {
            let contents = cell.contents();
            let contents = if cell.is_wide_continuation() {
                String::new()
            } else if contents.is_empty() {
                " ".to_string()
            } else {
                contents.to_string()
            };
            (
                contents,
                cell.fgcolor(),
                cell.bgcolor(),
                cell.bold(),
                cell.is_wide(),
                cell.inverse(),
            )
        }
        None => (
            " ".to_string(),
            vt100::Color::Default,
            vt100::Color::Default,
            false,
            false,
            false,
        ),
    }
}

pub(crate) fn build_row(screen: &vt100::Screen, row: u16, columns: u16) -> Line {
    let mut plain = String::with_capacity(columns as usize);
    let mut runs = Vec::new();
    let mut column = 0u16;
    while column < columns {
        let (contents, foreground, background, bold, wide, inverse) =
            cell_attrs(screen, row, column);
        if wide {
            plain.push_str(&contents);
            runs.push(HistSpan {
                text: contents,
                fg: foreground,
                bg: background,
                bold,
                inverse,
                col: column as i32,
                cells: 2,
            });
            column += 2;
            continue;
        }

        let start_column = column;
        let mut text = contents.clone();
        plain.push_str(&contents);
        column += 1;
        while column < columns {
            let (next, next_fg, next_bg, next_bold, next_wide, next_inverse) =
                cell_attrs(screen, row, column);
            if next_wide
                || next_fg != foreground
                || next_bg != background
                || next_bold != bold
                || next_inverse != inverse
            {
                break;
            }
            plain.push_str(&next);
            text.push_str(&next);
            column += 1;
        }

        let cells = (column - start_column) as i32;
        let invisible_default_blank = text.chars().all(|character| character == ' ')
            && matches!(background, vt100::Color::Default)
            && !inverse;
        if !invisible_default_blank {
            runs.push(HistSpan {
                text,
                fg: foreground,
                bg: background,
                bold,
                inverse,
                col: start_column as i32,
                cells,
            });
        }
    }
    (plain, runs, screen.row_wrapped(row))
}

pub(crate) fn detect_scroll(previous: &[Line], current: &[Line]) -> usize {
    let mut best_shift = 0usize;
    let mut best_length = 0usize;
    for shift in 0..previous.len() {
        let mut length = 0usize;
        while shift + length < previous.len()
            && length < current.len()
            && previous[shift + length].0 == current[length].0
        {
            length += 1;
        }
        if length > best_length {
            best_length = length;
            best_shift = shift;
        }
    }
    best_shift
}
