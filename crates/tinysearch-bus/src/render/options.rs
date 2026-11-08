//! Knobs for [`render_text_with`](super::render_text_with) and
//! [`render_markdown_with`](super::render_markdown_with).

use serde::{Deserialize, Serialize};

/// Snippets in rendered output are capped at this many characters.
pub const TEXT_EXCERPT_CHARS: usize = 500;

/// Per-page character cap [`RenderOptions::compact`] uses for page contents.
pub const COMPACT_CONTENTS_CHARS: usize = 1_500;

/// How much a rendering shows. [`Default`] is the original output: every
/// citation listed under `Sources`, snippets cut at [`TEXT_EXCERPT_CHARS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct RenderOptions {
    /// Cap, in characters, on each result's snippet.
    pub excerpt_chars: usize,
    /// Cap, in characters, on each page's text in a page-contents response.
    pub contents_chars: usize,
    /// Leave out citations that repeat a listed result, and show at most
    /// `max_results` of the rest.
    pub compact_sources: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            excerpt_chars: TEXT_EXCERPT_CHARS,
            contents_chars: TEXT_EXCERPT_CHARS,
            compact_sources: false,
        }
    }
}

impl RenderOptions {
    /// The compact rendering: no repeated sources and
    /// [`COMPACT_CONTENTS_CHARS`] per page.
    #[must_use]
    pub const fn compact() -> Self {
        Self {
            excerpt_chars: TEXT_EXCERPT_CHARS,
            contents_chars: COMPACT_CONTENTS_CHARS,
            compact_sources: true,
        }
    }

    /// The snippet cap for a response of `role`.
    pub(super) const fn excerpt_for(&self, role: Option<crate::Role>) -> usize {
        match role {
            Some(crate::Role::Contents) => self.contents_chars,
            _ => self.excerpt_chars,
        }
    }
}
