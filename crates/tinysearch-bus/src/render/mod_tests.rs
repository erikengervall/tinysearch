use super::*;
use crate::{Citation, SearchResult};

fn response(role: Option<Role>) -> ExecuteToolResponse {
    ExecuteToolResponse {
        provider: "exa".into(),
        results: vec![SearchResult {
            title: "Tokio select".into(),
            url: "https://tokio.rs/select".into(),
            snippet: Some("Waits on multiple futures".into()),
            published: Some("2026-09-01".into()),
        }],
        citations: Vec::new(),
        answer: None,
        status: SearchStatus::Ok,
        provider_data: None,
        role,
        fallback_from: Vec::new(),
    }
}

fn answer_fixture() -> ExecuteToolResponse {
    let mut answer = response(Some(Role::Answer));
    answer.provider = "gemini".into();
    answer.results.clear();
    answer.answer = Some("It returns the first future to complete.".into());
    answer.citations = vec![
        Citation {
            url: "https://docs.rs/tokio".into(),
            title: Some("tokio docs".into()),
        },
        Citation {
            url: "https://tokio.rs".into(),
            title: None,
        },
    ];
    answer.fallback_from = vec!["exa".into()];
    answer
}

#[test]
fn search_results_carry_the_via_marker() {
    let text = render_text(&response(Some(Role::Search)), "tokio select", 5);
    assert!(text.starts_with("Search results for: tokio select (via Exa)"));
    assert!(text.contains("1. Tokio select"));
}

#[test]
fn answers_render_text_citations_and_fallbacks() {
    let text = render_text(&answer_fixture(), "what does select do", 5);
    assert!(text.starts_with("Answer for: what does select do (via Gemini, after Exa)"));
    assert!(text.contains("It returns the first future"));
    assert!(text.contains("[1] tokio docs — https://docs.rs/tokio"));
    assert!(render_markdown(&answer_fixture(), "q", 5).contains("### Sources"));
}

#[test]
fn empty_results_still_attribute_the_provider() {
    let mut empty = response(None);
    empty.results.clear();
    empty.status = SearchStatus::Empty;
    assert_eq!(
        render_text(&empty, "nothing", 5),
        "No results found for: nothing (via Exa)"
    );
}

#[test]
fn in_progress_research_is_flagged() {
    let mut running = response(Some(Role::Answer));
    running.provider = "gemini_deep_research".into();
    running.results.clear();
    running.status = SearchStatus::InProgress;
    assert!(
        render_text(&running, "deep topic", 5)
            .starts_with("Research still running for: deep topic (via Gemini Deep Research)")
    );
}

#[test]
fn results_are_capped() {
    let mut many = response(None);
    many.results = (0..8)
        .map(|i| SearchResult {
            title: format!("r{i}"),
            url: format!("https://e.example/{i}"),
            snippet: None,
            published: None,
        })
        .collect();
    let text = render_text(&many, "q", 3);
    assert!(text.contains("3. r2"));
    assert!(!text.contains("4. r3"));
}

#[test]
fn long_snippets_are_truncated() {
    let mut long = response(None);
    long.results[0].snippet = Some("x".repeat(600));
    let text = render_text(&long, "q", 5);
    assert!(text.contains(&format!("{}…", "x".repeat(500))));
    assert!(!text.contains(&"x".repeat(501)));
}

#[test]
fn subject_reads_query_urls_or_objective() {
    assert_eq!(subject(&serde_json::json!({"query": " q "})), "q");
    assert_eq!(
        subject(&serde_json::json!({"urls": ["https://a", "https://b"]})),
        "https://a, https://b"
    );
    assert_eq!(subject(&serde_json::json!({"objective": " o "})), "o");
    assert_eq!(subject(&serde_json::json!({})), "");
}

fn image_fixture() -> ExecuteToolResponse {
    let page = |title: &str, url: &str| SearchResult {
        title: title.into(),
        url: url.into(),
        snippet: None,
        published: None,
    };
    let mut images = response(Some(Role::Images));
    images.provider = "brave".into();
    images.results = vec![
        page("Red panda", "https://zoo.example/panda"),
        page("Red panda, again", "https://zoo.example/panda"),
        page("No image", "https://zoo.example/text"),
    ];
    images.provider_data = Some(serde_json::json!({IMAGE_ASSETS:[
        {"source_url":"https://zoo.example/panda","image_url":"https://img.example/1.jpg"},
        {"source_url":"https://zoo.example/panda","image_url":"https://img.example/2.jpg"},
        {"source_url":"https://zoo.example/other","image_url":" "}
    ]}));
    images
}

