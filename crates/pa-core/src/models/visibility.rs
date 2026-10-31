//! The `visibleModels` limit: the model patterns one audience may see and
//! select. Where `allowedModels` refuses a model at run time (a resolution
//! error), this one removes it from the catalog the audience reads, so a model
//! outside the list is not merely unavailable — it is never offered.

use globset::{Glob, GlobSet, GlobSetBuilder};

/// A model the limit hides: typed so the resolution seams can downcast the
/// refusal out of the plain resolution errors and answer with a message that
/// does not read like a transient failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelVisibilityRefusal {
    /// The refused model, full selector form `provider/model-id`.
    pub selector: String,
    /// The patterns in force, in configuration order.
    pub allowed: Vec<String>,
}

impl std::fmt::Display for ModelVisibilityRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No "unavailable"/"unauthenticated"/"expired" wording here: those read
        // as a transient fault and invite a retry with a different model, which
        // is exactly the substitution this limit exists to prevent.
        let allowed = if self.allowed.is_empty() {
            "no model is enabled".to_string()
        } else {
            self.allowed.join(", ")
        };
        write!(
            f,
            "Model \"{}\" is not enabled (settings \"visibleModels\"; allowed: {allowed}). \
             Do not switch to another model: work with the allowed model or report that it \
             cannot complete the task.",
            self.selector
        )
    }
}

impl std::error::Error for ModelVisibilityRefusal {}

/// The compiled `visibleModels` limit for one audience.
///
/// Patterns follow the `allowedModels` grammar — case-insensitive, matched
/// against the full selector `provider/model-id` and the bare id, with `*`/`?`
/// globbing — but compile once here instead of per model: the catalog holds
/// thousands of entries and the check runs on every listing.
#[derive(Debug, Clone)]
pub struct ModelVisibility {
    patterns: Vec<String>,
    selectors: Option<GlobSet>,
}

impl ModelVisibility {
    /// Compile the patterns. `None` means no limit at all — not an empty
    /// pattern list, which allows nothing.
    #[must_use]
    pub fn new(patterns: Option<&[String]>) -> Option<Self> {
        let patterns = patterns?;
        let patterns: Vec<String> = patterns
            .iter()
            .map(|pattern| pattern.trim().to_string())
            .filter(|pattern| !pattern.is_empty())
            .collect();
        // An empty list is a configured "nothing is visible": keep it as a
        // limit whose pattern set is empty rather than dropping to no limit.
        let mut builder = GlobSetBuilder::new();
        let mut any_pattern = false;
        for pattern in &patterns {
            // Globs and literals share one matcher: a literal is escaped first so
            // its characters are data, and the bare-id arm of `permits` covers
            // the provider-less spelling.
            let compiled = if pattern.contains(['*', '?', '[']) {
                pattern.to_lowercase()
            } else {
                glob_escape(&pattern.to_lowercase())
            };
            if let Ok(glob) = Glob::new(&compiled) {
                builder.add(glob);
                any_pattern = true;
            }
        }
        let selectors = any_pattern.then(|| builder.build().ok()).flatten();
        Some(Self {
            patterns,
            selectors,
        })
    }

    /// The configured patterns, in configuration order.
    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Whether `selector` may be seen. A bare-id match is checked separately
    /// from the full selector, matching the `allowedModels` grammar.
    #[must_use]
    pub fn permits(&self, selector: &str) -> bool {
        let Some(selectors) = &self.selectors else {
            return false;
        };
        let selector = selector.to_lowercase();
        if selectors.is_match(&selector) {
            return true;
        }
        let bare_id = selector
            .split_once('/')
            .map_or(selector.as_str(), |(_, id)| id);
        selectors.is_match(bare_id)
    }

    /// The typed refusal for a hidden selector.
    #[must_use]
    pub fn refusal(&self, selector: &str) -> ModelVisibilityRefusal {
        ModelVisibilityRefusal {
            selector: selector.to_string(),
            allowed: self.patterns.clone(),
        }
    }
}

/// Escape a literal pattern so `Glob::new` treats every character as data.
fn glob_escape(pattern: &str) -> String {
    let mut escaped = String::with_capacity(pattern.len());
    for character in pattern.chars() {
        if matches!(character, '*' | '?' | '[' | ']' | '{' | '}' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visibility(patterns: &[&str]) -> ModelVisibility {
        let owned: Vec<String> = patterns.iter().map(|p| (*p).to_string()).collect();
        ModelVisibility::new(Some(&owned)).expect("a configured limit")
    }

    #[test]
    fn an_unconfigured_limit_is_none() {
        assert!(ModelVisibility::new(None).is_none());
    }

    #[test]
    fn an_empty_list_hides_everything() {
        let limit = visibility(&[]);
        assert!(!limit.permits("yes/deepseek-v4.1-flash"));
        assert!(limit.patterns().is_empty());
    }

    #[test]
    fn glob_and_literal_patterns_match_the_selector_or_the_bare_id() {
        let limit = visibility(&["yes/*", "prime-inference/internal/glm-5.3-fast"]);
        assert!(limit.permits("yes/deepseek-v4.1-flash"));
        assert!(limit.permits("YES/DeepSeek-V4.1-Flash"));
        assert!(!limit.permits("deepseek/deepseek-v4-pro"));
        // Literal full selectors match their own spelling; the bare id of that
        // same model is a different string and only matches a bare-id pattern.
        assert!(limit.permits("prime-inference/internal/glm-5.3-fast"));
        assert!(!limit.permits("internal/glm-5.3-fast"));
        assert!(!limit.permits("prime-inference/internal/glm-5.3"));
        // A bare-id pattern matches the id under any provider.
        let bare = visibility(&["internal/glm-5.3-fast"]);
        assert!(bare.permits("prime-inference/internal/glm-5.3-fast"));
        // A literal star in the pattern is a wildcard, not data.
        let escaped = visibility(&["provider/a*b"]);
        assert!(escaped.permits("provider/axxb"));
    }

    #[test]
    fn the_refusal_does_not_read_like_a_transient_failure() {
        let limit = visibility(&["yes/deepseek-v4.1-flash"]);
        let message = limit.refusal("opencode-go/gpt-5.5").to_string();
        assert!(message.contains("is not enabled"), "{message}");
        assert!(
            message.contains("allowed: yes/deepseek-v4.1-flash"),
            "{message}"
        );
        assert!(
            message.contains("Do not switch to another model"),
            "{message}"
        );
        for misleading in ["unavailable", "unauthenticated", "expired"] {
            assert!(!message.contains(misleading), "{misleading} in {message}");
        }
    }

    #[test]
    fn an_empty_limit_refusal_says_nothing_is_enabled() {
        let message = visibility(&[]).refusal("yes/glm-5.3").to_string();
        assert!(message.contains("no model is enabled"), "{message}");
    }
}
