//! Display-only transcript timestamps. The stored messages, tool results,
//! and daemon wire frames remain unchanged.
use crate::{Line, Span};

pub(crate) const PREFIX_WIDTH: usize = 24;

/// The last few converted seconds. A flush re-renders every entry, in order, so
/// a handful of slots absorbs a whole screen while keeping the render path off a
/// Win32 or `localtime_r` call per row per frame.
static LAST_LOCAL: std::sync::Mutex<LocalCache> = std::sync::Mutex::new([None; CACHE_SLOTS]);

/// How many seconds the memo keeps; a screen of entries usually lands in fewer.
const CACHE_SLOTS: usize = 8;

type LocalParts = (i32, u32, u32, u32, u32, u32, u32);
type LocalCache = [Option<(u64, Option<LocalParts>)>; CACHE_SLOTS];

/// The local civil time for `timestamp`, memoized per second. A hit reports the
/// timestamp's own milliseconds, so the cache never changes what is shown.
fn local_parts(timestamp: u64) -> Option<LocalParts> {
    let second = timestamp / 1_000;
    let millis = (timestamp % 1_000) as u32;
    let mut cache = LAST_LOCAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, parts)) = cache.iter().flatten().find(|(at, _)| *at == second) {
        return parts.map(|(year, month, day, hour, minute, sec, _)| {
            (year, month, day, hour, minute, sec, millis)
        });
    }
    let parts = pa_types::platform::local_time_parts(timestamp);
    let slot = second as usize % cache.len();
    cache[slot] = Some((second, parts));
    parts
}

#[must_use]
pub(crate) fn now_ms() -> u64 {
    pa_types::platform::now_unix_ms()
}

/// Prefix one complete rendered entry, not every physical row. Its existing
/// rows are untouched so selection and Markdown code indentation keep their
/// meaning.
///
/// The stamp lands on the first row that carries text. Every other painted row
/// takes the same 24 columns as indentation, so the body stays aligned under the
/// stamp: a user message pads its block with rows whose only content is the
/// block background, and leaving those unindented would start their background
/// at column 0 while the text began 24 columns in. A row that is truly empty —
/// the spacer between entries, which paints nothing — is left untouched.
pub(crate) fn decorate(rows: &mut Vec<Line>, timestamp: Option<u64>, style: ratatui::style::Style) {
    let Some((year, month, day, hour, minute, second, millis)) = timestamp.and_then(local_parts)
    else {
        return;
    };
    let stamp = format!(
        "[{year:02}/{month:02}/{day:02} {hour:02}:{minute:02}:{second:02}.{millis:03}] ",
        year = year.rem_euclid(100)
    );
    let mut body = false;
    for row in rows {
        if row.is_empty() {
            continue;
        }
        // The stamp belongs to the first row that carries visible text. Zone
        // markers are zero-width and ride on the block's padding rows too, so
        // they are stripped before the test: a padding row is one whose spans
        // are background paint and markers only.
        let stamp_here = !body && row.iter().any(|span| !is_blank_content(&span.content));
        if stamp_here {
            body = true;
        }
        row.insert(
            0,
            Span::styled(
                if stamp_here {
                    stamp.clone()
                } else {
                    " ".repeat(PREFIX_WIDTH)
                },
                style,
            ),
        );
    }
}

/// Whether a span paints no visible text. Whitespace and the zero-width OSC 133
/// zone markers both read as blank; the markers ride on rows that paint nothing
/// else, so counting them as text would put the stamp on a padding row.
fn is_blank_content(content: &str) -> bool {
    let mut rest = content.trim();
    while let Some(tail) = strip_zone_marker(rest) {
        rest = tail.trim();
    }
    rest.is_empty()
}