#[test]
fn image_results_list_each_image_under_its_page() {
    let text = render_text(&image_fixture(), "red panda", 5);
    assert!(text.starts_with("Image results for: red panda (via Brave)"));
    assert!(text.contains(
        "1. Red panda\n   https://zoo.example/panda\n   Image: https://img.example/1.jpg"
    ));
    assert!(text.contains(
        "2. Red panda, again\n   https://zoo.example/panda\n   Image: https://img.example/2.jpg"
    ));
    assert!(text.contains("3. No image\n   https://zoo.example/text"));
    assert_eq!(text.matches("Image: ").count(), 2);

    let markdown = render_markdown(&image_fixture(), "red panda", 5);
    assert!(markdown.contains(
        "## [Red panda](https://zoo.example/panda)\n\n![Red panda](https://img.example/1.jpg)"
    ));
    assert!(markdown.contains("![Red panda, again](https://img.example/2.jpg)"));
}

#[test]
fn an_image_search_without_images_says_so() {
    let mut none = image_fixture();
    none.results.clear();
    none.provider_data = None;
    none.status = SearchStatus::Empty;
    assert_eq!(
        render_text(&none, "red panda", 5),
        "No images found for: red panda (via Brave)"
    );
}

fn listed(count: usize) -> ExecuteToolResponse {
    let mut many = response(Some(Role::Search));
    many.results = (0..count)
        .map(|i| SearchResult {
            title: format!("r{i}"),
            url: format!("https://e.example/{i}"),
            snippet: None,
            published: None,
        })
        .collect();
    many.citations = many
        .results
        .iter()
        .map(|r| Citation {
            url: r.url.clone(),
            title: Some(r.title.clone()),
        })
        .collect();
    many
}

#[test]
fn default_options_keep_the_sources_block() {
    let text = render_text(&listed(5), "q", 5);
    assert!(text.contains("Sources:"));
    assert!(render_markdown(&listed(5), "q", 5).contains("### Sources"));
}

#[test]
fn compact_drops_sources_that_repeat_every_listed_result() {
    let options = RenderOptions::compact();
    let text = render_text_with(&listed(5), "q", 5, &options);
    assert!(!text.contains("Sources:"));
    assert_eq!(text.matches("https://e.example/0").count(), 1);
    let markdown = render_markdown_with(&listed(5), "q", 5, &options);
    assert!(!markdown.contains("### Sources"));
    assert_eq!(markdown.matches("https://e.example/0").count(), 1);
}

#[test]
fn compact_still_shows_a_citation_that_is_not_a_result() {
    let mut extra = listed(2);
    extra.citations.push(Citation {
        url: "https://other.example/page".into(),
        title: Some("Other".into()),
    });
    let options = RenderOptions::compact();
    let text = render_text_with(&extra, "q", 5, &options);
    assert!(text.contains("Sources:"));
    assert!(text.contains("[3] Other — https://other.example/page"));
    assert_eq!(text.matches("https://e.example/0").count(), 1);
    let markdown = render_markdown_with(&extra, "q", 5, &options);
    assert!(markdown.contains("- [Other](https://other.example/page)"));
    assert_eq!(markdown.matches("https://e.example/0").count(), 1);
}

#[test]
fn compact_treats_a_result_hidden_by_max_results_as_not_listed() {
    let text = render_text_with(&listed(4), "q", 2, &RenderOptions::compact());
    assert!(!text.contains("[1]"));
    assert!(text.contains("[3] r2 — https://e.example/2"));
    assert!(text.contains("[4] r3 — https://e.example/3"));
}

#[test]
fn compact_caps_the_sources_at_max_results() {
    let mut answer = answer_fixture();
    answer.citations = (0..6)
        .map(|i| Citation {
            url: format!("https://c.example/{i}"),
            title: None,
        })
        .collect();
    let options = RenderOptions::compact();
    let text = render_text_with(&answer, "q", 3, &options);
    assert!(text.contains("[3] https://c.example/2"));
    assert!(!text.contains("https://c.example/3"));
    let markdown = render_markdown_with(&answer, "q", 3, &options);
    assert!(markdown.contains("https://c.example/2"));
    assert!(!markdown.contains("https://c.example/3"));
}

