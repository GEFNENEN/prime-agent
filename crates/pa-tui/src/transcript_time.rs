//! Display-only transcript timestamps. The stored messages, tool results,
//! and daemon wire frames remain unchanged.
use crate::{Line, Span};

/// The narrowest body a stamped entry keeps; below it the prefix yields so a
/// narrow window still shows its text.
pub(crate) const MIN_BODY_WIDTH: usize = 8;

/// The date part of the stamp. The menu exposes these four values in order;
/// `YY/MM/DD` is the shipped default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TimestampDate {
    /// Time only.
    None,
    /// `MM/DD`, no year.
    MonthDay,
    /// `YY/MM/DD` (two-digit year).
    #[default]
    ShortYear,
    /// `YYYY/MM/DD` (four-digit year).
    FullYear,
}

/// The values the `/settings` date row cycles, in display order.
pub(crate) const TIMESTAMP_DATE_VALUES: [&str; 4] = ["none", "MM/DD", "YY/MM/DD", "YYYY/MM/DD"];

impl TimestampDate {
    /// Parse a persisted value; anything unrecognized reads as the shipped
    /// `YY/MM/DD` so a hand-edited settings file cannot blank the stamp.
    pub(crate) fn from_setting(value: &str) -> Self {
        match value {
            "none" => Self::None,
            "MM/DD" => Self::MonthDay,
            "YYYY/MM/DD" => Self::FullYear,
            _ => Self::ShortYear,
        }
    }
}

/// How a transcript stamp renders: the master switch plus the date and
/// fractional-second choices. `Copy + Eq` so it can join the view's layout
/// cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimestampFormat {
    pub(crate) enabled: bool,
    pub(crate) date: TimestampDate,
    /// Fractional-second digits: 0 hides the fraction, 1-3 truncate.
    pub(crate) millis: u8,
}

impl Default for TimestampFormat {
    fn default() -> Self {
        Self {
            enabled: false,
            date: TimestampDate::ShortYear,
            millis: 3,
        }
    }
}

impl TimestampFormat {
    /// The format from the settings seam; an out-of-range precision clamps to
    /// the three digits the clock carries.
    pub(crate) fn new(enabled: bool, date: &str, millis: u8) -> Self {
        Self {
            enabled,
            date: TimestampDate::from_setting(date),
            millis: millis.min(3),
        }
    }

    /// The columns the stamp occupies, brackets and trailing space included.
    pub(crate) fn prefix_width(self) -> usize {
        let date = match self.date {
            TimestampDate::None => 0,
            TimestampDate::MonthDay => 6,
            TimestampDate::ShortYear => 9,
            TimestampDate::FullYear => 11,
        };
        let fraction = match self.millis {
            0 => 0,
            digits => 1 + usize::from(digits),
        };
        3 + date + 8 + fraction
    }

