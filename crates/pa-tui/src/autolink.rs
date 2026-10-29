//! marked's GFM inline autolink rules (marked 18.0.7, the TS markdown
//! tokenizer), ported verbatim: after every other inline construct fails, marked tries
//! `autolink` (`<scheme:...>` / `<email>`) then `url` (bare `http(s)://`, `ftp://`, `www.`,
//! and email shapes, the latter only outside a link label).

use std::sync::LazyLock;

use fancy_regex::Regex;

/// `rules.inline.autolink`: angle-form link. Group 1 is the link target,
/// group 2 the `@` marking the email alternative.
static ANGLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s\x00-\x1f<>]*|[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+(@)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+(?![-_]))>",
    )
    .expect("static autolink regex")
});

/// `rules.inline.url` (gfm): bare url/www/email. Group 1 is the scheme
/// (`www.` included), group 2 the `@` marking the email alternative.
static BARE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^((?:[hH][tT][tT][pP][sS]?|[fF][tT][pP])://|www\.)(?:[a-zA-Z0-9\-]+\.?)+[^\s<]*|^[A-Za-z0-9._+-]+(@)[a-zA-Z0-9-_]+(?:\.[a-zA-Z0-9-_]*[a-zA-Z0-9])+(?![-_])",
    )
    .expect("static url regex")
});

/// `rules.inline._backpedal`: trailing-punctuation trim. Balanced paren groups survive;
/// a punctuation run may only be consumed when it does not end the string (the `(?!$)`
/// guard), so the tokenizer's fixpoint loop peels a trailing run one character per pass.
static BACKPEDAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:[^?!.,:;*_'"~()&]+|\([^)]*\)|&(?![a-zA-Z0-9]+;$)|[?!.,:;*_'"~)]+(?!$))+"#)
        .expect("static backpedal regex")
});

/// One autolink match: the visible label, the href marked puts on the token (before
/// `new URL()` normalization), and the raw text the lexer consumes for it (for bare urls
/// the backpedaled match, so a trimmed trailing punctuation run stays in the text stream).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutolinkToken {
    pub raw: String,
    pub text: String,
    pub href: String,
}

/// The `autolink` rule: `<scheme:...>` or `<email>`. Never backpedaled.
pub(crate) fn angle_token(src: &str) -> Option<AutolinkToken> {
    let caps = ANGLE.captures(src).ok().flatten()?;
    let raw = caps.get(0)?.as_str().to_string();
    let text = caps.get(1)?.as_str().to_string();
    let href = if caps.get(2).is_some() {
        format!("mailto:{text}")
    } else {
        text.clone()
    };
    Some(AutolinkToken { raw, text, href })
}

/// The gfm `url` rule: bare protocol links, `www.` links (href gains a `http://` scheme),
/// and bare emails (href gains `mailto:`). The url/www branch runs the backpedal fixpoint;
/// the email branch needs none (its domain grammar cannot end on punctuation).
pub(crate) fn bare_token(src: &str) -> Option<AutolinkToken> {
    let caps = BARE.captures(src).ok().flatten()?;
    if caps.get(2).is_some() {
        let raw = caps.get(0)?.as_str().to_string();
        let href = format!("mailto:{raw}");
        return Some(AutolinkToken {
            text: raw.clone(),
            href,
            raw,
        });
    }
    let scheme = caps.get(1)?.as_str().to_string();
    let matched = caps.get(0)?.as_str();
    let (start, end) = backpedal_range(matched);
    let text = &matched[start..end];
    let href = if scheme == "www." {
        format!("http://{text}")
    } else {
        text.to_string()
    };
    Some(AutolinkToken {
        raw: text.to_string(),
        text: text.to_string(),
        href,
    })
}

/// marked's `_backpedal` fixpoint as a byte range over one matched url: strip
/// the trailing punctuation run, keeping balanced parens. The loop walks
/// (start, end) ranges over the one matched slice instead of re-copying the
/// string each pass, so a long trailing-punctuation run peels without an
/// allocation per peeled character.
fn backpedal_range(matched: &str) -> (usize, usize) {
    let mut start = 0;
    let mut end = matched.len();
    while let Ok(Some(m)) = BACKPEDAL.find(&matched[start..end]) {
        if m.start() == 0 && m.end() == end - start {
            break;
        }
        start += m.start();
        end = start + m.end() - m.start();
    }
    (start, end)
}

/// Cheap per-position gate for the gfm `url` rule: the rule can only match when its
/// literal prefix is present (a scheme case-insensitive, the `www.` literal) or a
/// local-part run leads directly to an `@`. Both tests are bounded char scans.
pub(crate) fn bare_candidate(bytes: &[char], i: usize, line_has_at: bool) -> bool {
    for (prefix, ci) in [
        ("https://", true),
        ("http://", true),
        ("ftp://", true),
        ("www.", false),
    ] {
        if starts_with(bytes, i, prefix, ci) {
            return true;
        }
    }
    if line_has_at && (i == 0 || !is_local_part(bytes[i - 1])) {
        // Only the first character of a local-part run can start an email match, and one
        // scan per run keeps a long local-part run linear instead of quadratic.
        let mut j = i;
        while j < bytes.len() && is_local_part(bytes[j]) {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == '@' {
            return true;
        }
    }
    false
}

fn is_local_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-')
}

/// A bare URL run at `at`, for a surface that renders text literally (the user
/// message echo): the same gfm `url` grammar the markdown inline renderer uses,
/// minus the email alternative, so a typed URL becomes clickable without the
/// renderer having to interpret markdown around it.
///
/// Returns the run's length in chars. The run never includes trailing
/// punctuation (the shared backpedal rule), so `see https://x.dev/a.` links
/// `https://x.dev/a` and leaves the period in the text.
pub(crate) fn bare_url_run(bytes: &[char], at: usize) -> Option<UrlRun> {
    for prefix in ["https://", "http://", "www."] {
        let case_insensitive = prefix != "www.";
        if !starts_with(bytes, at, prefix, case_insensitive) {
            continue;
        }
        let rest: String = bytes[at..].iter().collect();
        let caps = BARE.captures(&rest).ok().flatten()?;
        // The email alternative cannot fire for these prefixes (no `@` group),
        // but a defensive skip keeps the two run shapes from mixing.
        if caps.get(2).is_some() {
            return None;
        }
        let matched = caps.get(0)?.as_str();
        let (start, end) = backpedal_range(matched);
        let trimmed = &matched[start..end];
        if trimmed.is_empty() {
            return None;
        }
        return Some(UrlRun {
            len: trimmed.chars().count(),
        });
    }
    None
}

/// One bare URL's length in chars.
pub(crate) struct UrlRun {
    pub len: usize,
}

fn starts_with(bytes: &[char], i: usize, prefix: &str, ignore_case: bool) -> bool {
    prefix.chars().enumerate().all(|(k, pc)| {
        bytes.get(i + k).is_some_and(|&c| {
            if ignore_case {
                c.eq_ignore_ascii_case(&pc)
            } else {
                c == pc
            }
        })
    })
}