#[test]
fn excerpt_length_is_configurable() {
    let mut long = response(Some(Role::Search));
    long.results[0].snippet = Some("x".repeat(600));
    let options = RenderOptions {
        excerpt_chars: 100,
        ..RenderOptions::default()
    };
    let text = render_text_with(&long, "q", 5, &options);
    assert!(text.contains(&format!("{}…", "x".repeat(100))));
    assert!(!text.contains(&"x".repeat(101)));
    let markdown = render_markdown_with(&long, "q", 5, &options);
    assert!(markdown.contains(&format!("{}…", "x".repeat(100))));
    assert!(!markdown.contains(&"x".repeat(101)));
}

#[test]
fn contents_pages_are_bounded_by_the_contents_cap() {
    let mut page = response(Some(Role::Contents));
    page.results[0].snippet = Some("y".repeat(4000));
    let options = RenderOptions {
        contents_chars: 1500,
        ..RenderOptions::default()
    };
    let text = render_text_with(&page, "https://tokio.rs/select", 5, &options);
    assert!(text.contains(&format!("{}…", "y".repeat(1500))));
    assert!(!text.contains(&"y".repeat(1501)));
    // The search excerpt cap does not apply to contents.
    let narrow = RenderOptions {
        excerpt_chars: 50,
        contents_chars: 200,
        ..RenderOptions::default()
    };
    let text = render_text_with(&page, "u", 5, &narrow);
    assert!(text.contains(&"y".repeat(200)));
    assert!(!text.contains(&"y".repeat(201)));
}

#[test]
fn compact_matches_repeated_sources_by_case_insensitive_host_only() {
    let mut r = listed(1);
    r.results[0].url = "https://Example.COM/Path".into();
    r.citations = vec![
        Citation {
            url: "HTTPS://example.com/Path".into(),
            title: None,
        },
        Citation {
            url: "https://example.com/path".into(),
            title: None,
        },
    ];
    let text = render_text_with(&r, "q", 5, &RenderOptions::compact());
    assert!(!text.contains("[1]"));
    assert!(text.contains("[2] https://example.com/path"));
}

#[test]
fn contents_answer_copy_of_the_page_is_left_out_under_a_budget_only() {
    let mut page = response(Some(Role::Contents));
    page.provider = "tinyfish".into();
    page.results[0].snippet = Some("y".repeat(4000));
    page.answer = Some("y".repeat(4000));
    let narrow = RenderOptions {
        contents_chars: 200,
        ..RenderOptions::default()
    };
    for out in [
        render_text_with(&page, "u", 5, &narrow),
        render_markdown_with(&page, "u", 5, &narrow),
    ] {
        assert!(!out.contains(&"y".repeat(201)));
    }
    // Default output is unchanged.
    assert!(render_text(&page, "u", 5).contains(&"y".repeat(4000)));
    // Answer-role responses are never trimmed.
    let mut answer = answer_fixture();
    answer.answer = Some("a".repeat(4000));
    let out = render_text_with(&answer, "q", 5, &RenderOptions::compact());
    assert!(out.contains(&"a".repeat(4000)));
}

#[test]
fn the_default_contents_cap_matches_todays_output() {
    let mut page = response(Some(Role::Contents));
    page.results[0].snippet = Some("y".repeat(900));
    let text = render_text(&page, "u", 5);
    assert!(text.contains(&format!("{}…", "y".repeat(500))));
    assert!(!text.contains(&"y".repeat(501)));
}

#[test]
fn render_options_round_trip_and_default_missing_fields() -> Result<(), serde_json::Error> {
    let parsed: RenderOptions = serde_json::from_str(r#"{"compact_sources":true}"#)?;
    assert!(parsed.compact_sources);
    assert_eq!(parsed.excerpt_chars, TEXT_EXCERPT_CHARS);
    assert_eq!(parsed.contents_chars, TEXT_EXCERPT_CHARS);
    assert_eq!(RenderOptions::compact().contents_chars, 1500);
    assert_eq!(RenderOptions::default(), serde_json::from_str("{}")?);
    Ok(())
}
