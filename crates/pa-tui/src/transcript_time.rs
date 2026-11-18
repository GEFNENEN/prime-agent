//! Display-only transcript timestamps. The stored messages, tool results,
//! and daemon wire frames remain unchanged.
use crate::{Line, Span};

pub(crate) const PREFIX_WIDTH: usize = 24;

#[must_use]
pub(crate) fn now_ms() -> u64 {
    pa_types::platform::now_unix_ms()
}

/// Prefix one complete rendered entry, not every physical row. Its existing
/// rows are untouched so selection and Markdown code indentation keep their
/// meaning; continuation rows align below the body, and leading spacers stay
/// blank.
pub(crate) fn decorate(rows: &mut Vec<Line>, timestamp: Option<u64>, style: ratatui::style::Style) {
    let Some((year, month, day, hour, minute, second, millis)) =
        timestamp.and_then(pa_types::platform::local_time_parts)
    else {
        return;
    };
    let stamp = format!(
        "[{year:02}/{month:02}/{day:02} {hour:02}:{minute:02}:{second:02}.{millis:03}] ",
        year = year.rem_euclid(100)
    );
    let mut body = false;
    for row in rows {
        if row.iter().all(|span| span.content.trim().is_empty()) {
            continue;
        }
        row.insert(
            0,
            Span::styled(
                if body {
                    " ".repeat(PREFIX_WIDTH)
                } else {
                    body = true;
                    stamp.clone()
                },
                style,
            ),
        );
    }
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
}
