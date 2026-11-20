use kuchiki::traits::TendrilSink;
use org_defuddle_core::{
    parse_html_to_org, render_html_fragment_to_org, youtube_inline_caption_info, DefuddleOptions,
    IncludeReplies,
};
use pulldown_cmark::{html::push_html, Options, Parser};

#[test]
fn youtube_inline_json_with_braces_in_title() {
    let player = serde_json::json!({
        "videoDetails": {"videoId": "test123", "title": "A } brace"},
        "captions": {"playerCaptionsTracklistRenderer": {"captionTracks": [{
            "languageCode": "en", "baseUrl": "https://www.youtube.com/api/timedtext?v=test123&lang=en"
        }]}}
    });
    let html = format!("<script>var ytInitialPlayerResponse = {player};</script>");
    assert!(youtube_inline_caption_info(
        &html,
        Some("https://www.youtube.com/watch?v=test123"),
        None
    )
    .unwrap()
    .is_some());
}

#[test]
fn substack_body_from_preloads_json() {
    let preload = serde_json::json!({"post": {
        "title": "Preloads JSON Post", "publishedBylines": [{"name": "Alice Writer"}],
        "subtitle": "A detailed subtitle", "post_date": "2026-11-12T17:31:44.927Z",
        "body_html": "<p>Hello from preloads-json body.</p>"
    }});
    let html = format!("<html><head><title>Fallback</title></head><body><script id=\"preloads-json\" type=\"application/json\">{preload}</script></body></html>");
    let output = parse_html_to_org(
        &html,
        DefuddleOptions {
            url: Some("https://example.substack.com/p/preloads-json-post".into()),
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert_eq!(output.title, "Preloads JSON Post");
    assert_eq!(output.author, "Alice Writer");
    assert_eq!(output.description, "A detailed subtitle");
    assert_eq!(output.published, "2026-11-12T17:31:44.927Z");
    assert!(
        output.org.contains("Hello from preloads-json body."),
        "{}",
        output.org
    );
}

#[test]
fn math_title_payload_does_not_become_live_markup() {
    for math in [
        "<math><mi>x</mi><title>&lt;img src=x onerror=alert(1)&gt;</title></math>",
        "<math><mrow><title>&lt;img src=x onerror=alert(1)&gt;</title><mi>y</mi></mrow></math>",
    ] {
        let html = format!("<article><h1>Math title</h1><p>A detailed article about a simple model and its derivation with worked examples.</p><p>Value {math} here.</p><p>More detailed conclusions about the model follow.</p></article>");
        let output = parse_html_to_org(
            &html,
            DefuddleOptions {
                url: Some("https://example.com/math".into()),
                markdown: true,
                ..DefuddleOptions::default()
            },
        )
        .unwrap();
        let reparsed = kuchiki::parse_html().one(output.html.clone());
        assert_eq!(
            reparsed.select("[onerror]").unwrap().count(),
            0,
            "{}",
            output.html
        );
        assert_eq!(reparsed.select("math title").unwrap().count(), 0);
        assert!(output.org.contains("Value"));
    }
}

#[test]
fn mastodon_missing_handles_do_not_merge_comments_into_post() {
    let html = "<html><body><div class=\"detailed-status\"><div class=\"status__content__text\"><p>Main post body.</p></div></div><div class=\"status status-public\"><div class=\"status__content__text\"><p>Reply from another person.</p></div></div></body></html>";
    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            url: Some("https://mastodon.social/@user/12345678".into()),
            include_replies: IncludeReplies::None,
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert!(output.org.contains("Main post body."));
    assert!(
        !output.org.contains("Reply from another person."),
        "{}",
        output.org
    );
}

#[test]
fn mixed_footnote_collectors_keep_all_definitions() {
    for (html, definitions) in [
        (
            r##"<p>Side<span class="sidenote-container"><span class="sidenote">Sidenote text</span></span> and list<sup id="fnref:1"><a href="#fn:1">1</a></sup>.</p><div class="footnotes"><ol><li id="fn:1"><p>List text <a href="#fnref:1">↩</a></p></li></ol></div>"##,
            vec!["Sidenote text", "List text"],
        ),
        (
            r##"<p>A<span data-definition="note-a"><a href="#">*</a></span> B<sup id="fnref:b"><a href="#fn:b">1</a></sup> C<sup id="fnref:c"><a href="#fn:c">2</a></sup>.</p><aside id="note-a" style="display:none">Aside text</aside><div class="footnotes"><ol><li id="fn:b"><p>Note B</p></li><li id="fn:c"><p>Note C</p></li></ol></div>"##,
            vec!["Aside text", "Note B", "Note C"],
        ),
    ] {
        let org = render_html_fragment_to_org(html);
        let count = org.lines().filter(|line| line.starts_with("[fn:")).count();
        assert_eq!(count, definitions.len(), "{org}");
        for definition in definitions {
            assert!(org.contains(definition), "{org}");
        }
        let output = parse_html_to_org(
            &format!("<article>{html}</article>"),
            DefuddleOptions {
                content_selector: Some("article".into()),
                ..DefuddleOptions::default()
            },
        )
        .unwrap();
        let document = kuchiki::parse_html().one(output.html);
        let ids = document
            .select("[id]")
            .unwrap()
            .map(|node| node.attributes.borrow().get("id").unwrap().to_string())
            .collect::<std::collections::HashSet<_>>();
        for link in document.select("a[href]").unwrap() {
            let attrs = link.attributes.borrow();
            if let Some(target) = attrs.get("href").and_then(|href| href.strip_prefix("#fn")) {
                assert!(
                    ids.contains(&format!("fn{target}")),
                    "missing note or return target: {target}"
                );
            }
        }
    }
}

#[test]
fn latexml_notes_have_one_definition_and_return_links_in_html() {
    let note = |id: &str| {
        format!(
            r#"<span id="{id}" class="ltx_note ltx_role_footnote"><sup class="ltx_note_mark">1</sup><span class="ltx_note_outer" style="display:none"><span class="ltx_note_content"><sup class="ltx_note_mark">1</sup><span class="ltx_tag ltx_tag_note">1</span>Unique explanatory note.</span></span></span>"#
        )
    };
    let html = format!(
        "<article><p>The first observation{} and the second{} use the same note.</p></article>",
        note("n1"),
        note("n2")
    );
    let output = parse_html_to_org(
        &html,
        DefuddleOptions {
            content_selector: Some("article".into()),
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert!(output.org.contains("[fn:1] Unique explanatory note."));
    let document = kuchiki::parse_html().one(output.html.clone());
    assert_eq!(
        document.select("#footnotes li").unwrap().count(),
        1,
        "{}",
        output.html
    );
    assert_eq!(
        document.select("sup a[href=\"#fn:1\"]").unwrap().count(),
        2,
        "{}",
        output.html
    );
    assert_eq!(document.select(".footnote-backref").unwrap().count(), 2);
    assert_eq!(document.select(".ltx_note_content").unwrap().count(), 0);
}

#[test]
fn redesigned_x_keeps_quotes_separate_and_honors_reply_and_image_options() {
    let html = r#"<html><body><ul>
      <li><article><a href="/alice">Alice</a><a href="/alice/status/2120597035028255136">Date</a>
        <div dir="auto" lang="en">Main post.
Second paragraph.</div><img src="https://pbs.twimg.com/media/main.jpg" alt="Main image">
        <div data-slot="embedded-tweet-frame"><article><a href="/bob">Bob</a><a href="/bob/status/2120597035028255137">Date</a>
          <div dir="auto" lang="en">Quoted post.</div><img src="https://pbs.twimg.com/media/quoted.jpg" alt="Quoted image">
        </article></div>
      </article></li>
      <li><article><a href="/alice">Alice</a><a href="/alice/status/2120597035028255138">Date</a>
        <div dir="auto" lang="en">Separate reply by the same author.</div>
      </article></li>
    </ul></body></html>"#;
    for include_images in [true, false] {
        let output = parse_html_to_org(
            html,
            DefuddleOptions {
                url: Some("https://x.com/alice/status/2120597035028255136".into()),
                include_images,
                include_replies: IncludeReplies::None,
                ..DefuddleOptions::default()
            },
        )
        .unwrap();
        assert_eq!(output.extractor_type.as_deref(), Some("twitter"));
        assert_eq!(output.author, "@alice");
        assert!(
            output.org.contains("Main post.\n\nSecond paragraph."),
            "{}",
            output.org
        );
        assert!(output.org.contains("*Bob @bob*"));
        assert!(output.org.contains("Quoted post."));
        assert!(!output.org.contains("Separate reply"));
        assert_eq!(
            output.org.matches("media/main.jpg").count(),
            usize::from(include_images)
        );
        assert_eq!(
            output.org.matches("media/quoted.jpg").count(),
            usize::from(include_images)
        );
    }
}

#[test]
fn headings_keep_math_and_note_references_without_navigation_labels() {
    let html = r##"<article><h2>Value <math data-latex="x^2"><mi>x</mi></math> <sup><a role="doc-noteref" href="#note">1</a></sup><span class="permalink-widget"><button>Copy link</button></span></h2>
      <p>The equation is explained in the accompanying note.</p>
      <h2><span class="transactions">Transaction history</span></h2><p>These interactions are article content.</p>
      <div class="footnotes"><ol><li id="note">Conditions for the measurement.</li></ol></div></article>"##;
    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            content_selector: Some("article".into()),
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert!(
        output.org.contains("** Value $x^2$ [fn:1]"),
        "{}",
        output.org
    );
    assert!(output
        .org
        .contains("[fn:1] Conditions for the measurement."));
    assert!(!output.org.contains("Copy link"));
    assert!(output.org.contains("** Transaction history"));
}

#[test]
fn legacy_substack_preloads_keep_metadata_and_sanitize_body() {
    let preload = serde_json::json!({"feedData": {"initialPost": {"post": {
        "title": "Legacy Post", "publishedBylines": [{"name": "Bob Writer"}],
        "body_html": "<p>Legacy post body.</p><img src=\"https://example.com/photo.jpg\"><script>alert(1)</script><a href=\"javascript:alert(1)\">Link text</a>"
    }}}});
    let encoded = serde_json::to_string(&preload.to_string())
        .unwrap()
        .replace('<', "\\u003c");
    let html = format!(
        "<html><body><script>window._preloads = JSON.parse({encoded})</script></body></html>"
    );
    let output = parse_html_to_org(
        &html,
        DefuddleOptions {
            url: Some("https://writer.example/p/legacy".into()),
            include_images: false,
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert_eq!(output.title, "Legacy Post");
    assert_eq!(output.author, "Bob Writer");
    assert!(output.org.contains("Legacy post body."));
    assert!(!output.html.contains("<img"));
    assert!(!output.html.contains("<script"));
    assert!(!output.html.contains("javascript:"));
}

#[test]
fn selector_cleanup_preserves_data_cells_and_section_heading_ids() {
    let html = r#"<article><p>These measurements and observations belong to the report.</p>
      <table><thead><tr><th>Recorded date</th></tr></thead><tbody><tr class="related"><td class="date">2026-11-19</td></tr></tbody></table>
      <table><tr><td class="date">Layout date label</td></tr></table>
      <section class="level2" id="related-work"><h2>Related work</h2><p>Keep this section.</p></section>
      <p>Article conclusion.</p></article>"#;
    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            content_selector: Some("article".into()),
            remove_content_patterns: false,
            remove_low_scoring: false,
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert!(output.org.contains("2026-11-19"), "{}", output.org);
    assert!(output.org.contains("Keep this section."));
    assert!(!output.org.contains("Layout date label"));
}

fn parse_article(html: &str) -> org_defuddle_core::DefuddleOutput {
    parse_html_to_org(
        html,
        DefuddleOptions {
            content_selector: Some("article".into()),
            ..DefuddleOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn block_code_keeps_other_paragraph_content() {
    let output = parse_article(
        r#"<article><p><img src="https://example.com/chart.png" alt="Chart"><code>one()<br>two()</code></p><p>Conclusion.</p></article>"#,
    );
    assert!(output.org.contains("chart.png"), "{}", output.org);
    assert!(output.org.contains("#+begin_src\none()\ntwo()\n#+end_src"));
}

#[test]
fn heading_symbol_footnotes_remain_references() {
    for role in [r#" role="doc-noteref""#, ""] {
        let output = parse_article(&format!(
            r##"<article><h2>Finding<sup class="footnote-reference"><a{role} href="#note">*</a></sup></h2><p>Explanation.</p><div class="footnotes"><ol><li id="note">Required qualification.</li></ol></div></article>"##
        ));
        assert!(output.org.contains("** Finding [fn:1]"), "{}", output.org);
    }
}

#[test]
fn heading_math_is_not_escaped_as_org_emphasis() {
    let output = parse_article(
        r#"<article><h2>Formula <math data-latex="\text{/literal/}"><mi>x</mi></math></h2><p>Explanation.</p></article>"#,
    );
    assert!(
        output.org.contains(r"$\text{/literal/}$"),
        "{:?}",
        output.org
    );
}

#[test]
fn redesigned_x_does_not_take_date_from_quote() {
    for id in ["2120597035028255136", "123"] {
        let html = format!(
            r#"<article><a href="/alice">Alice</a><a href="/alice/status/{id}">Today</a><div dir="auto" lang="en">Current post.</div><div data-slot="embedded-tweet-frame"><article><a href="/bob">Bob</a><a href="/bob/status/1688675125388288000"><time datetime="2023-08-07T00:00:00Z">Old</time></a><div dir="auto" lang="en">Old quoted post.</div></article></div></article>"#
        );
        let output = parse_html_to_org(
            &html,
            DefuddleOptions {
                url: Some(format!("https://x.com/alice/status/{id}")),
                ..DefuddleOptions::default()
            },
        )
        .unwrap();
        if id == "123" {
            assert!(output.published.is_empty(), "{}", output.published);
        } else {
            assert!(
                output.published.starts_with("2026-11-11"),
                "{}",
                output.published
            );
        }
    }
}

#[test]
fn x_emoji_images_become_text() {
    let html = r#"<article><a href="/alice">Alice</a><a href="/alice/status/2120597035028255136">Today</a><div dir="auto" lang="en">Hello <img src="https://abs-0.twimg.com/emoji/v2/svg/1f600.svg" alt="😀"> world.</div></article>"#;
    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            url: Some("https://x.com/alice/status/2120597035028255136".into()),
            include_images: false,
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert!(output.org.contains("Hello 😀 world."), "{}", output.org);
    assert!(!output.org.contains("/emoji/"));
}

#[test]
fn distinct_note_links_are_not_deduplicated() {
    let note = |id: &str, url: &str| {
        format!(
            r#"<span id="{id}" class="ltx_note ltx_role_footnote"><sup class="ltx_note_mark">1</sup><span class="ltx_note_outer"><span class="ltx_note_content"><a href="{url}">Source</a></span></span></span>"#
        )
    };
    let output = parse_article(&format!(
        "<article><p>First{} second{}.</p></article>",
        note("n1", "https://example.com/first"),
        note("n2", "https://example.com/second")
    ));
    assert!(output.org.contains("https://example.com/first"));
    assert!(
        output.org.contains("https://example.com/second"),
        "{}",
        output.org
    );
}

#[test]
fn legacy_substack_accepts_minified_assignment() {
    let preload =
        serde_json::json!({"post": {"title":"Saved post", "body_html":"<p>Important body.</p>"}});
    let encoded = serde_json::to_string(&preload.to_string()).unwrap();
    let html = format!(
        "<html><body><script>window._preloads=JSON.parse({encoded})</script></body></html>"
    );
    let output = parse_html_to_org(
        &html,
        DefuddleOptions {
            url: Some("https://example.substack.com/p/test".into()),
            ..DefuddleOptions::default()
        },
    )
    .unwrap();
    assert_eq!(output.title, "Saved post");
    assert!(output.org.contains("Important body."));
}

#[test]
fn favicon_uses_upstream_metadata_priority_and_http_origin() {
    for (head, url, expected) in [
        (
            "",
            "https://example.com/posts/article",
            "https://example.com/favicon.ico",
        ),
        (
            r#"<meta property="og:image:favicon" content="https://cdn.example/icon.png"><link rel="icon" href="/other.ico">"#,
            "https://example.com/posts/article",
            "https://cdn.example/icon.png",
        ),
        ("", "file:///tmp/article.html", ""),
    ] {
        let output = parse_html_to_org(
            &format!("<html><head>{head}</head><body><article><p>Article body.</p></article></body></html>"),
            DefuddleOptions { url: Some(url.into()), ..Default::default() },
        ).unwrap();
        assert_eq!(output.favicon, expected);
    }
}

#[test]
fn response_schema_graph_exposes_its_entities() {
    let html = r#"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"Article","headline":"Graph title"},{"@type":"WebPage","name":"Graph page"}]}</script><article><p>Article body.</p></article>"#;
    let output = parse_article(html);
    let json = org_defuddle_core::output_json_value_for_html(&output, html);
    assert_eq!(
        json["schemaOrgData"],
        serde_json::json!([
            {"@type":"Article","headline":"Graph title"},
            {"@type":"WebPage","name":"Graph page"}
        ])
    );
}

#[test]
fn response_meta_tags_match_upstream_nullable_keys_and_content_filter() {
    let html = r#"<meta charset="utf-8"><meta name="missing"><meta name="empty" content=""><meta name="description" content="A &amp; B"><meta property="og:title" content="Article"><article><p>Article body.</p></article>"#;
    let output = parse_article(html);
    let json = org_defuddle_core::output_json_value_for_html(&output, html);
    assert_eq!(
        json["metaTags"],
        serde_json::json!([
            {"name":"description","property":null,"content":"A & B"},
            {"name":null,"property":"og:title","content":"Article"}
        ])
    );
}

#[test]
fn inline_code_does_not_match_unrelated_pre_style_text() {
    let output = parse_article(
        r#"<article><p>Inline <code style="white-space:normal;--preview-color:red">value()</code> remains in prose.</p></article>"#,
    );
    assert!(!output.org.contains("#+begin_src"), "{}", output.org);
    assert!(output.org.contains("Inline ~value()~ remains in prose."));
}

#[test]
fn substack_preload_body_is_parsed_before_attaching_its_wrapper() {
    let preload = serde_json::json!({"post": {
        "title":"Recovered body", "body_html":"<p>First paragraph.</p></div><p>Last paragraph.</p>"
    }});
    let output = parse_html_to_org(
        &format!("<script id=\"preloads-json\" type=\"application/json\">{preload}</script>"),
        DefuddleOptions {
            url: Some("https://example.substack.com/p/recovered".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(output.org.contains("First paragraph."));
    assert!(output.org.contains("Last paragraph."), "{}", output.org);
}

fn markdown_document(markdown: &str, options: Options) -> kuchiki::NodeRef {
    let mut html = String::new();
    push_html(&mut html, Parser::new_ext(markdown, options));
    kuchiki::parse_html().one(html)
}

#[test]
fn markdown_inline_code_preserves_spaces_and_backtick_runs() {
    for (source, expected) in [
        ("a  b", "a  b"),
        ("  a  ", "  a  "),
        ("   ", "   "),
        ("\ta\t", "\ta\t"),
        ("a\n b", "a  b"),
        ("a\n\n\nb", "a   b"),
        ("a\n\n\n<em>b</em>", "a   *b*"),
        ("a `` b ` c", "a `` b ` c"),
        ("`edge``", "`edge``"),
        ("a\tb", "a\tb"),
        ("a\u{a0}b", "a\u{a0}b"),
        ("<span>  a  </span>", "  a  "),
        ("a<span>  b  </span>c", "a  b  c"),
        ("<span><span> a </span></span>", " a "),
        ("<span> a </span><span> b </span>", " a  b "),
        ("<span>\ta\t</span>", "\ta\t"),
        ("<em>  a  </em>", "*  a  *"),
        ("<em> </em>", " "),
        ("a<em>  </em>b", "a  b"),
        ("<strong>  </strong>", "  "),
        ("a<strong>\t</strong>b", "a\tb"),
        ("<strong><em>  </em></strong>", "  "),
        ("a<del>\u{a0}</del>b", "a\u{a0}b"),
    ] {
        for standardize in [false, true] {
            let output = parse_html_to_org(
                &format!("<article><p>Use <code>{source}</code> here.</p></article>"),
                DefuddleOptions {
                    content_selector: Some("article".into()),
                    standardize,
                    separate_markdown: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let document = markdown_document(&output.content_markdown, Options::empty());
            assert_eq!(
                document.select_first("code").unwrap().text_contents(),
                expected,
                "source={source:?}, standardize={standardize}, Markdown={:?}",
                output.content_markdown
            );
        }
    }
}

#[test]
fn markdown_keeps_continuation_blocks_inside_footnotes() {
    let code = "<pre><code>first line\n\n  last line</code></pre>";
    let block_task = format!(
        r#"<ul><li class="task-list-item"><input type="checkbox" checked>{code}</li></ul>"#
    );
    let inline_task =
        r#"<ul><li class="task-list-item"><input type="checkbox"><code>a``b</code> tail</li></ul>"#;
    let block_text = "first line\n\n  last line\n";
    for (body, code_selector, expected_code) in [
        (code, "pre > code", block_text),
        (inline_task, "li > input:not([checked]) + code", "a``b"),
        (
            block_task.as_str(),
            "li > input[checked] + pre > code",
            block_text,
        ),
    ] {
        let notes = format!(
            r##"<p>Text<sup><a href="#fn:one">1</a></sup><sup><a href="#fn:two">2</a></sup>.</p>
                <div id="footnotes"><ol><li id="fn:one">{body}<p>Last paragraph.</p></li>
                <li id="fn:two"><p>Second note.</p></li></ol></div><p>After notes.</p>"##
        );
        for container in [notes.clone(), format!("<ul><li>{notes}</li></ul>")] {
            for standardize in [false, true] {
                let output = parse_html_to_org(
                    &format!("<article>{container}</article>"),
                    DefuddleOptions {
                        content_selector: Some("article".into()),
                        standardize,
                        separate_markdown: true,
                        ..Default::default()
                    },
                )
                .unwrap();
                let document = markdown_document(
                    &output.content_markdown,
                    Options::ENABLE_FOOTNOTES | Options::ENABLE_TASKLISTS,
                );
                let mut definitions = document.select(".footnote-definition").unwrap();
                let first = definitions.next().unwrap();
                let first = first.as_node();
                assert_eq!(
                    first
                        .select_first(code_selector)
                        .unwrap_or_else(|_| panic!(
                            "missing {code_selector} in {}",
                            output.content_markdown
                        ))
                        .text_contents(),
                    expected_code
                );
                assert_eq!(
                    first.select_first("p:last-child").unwrap().text_contents(),
                    "Last paragraph."
                );
                assert!(definitions
                    .next()
                    .unwrap()
                    .text_contents()
                    .contains("Second note."));
                assert!(definitions.next().is_none());
                assert!(document
                    .select("body > p, body > ul > li > p")
                    .unwrap()
                    .any(|p| p.text_contents() == "After notes."));
            }
        }
    }
}

#[test]
fn markdown_equation_rows_keep_separate_formulas_and_literal_code() {
    for (first, second, literal) in [
        ("a", "b", false),
        (r"\text{[a](b)}", r"\text{[c](d)}", true),
    ] {
        let html = format!(
            r#"<article><p>Before.</p><table class="ltx_equation"><tr class="ltx_eqn_inline"><td><math><annotation encoding="application/x-tex">{first}</annotation></math></td></tr><tr class="ltx_eqn_inline"><td><math><annotation encoding="application/x-tex">{second}</annotation></math></td></tr></table><p>After.</p></article>"#
        );
        for standardize in [false, true] {
            let output = parse_html_to_org(
                &html,
                DefuddleOptions {
                    content_selector: Some("article".into()),
                    standardize,
                    separate_markdown: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let document = markdown_document(&output.content_markdown, Options::ENABLE_MATH);
            let paragraphs = document
                .select("p")
                .unwrap()
                .map(|node| node.text_contents())
                .collect::<Vec<_>>();
            assert_eq!(
                paragraphs,
                ["Before.", first, second, "After."],
                "{}",
                output.content_markdown
            );
            let formulas = document
                .select(if literal { "code" } else { ".math" })
                .unwrap()
                .map(|node| node.text_contents())
                .collect::<Vec<_>>();
            assert_eq!(formulas, [first, second], "{}", output.content_markdown);
        }
    }
}

#[test]
fn markdown_literal_math_does_not_expose_autolinks_without_math_support() {
    for latex in [
        r"\text{<https://example.com>}",
        r"\text{<mailto:reader@example.com>}",
        r"\text{a `` <https://example.com> `` b}",
    ] {
        for block in [false, true] {
            let math = format!(
                r#"<math data-latex="{}"><mi>x</mi></math>"#,
                latex.replace('<', "&lt;").replace('>', "&gt;")
            );
            let body = if block {
                format!("<p>{math}</p>")
            } else {
                format!("<p>Before {math} after.</p>")
            };
            let output = parse_html_to_org(
                &format!("<article>{body}</article>"),
                DefuddleOptions {
                    content_selector: Some("article".into()),
                    separate_markdown: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let document = markdown_document(&output.content_markdown, Options::empty());
            assert_eq!(
                document.select("a").unwrap().count(),
                0,
                "{}",
                output.content_markdown
            );
            assert_eq!(
                document
                    .select_first("code")
                    .unwrap()
                    .text_contents()
                    .trim_end_matches('\n'),
                latex,
                "{}",
                output.content_markdown
            );
        }
    }
}

#[test]
fn markdown_keeps_simple_explicit_latex_before_mathml_and_alttext() {
    for attributes in [r#"data-latex="A, B""#, r#"data-latex="A, B" alttext="C""#] {
        let output = parse_html_to_org(
            &format!(
                "<article><p>Before <math {attributes}><mi>x</mi></math> after.</p></article>"
            ),
            DefuddleOptions {
                content_selector: Some("article".into()),
                separate_markdown: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            output.content_markdown.contains("$A, B$"),
            "{}",
            output.content_markdown
        );
    }
}

#[test]
fn markdown_output_fields_do_not_reactivate_encoded_html() {
    let payload = "&lt;img src=x onerror=alert(1)&gt;";
    let snippets = [
        (
            format!("<figure><img src='https://example.com/photo.png' alt='x)\n\n{payload}\n\n![y](z)'><figcaption>Caption</figcaption></figure>"),
            "Caption",
            Some("x) <img src=x onerror=alert(1)> ![y](z)"),
        ),
        (
            format!(r#"<div class="admonition"><p class="admonition-title">{payload}</p><p>Body.</p></div>"#),
            "<img src=x onerror=alert(1)>",
            None,
        ),
        (
            format!(r#"<figure><img src="https://example.com/photo.png"><figcaption><span class="ltx_tag_figure">{payload}</span>Caption</figcaption></figure>"#),
            "<img src=x onerror=alert(1)>",
            None,
        ),
        (
            format!("<p>Reference <sup id=\"fnref:x]\n\n{payload}\n\n[^y\">1</sup></p>"),
            "onerror%003dalert%00281%0029",
            None,
        ),
        (
            format!("<div id=\"footnotes\"><ol><li id=\"fn:x]\n\n{payload}\n\n[^y\">Note</li></ol></div>"),
            "Note",
            None,
        ),
        (
            format!(r#"<table class="ltx_equation"><tr><td><math><annotation encoding="application/x-tex">{payload}</annotation></math></td></tr></table>"#),
            "< img src=x onerror=alert(1)>",
            None,
        ),
        (
            "<p>&lt;javascript:alert(1)&gt; &lt;javascript:alert(1)<span>&gt;</span></p>".into(),
            "<javascript:alert(1)> <javascript:alert(1)>",
            None,
        ),
        (
            "<p>&lt;img<span> src=x onerror=alert(1)&gt;</span></p>".into(),
            "<img src=x onerror=alert(1)>",
            None,
        ),
        (
            "<p>&lt;<span>img src=x onerror=alert(1)&gt;</span></p>".into(),
            "<img src=x onerror=alert(1)>",
            None,
        ),
    ];
    for standardize in [false, true] {
        for (snippet, expected_text, expected_alt) in &snippets {
            let output = parse_html_to_org(
                &format!("<article>{snippet}</article>"),
                DefuddleOptions {
                    content_selector: Some("article".into()),
                    standardize,
                    separate_markdown: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let document = markdown_document(&output.content_markdown, Options::ENABLE_FOOTNOTES);
            let visible_text = document
                .text_contents()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                visible_text.contains(expected_text),
                "Lost literal content: {}",
                output.content_markdown
            );
            if let Some(expected_alt) = expected_alt {
                let image = document.select_first("img").unwrap();
                assert_eq!(image.attributes.borrow().get("alt"), Some(*expected_alt));
            }
            assert_eq!(
                document
                    .select("[onerror], [onload], script, style, iframe[srcdoc]")
                    .unwrap()
                    .count(),
                0,
                "{}",
                output.content_markdown
            );
            assert_eq!(
                document.select("a").unwrap().count(),
                0,
                "{}",
                output.content_markdown
            );
        }
    }
}

#[test]
fn markdown_footnote_names_keep_hyphens_and_decode_link_targets() {
    for (reference, href, definition, expected) in [
        ("named-note-2", "#fn:named-note", "named-note", "named-note"),
        ("part-2", "", "part-2", "part-2"),
        ("part-2", "#fn:", "part-2", "part-2"),
        (
            "unicode",
            "#fn:%CE%B1%F0%9F%98%80",
            "α😀",
            "%03b1%d83d%de00",
        ),
        ("note-3", "#fn:note%20a%2Bb", "note a+b", "note%0020a%002bb"),
        (
            "note%20name%ZZ",
            "#fn:note%20name%ZZ",
            "note%20name%ZZ",
            "note%002520name%0025zz",
        ),
        (
            "note%20name%",
            "#fn:note%20name%",
            "note%20name%",
            "note%002520name%0025",
        ),
        (
            "note%20name%2",
            "#fn:note%20name%2",
            "note%20name%2",
            "note%002520name%00252",
        ),
        (
            "note%20name%C3",
            "#fn:note%20name%C3",
            "note%20name%C3",
            "note%002520name%0025c3",
        ),
        (
            "literal-percent",
            "#fn:note%2520name",
            "note%20name",
            "note%002520name",
        ),
    ] {
        let output = parse_html_to_org(&format!(r##"<article><p>Text<sup id="fnref:{reference}"><a href="{href}">1</a></sup>.</p><div id="footnotes"><ol><li id="fn:{definition}">Note body.</li></ol></div></article>"##), DefuddleOptions {
            content_selector: Some("article".into()), standardize: false, separate_markdown: true, ..Default::default()
        }).unwrap();
        assert!(
            output
                .content_markdown
                .contains(&format!("Text[^{expected}]")),
            "{}",
            output.content_markdown
        );
        assert!(
            output
                .content_markdown
                .contains(&format!("[^{expected}]: Note body.")),
            "{}",
            output.content_markdown
        );
    }
}

#[test]
fn markdown_image_metadata_stays_literal_and_caption_labels_appear_once() {
    let html = r#"<article><figure><img src="https://example.com/p(1).png?x=1&amp;y=2" alt="A [label] &lt;tag&gt; and \ &amp;lt;b&amp;gt; &amp;copy; A &amp; B"><figcaption><p><span class="ltx_tag_figure">Figure [1]</span> Caption.</p></figcaption></figure><img src="https://example.com/title.png" title="A &quot;title&quot; with \ and &lt;tag&gt;"></article>"#;
    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            content_selector: Some("article".into()),
            standardize: false,
            separate_markdown: true,
            ..Default::default()
        },
    )
    .unwrap();
    let document = markdown_document(&output.content_markdown, Options::empty());
    assert_eq!(document.select("img").unwrap().count(), 2);
    let image = document.select_first("img").unwrap();
    let attrs = image.attributes.borrow();
    assert_eq!(
        attrs.get("src"),
        Some("https://example.com/p(1).png?x=1&y=2")
    );
    assert_eq!(
        attrs.get("alt"),
        Some(r"A [label] <tag> and \ &lt;b&gt; &copy; A & B")
    );
    let titled = document.select_first("img[title]").unwrap();
    assert_eq!(
        titled.attributes.borrow().get("title"),
        Some(r#"A "title" with \ and <tag>"#)
    );
    assert_eq!(
        document
            .select_first("body")
            .unwrap()
            .text_contents()
            .matches("Figure [1]")
            .count(),
        1,
        "{}",
        output.content_markdown
    );
}