/// Drop one leading zone marker sequence, if there is one.
fn strip_zone_marker(text: &str) -> Option<&str> {
    [crate::osc133::ZONE_START, crate::osc133::ZONE_END_PREFIX]
        .into_iter()
        .find_map(|marker| text.strip_prefix(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ColorMode, Theme};

    #[test]
    fn one_message_time_precedes_its_first_visible_row_only() {
        let theme = Theme::builtin("prime", ColorMode::Color256);
        let style = theme.fg_style(crate::theme::ThemeColor::Dim);
        let mut rows = vec![Vec::new()];
        rows.extend(crate::chat::render_text_rows(
            "first\nsecond",
            style,
            80 - PREFIX_WIDTH,
        ));
        let time = 1_794_966_414_224_u64;
        decorate(
            &mut rows,
            Some(time),
            theme.fg_style(crate::theme::ThemeColor::Muted),
        );
        let lines: Vec<String> = rows
            .iter()
            .map(|line| line.iter().map(|span| span.content.as_str()).collect())
            .collect();
        assert!(lines[0].is_empty());
        assert!(
            lines[1].starts_with('['),
            "the stamp leads the first visible row"
        );
        assert!(lines[1][..PREFIX_WIDTH].ends_with("] "));
        assert_eq!(
            crate::width::str_width(&lines[1][..PREFIX_WIDTH]),
            PREFIX_WIDTH
        );
        assert!(lines[1].contains(".224]"));
        assert!(lines[1].contains('/'));
        assert!(!lines[2].contains(".224]"));
        assert!(lines[2].starts_with(&" ".repeat(PREFIX_WIDTH)));
        for line in &lines {
            assert!(crate::width::str_width(line) <= 80);
        }
    }

    /// A user message pads its block with rows whose only content is the block
    /// background. Those rows paint no text, so the stamp does not belong on
    /// them, but they are still rows of the block: they take the 24-column
    /// indent, or the padding ends short of the body and the block's right edge
    /// is ragged.
    #[test]
    fn the_user_blocks_padding_rows_are_indented_but_unstamped() {
        let theme = Theme::builtin("prime", ColorMode::Color256);
        let width = 80;
        let mut rows = crate::chat::render_user_block_with_style(
            "body",
            &theme,
            &crate::markdown::MarkdownStyle::from_theme(&theme),
            width - PREFIX_WIDTH,
        );
        let row_count = rows.len();
        decorate(
            &mut rows,
            Some(1_794_966_414_224),
            theme.fg_style(crate::theme::ThemeColor::Muted),
        );
        assert_eq!(rows.len(), row_count, "decorating keeps the row count");
        // The block's first and last rows carry zero-width OSC 133 markers;
        // strip them so the comparison is over visible text only.
        let mut unscoped = rows.clone();
        for line in &mut unscoped {
            crate::osc133::strip(line);
        }
        let text: Vec<String> = unscoped
            .iter()
            .map(|line| line.iter().map(|span| span.content.as_str()).collect())
            .collect();
        let mut banded = 0;
        for (index, line) in text.iter().enumerate() {
            let indented = line.starts_with(&" ".repeat(PREFIX_WIDTH));
            let stamped = line.contains(".224]");
            assert!(
                indented || stamped,
                "row {index} is either the stamp or the indent under it: {line:?}"
            );
            assert_eq!(
                crate::width::str_width(line),
                width,
                "row {index} fills the terminal width: {line:?}"
            );
            if !stamped {
                banded += 1;
            }
        }
        assert_eq!(banded, row_count - 1, "every other row only indents");
        let stamp_row = text
            .iter()
            .position(|line| line.contains(".224]"))
            .expect("the block is stamped");
        assert!(
            text[stamp_row].contains("body"),
            "the stamp lands on the row with text, not a padding row: {text:?}"
        );
        assert_eq!(
            text.iter().filter(|line| line.contains(".224]")).count(),
            1,
            "one stamp per entry: {text:?}"
        );
    }

    /// The per-second cache must not cost millisecond precision: two stamps in
    /// the same second differ only in their milliseconds.
    #[test]
    fn entries_sharing_a_second_keep_their_own_milliseconds() {
        let theme = Theme::builtin("prime", ColorMode::Color256);
        let style = theme.fg_style(crate::theme::ThemeColor::Muted);
        let render = |timestamp: u64| {
            let mut rows = vec![Vec::new()];
            rows.extend(crate::chat::render_text_rows(
                "x",
                theme.fg_style(crate::theme::ThemeColor::Dim),
                80 - PREFIX_WIDTH,
            ));
            decorate(&mut rows, Some(timestamp), style);
            rows[1]
                .iter()
                .map(|span| span.content.as_str())
                .collect::<String>()
        };
        // Both stamps fall in the same second, so the second call is a cache hit.
        let early = render(1_794_966_414_110);
        let late = render(1_794_966_414_987);
        assert!(early.contains(".110]"), "early: {early:?}");
        assert!(late.contains(".987]"), "late: {late:?}");
        let seconds = |stamp: &str| stamp[..stamp.rfind('.').unwrap()].to_string();
        assert_eq!(
            seconds(&early),
            seconds(&late),
            "the second-level fields agree"
        );
    }
}
