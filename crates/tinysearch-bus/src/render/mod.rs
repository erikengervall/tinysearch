//! Canonical text and markdown rendering of an [`ExecuteToolResponse`].
//!
//! Pure and I/O-free. The text form is what a model reads: a heading with a
//! `(via <Provider>)` marker that UIs may parse, then the answer, numbered
//! results and a `Sources:` list. Hosts layer their own structured metadata on
//! top; the wording here is part of the contract.
//!
//! A result whose page carries an image listed under
//! [`IMAGE_ASSETS`] is followed by that image's direct
//! URL (`Image: <url>` in text, an inline image in markdown), so image search
//! output keeps both the image and the page it came from.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde_json::Value;

use crate::labels::provider_label;
use crate::{Citation, ExecuteToolResponse, IMAGE_ASSETS, Role, SearchStatus};

mod options;
pub use options::{COMPACT_CONTENTS_CHARS, RenderOptions, TEXT_EXCERPT_CHARS};

/// What the call was about: the query, the URLs of a contents call, or the
/// objective, taken from the tool arguments.
#[must_use]
pub fn subject(arguments: &Value) -> String {
    if let Some(query) = arguments.get("query").and_then(Value::as_str) {
        return query.trim().to_string();
    }
    if let Some(urls) = arguments.get("urls").and_then(Value::as_array) {
        return urls
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
    }
    arguments
        .get("objective")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The one-line heading, e.g. `Search results for: q (via Exa)`.
#[must_use]
pub fn heading(response: &ExecuteToolResponse, subject: &str) -> String {
    let provider = provider_label(&response.provider);
    let via = if response.fallback_from.is_empty() {
        format!("via {provider}")
    } else {
        let skipped: Vec<_> = response
            .fallback_from
            .iter()
            .map(|p| provider_label(p))
            .collect();
        format!("via {provider}, after {}", skipped.join(", "))
    };
    match (response.role, response.status) {
        (_, SearchStatus::InProgress) => format!("Research still running for: {subject} ({via})"),
        (Some(Role::Answer), _) => format!("Answer for: {subject} ({via})"),
        (Some(Role::Contents), _) => format!("Page contents for: {subject} ({via})"),
        (Some(Role::Images), _) if response.results.is_empty() => {
            format!("No images found for: {subject} ({via})")
        }
        (Some(Role::Images), _) => format!("Image results for: {subject} ({via})"),
        _ if response.results.is_empty() && response.answer.is_none() => {
            format!("No results found for: {subject} ({via})")
        }
        _ => format!("Search results for: {subject} ({via})"),
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Truncate to `max_chars` characters, trimming trailing whitespace and
/// appending `suffix` only when something was cut.
fn truncate(s: &str, max_chars: usize, suffix: &str) -> String {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => format!("{}{suffix}", s[..idx].trim_end()),
        None => s.to_string(),
    }
}

/// The direct image URL for each result, paired from
/// [`IMAGE_ASSETS`] by source page. Each asset pairs at
/// most once, in order, so several images from one page stay distinct.
fn result_images(response: &ExecuteToolResponse) -> Vec<Option<&str>> {
    let assets: Vec<(&str, &str)> = response
        .provider_data
        .as_ref()
        .and_then(|data| data.get(IMAGE_ASSETS))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|asset| {
            let source = asset.get("source_url")?.as_str()?.trim();
            let image = non_empty(asset.get("image_url")?.as_str())?;
            Some((source, image))
        })
        .collect();
    let mut used = vec![false; assets.len()];
    response
        .results
        .iter()
        .map(|result| {
            let index = assets
                .iter()
                .enumerate()
                .position(|(index, (source, _))| !used[index] && *source == result.url.trim())?;
            used[index] = true;
            Some(assets[index].1)
        })
        .collect()
}

/// A URL as compared for repeats: the scheme and host of an HTTP(S) URL are
/// case-insensitive, the rest (including any userinfo) is not.
fn url_key(url: &str) -> String {
    let url = url.trim();
    let Some(split) = url.find("://") else {
        return url.to_string();
    };
    let (scheme, rest) = url.split_at(split);
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return url.to_string();
    }
    let rest = &rest[3..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let (userinfo, host) = match authority.rfind('@') {
        Some(at) => authority.split_at(at + 1),
        None => ("", authority),
    };
    format!(
        "{}://{userinfo}{}{tail}",
        scheme.to_ascii_lowercase(),
        host.to_ascii_lowercase()
    )
}

/// The answer text to print. A page-contents response may carry the first
/// page as its answer as well as in its result snippet; under non-default
/// options that copy would bypass the per-page budget, so it is left out when
/// the page is listed. Answer-role responses are never touched, and the
/// default options keep the original output.
fn answer_text<'a>(response: &'a ExecuteToolResponse, options: &RenderOptions) -> Option<&'a str> {
    let answer = non_empty(response.answer.as_deref())?;
    let duplicates_page = response.role == Some(Role::Contents)
        && !response.results.is_empty()
        && *options != RenderOptions::default();
    (!duplicates_page).then_some(answer)
}