    /// The complete prefix, e.g. `[26/11/18 10:26:54.224] `.
    fn stamp(self, parts: LocalParts) -> String {
        let (year, month, day, hour, minute, second, millis) = parts;
        let date = match self.date {
            TimestampDate::None => String::new(),
            TimestampDate::MonthDay => format!("{month:02}/{day:02} "),
            TimestampDate::ShortYear => {
                format!("{:02}/{month:02}/{day:02} ", year.rem_euclid(100))
            }
            TimestampDate::FullYear => format!("{year:04}/{month:02}/{day:02} "),
        };
        let fraction = if self.millis == 0 {
            String::new()
        } else {
            // Truncate, never round: the stamp must not run ahead of the time
            // the entry actually carries.
            let digits = format!("{millis:03}");
            format!(".{}", &digits[..usize::from(self.millis)])
        };
        format!("[{date}{hour:02}:{minute:02}:{second:02}{fraction}] ")
    }
}

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
/// takes the format's prefix columns as indentation, so the body stays aligned
/// under the stamp: a user message pads its block with rows whose only content
/// is the block background, and leaving those unindented would start their
/// background at column 0 while the text began under the stamp. A row that is
/// truly empty — the spacer between entries, which paints nothing — is left
/// untouched.
pub(crate) fn decorate(
    rows: &mut Vec<Line>,
    timestamp: Option<u64>,
    style: ratatui::style::Style,
    format: TimestampFormat,
) {
    let Some(parts) = timestamp.and_then(local_parts) else {
        return;
    };
    let stamp = format.stamp(parts);
    let prefix = format.prefix_width();
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
                    " ".repeat(prefix)
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
        let format = TimestampFormat::new(true, "YY/MM/DD", 3);
        let prefix = format.prefix_width();
        let mut rows = vec![Vec::new()];
        rows.extend(crate::chat::render_text_rows(
            "first\nsecond",
            style,
            80 - prefix,
        ));
        let time = 1_794_966_414_224_u64;
        decorate(
            &mut rows,
            Some(time),
            theme.fg_style(crate::theme::ThemeColor::Muted),
            format,
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
        assert!(lines[1][..prefix].ends_with("] "));
        assert_eq!(crate::width::str_width(&lines[1][..prefix]), prefix);
        assert!(lines[1].contains(".224]"));
        assert!(lines[1].contains('/'));
        assert!(!lines[2].contains(".224]"));
        assert!(lines[2].starts_with(&" ".repeat(prefix)));
        for line in &lines {
            assert!(crate::width::str_width(line) <= 80);
        }
    }

    /// A user message pads its block with rows whose only content is the block
    /// background. Those rows paint no text, so the stamp does not belong on
    /// them, but they are still rows of the block: they take the format's
    /// prefix columns as the indent, or the padding ends short of the body and
    /// the block's right edge is ragged. Every format must hold this.
    #[test]
    fn every_format_indents_the_user_blocks_padding_rows() {
        let theme = Theme::builtin("prime", ColorMode::Color256);
        let width = 80;
        let time = 1_794_966_414_224_u64;
        let parts = local_parts(time).expect("local time");
        for format in [
            TimestampFormat::new(true, "YY/MM/DD", 3),
            TimestampFormat::new(true, "YYYY/MM/DD", 3),
            TimestampFormat::new(true, "none", 0),
        ] {
            let prefix = format.prefix_width();
            let stamp = format.stamp(parts);
            let mut rows = crate::chat::render_user_block_with_style(
                "body",
                &theme,
                &crate::markdown::MarkdownStyle::from_theme(&theme),
                width - prefix,
            );
            let row_count = rows.len();
            decorate(
                &mut rows,
                Some(time),
                theme.fg_style(crate::theme::ThemeColor::Muted),
                format,
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
                let indented = line.starts_with(&" ".repeat(prefix));
                let stamped = line.contains(&stamp);
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
                .position(|line| line.contains(&stamp))
                .expect("the block is stamped");
            assert!(
                text[stamp_row].contains("body"),
                "the stamp lands on the row with text, not a padding row: {text:?}"
            );
            assert_eq!(
                text.iter().filter(|line| line.contains(&stamp)).count(),
                1,
                "one stamp per entry: {text:?}"
            );
        }
    }

    /// The per-second cache must not cost millisecond precision: two stamps in
    /// the same second differ only in their milliseconds.
    #[test]
    fn entries_sharing_a_second_keep_their_own_milliseconds() {
        let theme = Theme::builtin("prime", ColorMode::Color256);
        let style = theme.fg_style(crate::theme::ThemeColor::Muted);
        let format = TimestampFormat::new(true, "YY/MM/DD", 3);
        let prefix = format.prefix_width();
        let render = |timestamp: u64| {
            let mut rows = vec![Vec::new()];
            rows.extend(crate::chat::render_text_rows(
                "x",
                theme.fg_style(crate::theme::ThemeColor::Dim),
                80 - prefix,
            ));
            decorate(&mut rows, Some(timestamp), style, format);
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

    /// Every menu value maps to the documented columns, and the rendered stamp
    /// is exactly that wide (the width feeds the body layout).
    #[test]
    fn the_prefix_width_and_stamp_track_the_format() {
        let parts = (2026, 11, 18, 10, 26, 54, 224);
        let cases = [
            ("none", 0, "[10:26:54] ", 11),
            ("none", 1, "[10:26:54.2] ", 13),
            ("none", 2, "[10:26:54.22] ", 14),
            ("none", 3, "[10:26:54.224] ", 15),
            ("MM/DD", 0, "[11/18 10:26:54] ", 17),
            ("MM/DD", 3, "[11/18 10:26:54.224] ", 21),
            ("YY/MM/DD", 0, "[26/11/18 10:26:54] ", 20),
            ("YY/MM/DD", 3, "[26/11/18 10:26:54.224] ", 24),
            ("YYYY/MM/DD", 0, "[2026/11/18 10:26:54] ", 22),
            ("YYYY/MM/DD", 3, "[2026/11/18 10:26:54.224] ", 26),
        ];
        for (date, millis, stamp, width) in cases {
            let format = TimestampFormat::new(true, date, millis);
            assert_eq!(format.prefix_width(), width, "{date} {millis}");
            assert_eq!(format.stamp(parts), stamp, "{date} {millis}");
            assert_eq!(crate::width::str_width(stamp), width, "{stamp}");
        }
    }

    /// Milliseconds truncate rather than round, and an unknown or out-of-range
    /// stored value falls back to the shipped format.
    #[test]
    fn precision_truncates_and_bad_settings_fall_back() {
        let parts = (2026, 11, 18, 10, 26, 54, 987);
        assert_eq!(
            TimestampFormat::new(true, "none", 1).stamp(parts),
            "[10:26:54.9] "
        );
        assert_eq!(
            TimestampFormat::new(true, "none", 2).stamp(parts),
            "[10:26:54.98] "
        );
        assert_eq!(
            TimestampFormat::new(true, "bogus", 9),
            TimestampFormat {
                enabled: true,
                date: TimestampDate::ShortYear,
                millis: 3,
            }
        );
    }
}
