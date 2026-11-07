use super::*;

fn search(arguments: Value) -> ExecuteToolRequest {
    ExecuteToolRequest {
        name: "exa_search".into(),
        arguments,
    }
}

#[test]
fn search_defaults_to_the_low_latency_type() -> Result<()> {
    let (path, body) = exa_body(&search(json!({"query":"rust"})))?;
    assert_eq!(path, "/search");
    assert_eq!(body["type"], "lite");
    Ok(())
}

#[test]
fn search_keeps_a_type_the_caller_chose() -> Result<()> {
    let (_, body) = exa_body(&search(json!({"query":"rust","type":"deep"})))?;
    assert_eq!(body["type"], "deep");
    Ok(())
}

#[test]
fn default_search_sends_only_what_the_lite_type_accepts() -> Result<()> {
    // Exa rejects a `lite` request that sets anything outside its short list.
    let (_, body) = exa_body(&search(json!({
        "query":"rust",
        "max_results":3,
        "category":"news",
        "include_domains":["example.org"],
        "exclude_domains":["spam.org"],
        "start_published_date":"2026-01-01",
        "end_published_date":"2026-02-01",
        "include_text":true,
        "include_highlights":true
    })))?;
    let mut sent: Vec<&str> = body
        .as_object()
        .ok_or(Error::InvalidArguments)?
        .keys()
        .map(String::as_str)
        .collect();
    sent.sort_unstable();
    assert_eq!(
        sent,
        [
            "category",
            "contents",
            "endPublishedDate",
            "excludeDomains",
            "includeDomains",
            "numResults",
            "query",
            "startPublishedDate",
            "type"
        ]
    );
    assert_eq!(body["contents"], json!({"text":true,"highlights":true}));
    Ok(())
}

#[test]
fn only_search_takes_a_type() -> Result<()> {
    let (_, body) = exa_body(&ExecuteToolRequest {
        name: "exa_find_similar".into(),
        arguments: json!({"url":"https://example.org"}),
    })?;
    assert!(body.get("type").is_none());
    Ok(())
}

fn contents(arguments: Value) -> ExecuteToolRequest {
    ExecuteToolRequest {
        name: "exa_get_contents".into(),
        arguments,
    }
}

#[test]
fn contents_ask_for_whole_pages_unless_capped() -> Result<()> {
    let (_, body) = exa_body(&contents(json!({"urls":["https://a.example"]})))?;
    assert_eq!(body["text"], true);
    Ok(())
}

#[test]
fn contents_cap_each_page_at_max_characters() -> Result<()> {
    let (path, body) = exa_body(&contents(
        json!({"urls":["https://a.example"],"max_characters":1500}),
    ))?;
    assert_eq!(path, "/contents");
    assert_eq!(body["text"], json!({"maxCharacters":1500}));
    Ok(())
}

#[test]
fn a_zero_or_malformed_cap_is_ignored() -> Result<()> {
    for cap in [json!(0), json!("many"), json!(-5)] {
        let (_, body) = exa_body(&contents(
            json!({"urls":["https://a.example"],"max_characters":cap}),
        ))?;
        assert_eq!(body["text"], true);
    }
    Ok(())
}