/// The citations to list under `Sources`, with their 1-based numbers.
///
/// All of them unless `options.compact_sources` is set; then only those that
/// are not among the first `max_results` results, at most `max_results`.
fn sources<'a>(
    response: &'a ExecuteToolResponse,
    max_results: usize,
    options: &RenderOptions,
) -> Vec<(usize, &'a Citation)> {
    let numbered = response
        .citations
        .iter()
        .enumerate()
        .map(|(i, c)| (i + 1, c));
    if !options.compact_sources {
        return numbered.collect();
    }
    let shown: HashSet<String> = response
        .results
        .iter()
        .take(max_results)
        .map(|result| url_key(&result.url))
        .collect();
    numbered
        .filter(|(_, citation)| !shown.contains(&url_key(&citation.url)))
        .take(max_results)
        .collect()
}

/// Plain-text rendering, showing at most `max_results` results.
#[must_use]
pub fn render_text(response: &ExecuteToolResponse, subject: &str, max_results: usize) -> String {
    render_text_with(response, subject, max_results, &RenderOptions::default())
}

/// [`render_text`] with explicit [`RenderOptions`].
#[must_use]
pub fn render_text_with(
    response: &ExecuteToolResponse,
    subject: &str,
    max_results: usize,
    options: &RenderOptions,
) -> String {
    let excerpt_chars = options.excerpt_for(response.role);
    let mut lines = vec![heading(response, subject)];
    if let Some(answer) = answer_text(response, options) {
        lines.push(String::new());
        lines.push(answer.to_string());
    }
    let images = result_images(response);
    for (index, item) in response.results.iter().take(max_results).enumerate() {
        let title = if item.title.trim().is_empty() {
            "No title"
        } else {
            item.title.trim()
        };
        lines.push(format!("{}. {}", index + 1, title));
        lines.push(format!("   {}", item.url.trim()));
        if let Some(image) = images[index] {
            lines.push(format!("   Image: {image}"));
        }
        if let Some(date) = non_empty(item.published.as_deref()) {
            lines.push(format!("   Published: {date}"));
        }
        if let Some(snippet) = non_empty(item.snippet.as_deref()) {
            lines.push(format!("   {}", truncate(snippet, excerpt_chars, "…")));
        }
    }
    let sources = sources(response, max_results, options);
    if !sources.is_empty() {
        lines.push(String::new());
        lines.push("Sources:".into());
        for (number, citation) in sources {
            match non_empty(citation.title.as_deref()) {
                Some(title) => lines.push(format!("[{number}] {title} — {}", citation.url)),
                None => lines.push(format!("[{number}] {}", citation.url)),
            }
        }
    }
    lines.join("\n")
}

/// Markdown rendering, showing at most `max_results` results.
#[must_use]
pub fn render_markdown(
    response: &ExecuteToolResponse,
    subject: &str,
    max_results: usize,
) -> String {
    render_markdown_with(response, subject, max_results, &RenderOptions::default())
}

/// [`render_markdown`] with explicit [`RenderOptions`].
#[must_use]
pub fn render_markdown_with(
    response: &ExecuteToolResponse,
    subject: &str,
    max_results: usize,
    options: &RenderOptions,
) -> String {
    let excerpt_chars = options.excerpt_for(response.role);
    let mut out = format!("# {}\n", heading(response, subject));
    if let Some(answer) = answer_text(response, options) {
        let _ = write!(out, "\n{answer}\n");
    }
    let images = result_images(response);
    for (index, item) in response.results.iter().take(max_results).enumerate() {
        let title = if item.title.trim().is_empty() {
            "Untitled"
        } else {
            item.title.trim()
        };
        let _ = write!(out, "\n## [{title}]({})\n", item.url.trim());
        if let Some(date) = non_empty(item.published.as_deref()) {
            let _ = write!(out, "_Published: {date}_\n\n");
        }
        if let Some(snippet) = non_empty(item.snippet.as_deref()) {
            let _ = writeln!(out, "> {}", truncate(snippet, excerpt_chars, "…"));
        }
        if let Some(image) = images[index] {
            let _ = write!(out, "\n![{title}]({image})\n");
        }
    }
    let sources = sources(response, max_results, options);
    if !sources.is_empty() {
        out.push_str("\n### Sources\n");
        for (_, citation) in sources {
            let title = non_empty(citation.title.as_deref()).unwrap_or(citation.url.as_str());
            let _ = writeln!(out, "- [{title}]({})", citation.url);
        }
    }
    out
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
