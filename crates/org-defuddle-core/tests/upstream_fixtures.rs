use kuchiki::traits::TendrilSink;
use org_defuddle_core::{
    parse_html_to_org, render_html_fragment_to_org, DefuddleOptions, IncludeReplies,
};
use pulldown_cmark::{html, Event, Options, Parser};
use regex::Regex;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

const UPSTREAM_FIXTURE_COUNT: usize = 246;
const UPSTREAM_REVISION: &str = include_str!("../../../tests/upstream-revision");

#[derive(Debug, Deserialize)]
struct ExpectedMetadata {
    title: String,
    author: String,
    site: String,
    published: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamReference {
    name: String,
    url: String,
    content: String,
    #[serde(rename = "wordCount")]
    word_count: usize,
    #[serde(rename = "contentMarkdown")]
    content_markdown: String,
}

fn upstream_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("ORG_DEFUDDLE_DEFUDDLE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let local_tmp = PathBuf::from("/private/tmp/defuddle-elisp-source");
            local_tmp.is_dir().then_some(local_tmp)
        })?;
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&dir)
        .output()
        .expect("read upstream revision");
    assert!(head.status.success());
    assert_eq!(
        String::from_utf8_lossy(&head.stdout).trim(),
        UPSTREAM_REVISION.trim(),
        "check out tests/upstream-revision before running fixture tests"
    );
    assert_eq!(upstream_fixture_names(&dir).len(), UPSTREAM_FIXTURE_COUNT);
    Some(dir)
}

#[test]
fn all_upstream_fixture_org_regression_snapshots() {
    let Some(defuddle_dir) = upstream_dir() else {
        eprintln!(
            "skipping upstream fixture Org regression snapshots; set ORG_DEFUDDLE_DEFUDDLE_DIR to a defuddle checkout"
        );
        return;
    };

    let fixtures = upstream_fixture_names(&defuddle_dir);
    let expected_dir = expected_org_dir();
    let update = std::env::var_os("ORG_DEFUDDLE_UPDATE_EXPECTED_ORG").is_some();

    assert!(!fixtures.is_empty(), "no upstream HTML fixtures found");
    if update {
        std::fs::create_dir_all(&expected_dir)
            .unwrap_or_else(|err| panic!("failed to create {}: {err}", expected_dir.display()));
    } else {
        let expected = expected_org_snapshot_names(&expected_dir);
        assert_eq!(
            expected, fixtures,
            "Org snapshot set does not match the upstream fixture set; regenerate with ORG_DEFUDDLE_UPDATE_EXPECTED_ORG=1"
        );
    }

    for fixture in fixtures {
        let output = parse_upstream_fixture(&defuddle_dir, &fixture);
        let expected_path = expected_dir.join(format!("{fixture}.org"));
        if update {
            std::fs::write(&expected_path, &output.org)
                .unwrap_or_else(|err| panic!("failed to write {}: {err}", expected_path.display()));
            continue;
        }

        let expected_org = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", expected_path.display()));
        assert_eq!(
            output.org, expected_org,
            "fixture {fixture} exact Org mismatch"
        );
    }
}

#[test]
fn all_upstream_fixture_metadata_matches_reference() {
    let Some(defuddle_dir) = upstream_dir() else {
        eprintln!(
            "skipping upstream fixture metadata comparison; set ORG_DEFUDDLE_DEFUDDLE_DIR to a defuddle checkout"
        );
        return;
    };

    let mut mismatches = Vec::new();
    for fixture in upstream_fixture_names(&defuddle_dir) {
        let expected_path = defuddle_dir
            .join("tests")
            .join("expected")
            .join(format!("{fixture}.md"));
        let expected = read_expected_metadata(&expected_path);
        let output = parse_upstream_fixture(&defuddle_dir, &fixture);

        for (field, actual, expected) in [
            ("title", output.title.as_str(), expected.title.as_str()),
            ("author", output.author.as_str(), expected.author.as_str()),
            ("site", output.site.as_str(), expected.site.as_str()),
            (
                "published",
                output.published.as_str(),
                expected.published.as_str(),
            ),
        ] {
            if actual != expected {
                mismatches.push(format!(
                    "{fixture} {field}: actual {actual:?}, expected {expected:?}"
                ));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "upstream fixture metadata mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
#[ignore = "manual differential gate; build upstream with npm run build:node first"]
fn all_upstream_fixture_outputs_match_reference() {
    let (dir, references) = upstream_references();
    let selected = std::env::var("ORG_DEFUDDLE_DIFF_FIXTURE").is_ok();
    assert_eq!(
        references.len(),
        if selected { 1 } else { UPSTREAM_FIXTURE_COUNT }
    );
    let mut mismatches = Vec::new();
    for reference in references {
        let path = dir
            .join("tests/fixtures")
            .join(format!("{}.html", reference.name));
        let html = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let output = parse_html_to_org(
            &html,
            DefuddleOptions {
                url: Some(reference.url),
                separate_markdown: true,
                ..Default::default()
            },
        )
        .unwrap_or_else(|err| panic!("parse {}: {err}", reference.name));
        let expected_org = render_html_fragment_to_org(&reference.content);
        let actual_org = output
            .org
            .split_once("\n:END:\n\n")
            .map(|(_, body)| body)
            .unwrap_or(&output.org);
        if actual_org != expected_org {
            mismatches.push(format!(
                "{}: Org{}",
                reference.name,
                if selected {
                    format!("\n{}", first_org_line_difference(&expected_org, actual_org))
                } else {
                    String::new()
                }
            ));
        }
        if output.word_count != reference.word_count {
            mismatches.push(format!(
                "{}: wordCount expected {} actual {}",
                reference.name, reference.word_count, output.word_count
            ));
        }
        let expected_markdown = markdown_semantics(&reference.content_markdown);
        let actual_markdown = markdown_semantics(&output.content_markdown);
        if actual_markdown != expected_markdown {
            mismatches.push(format!(
                "{}: Markdown{}",
                reference.name,
                if selected {
                    format!(
                        "\n{}",
                        first_semantic_difference(&expected_markdown, &actual_markdown)
                    )
                } else {
                    String::new()
                }
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "upstream output mismatches ({}):\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn debbugs_bugreport_is_rendered_as_mail_archive() {
    let html = r##"<!doctype html>
<html>
<head><title>#70820 - [PATCH] Editable grep buffers - GNU bug report logs</title></head>
<body>
<h1>GNU bug report logs - <br>#70820<br>[PATCH] Editable grep buffers</h1>
<div class="pkginfo"><p>Package: <a>emacs</a>;</p></div>
<div class="buginfo">
  <p>Reported by: <a>Visuwesh &lt;visuweshm &lt;at&gt; gmail.com&gt;</a></p>
  <p>Date: Tue, 7 May 2024 16:26:01 UTC</p>
  <p>Severity: normal</p>
  <p>Tags: patch</p>
  <p><strong>Done:</strong> Eli Zaretskii &lt;eliz &lt;at&gt; gnu.org&gt;</p>
</div>
<div class="infmessage"><strong>Information forwarded</strong> to <code>bug-gnu-emacs</code></div>
<hr><p class="msgreceived"><a name="msg5"></a><a href="#5">Message #5</a> received at submit &lt;at&gt; debbugs.gnu.org (<a href="bugreport.cgi?bug=70820;msg=5">full text</a>, <a href="bugreport.cgi?bug=70820;msg=5;mbox=yes">mbox</a>):</p>
<pre class="headers"><b>From:</b> Visuwesh &lt;visuweshm &lt;at&gt; gmail.com&gt;
<b>To:</b> bug-gnu-emacs &lt;at&gt; gnu.org
<b>Subject:</b> [PATCH] Editable grep buffers
<b>Date:</b> Tue, 07 May 2024 21:55:09 +0530</pre>
<pre class="mime">[<a href="bugreport.cgi?msg=5;att=0">Message part 1</a> (text/plain, inline)]</pre>
<pre class="message">Tags: patch

Attached patch is a proof of concept for an occur-edit-mode alike for
*grep* buffers.
</pre>
<pre class="mime">[<a href="bugreport.cgi?att=1;msg=5;filename=grep-edit.diff">grep-edit.diff</a> (text/patch, attachment)]</pre>
<hr><p class="msgreceived"><a name="msg8"></a><a href="#8">Message #8</a> received at 70820 &lt;at&gt; debbugs.gnu.org (<a href="bugreport.cgi?bug=70820;msg=8">full text</a>, <a href="bugreport.cgi?bug=70820;msg=8;mbox=yes">mbox</a>):</p>
<pre class="headers"><b>From:</b> Jim Porter &lt;jporterbugs &lt;at&gt; gmail.com&gt;
<b>To:</b> Visuwesh &lt;visuweshm &lt;at&gt; gmail.com&gt;, 70820 &lt;at&gt; debbugs.gnu.org
<b>Subject:</b> Re: bug#70820: [PATCH] Editable grep buffers
<b>Date:</b> Tue, 7 May 2024 10:23:30 -0700</pre>
<pre class="message">How does this compare to the existing wgrep package?</pre>
</body>
</html>"##;

    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            url: Some("https://debbugs.gnu.org/cgi/bugreport.cgi?bug=70820".to_string()),
            include_images: true,
            remove_small_images: true,
            content_selector: None,
            include_replies: IncludeReplies::Extractors,
            remove_hidden_elements: true,
            remove_exact_selectors: true,
            remove_partial_selectors: true,
            remove_content_patterns: true,
            remove_low_scoring: true,
            standardize: true,
            debug: false,
            profile: false,
            frontmatter: false,
            markdown: false,
            separate_markdown: false,
        },
    )
    .expect("debbugs extraction should succeed");

    assert_eq!(output.extractor_type.as_deref(), Some("debbugs"));
    assert!(output
        .org
        .contains("** Message #5: [PATCH] Editable grep buffers"));
    assert!(output
        .org
        .contains(":FROM: Visuwesh <visuweshm <at> gmail.com>"));
    assert!(output.org.contains("#+begin_example\nTags: patch"));
    assert!(output.org.contains(
        "- Attachment :: [[https://debbugs.gnu.org/cgi/bugreport.cgi?att=1;msg=5;filename=grep-edit.diff][grep-edit.diff]] (text/patch, attachment)"
    ));
    assert!(output
        .org
        .contains("** Message #8: Re: bug#70820: [PATCH] Editable grep buffers"));
    assert!(output
        .org
        .contains("How does this compare to the existing wgrep package?"));
    assert!(!output.org.contains("Information forwarded"));
    assert!(!output.org.contains("#+begin_src"));
}

#[test]
fn org_manual_literal_examples_escape_org_syntax_in_code_blocks() {
    let html = r##"<!doctype html>
<html>
<head><title>Literal Examples (The Org Manual)</title></head>
<body>
<main>
<h1>Literal Examples</h1>
<p>Org examples can contain Org syntax literally.</p>
<pre class="example">#+BEGIN_EXAMPLE
Some example from the Org manual.
#+END_EXAMPLE</pre>
<pre class="example">#+BEGIN_SRC emacs-lisp
(defun org-xor (a b)
  "Exclusive or.")
,#+END_SRC</pre>
<pre class="example">  * This remains example text, not a headline.
  ,* Already escaped heading syntax.
  #+begin_quote
  ,#+already escaped keyword syntax.</pre>
</main>
</body>
</html>"##;

    let output = parse_html_to_org(
        html,
        DefuddleOptions {
            url: Some("https://orgmode.org/manual/Literal-Examples.html".to_string()),
            ..DefuddleOptions::default()
        },
    )
    .expect("Org Manual literal examples should parse");

    assert!(output.org.contains(",#+BEGIN_EXAMPLE"));
    assert!(output.org.contains(",#+END_EXAMPLE"));
    assert!(output.org.contains(",#+BEGIN_SRC emacs-lisp"));
    assert!(output.org.contains(",,#+END_SRC"));
    assert!(output
        .org
        .contains(",* This remains example text, not a headline."));
    assert!(output.org.contains(",,* Already escaped heading syntax."));
    assert!(output.org.contains(",#+begin_quote"));
    assert!(output.org.contains(",,#+already escaped keyword syntax."));
}

fn first_org_line_difference(expected: &str, actual: &str) -> String {
    let expected = expected.lines().collect::<Vec<_>>();
    let actual = actual.lines().collect::<Vec<_>>();
    let line = expected
        .iter()
        .zip(&actual)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| expected.len().min(actual.len()));
    let start = line.saturating_sub(2);
    let end = (line + 3).max(start).min(expected.len().max(actual.len()));
    let expected = (start..end)
        .map(|index| format!("{:>5}: {}", index + 1, expected.get(index).unwrap_or(&"")))
        .collect::<Vec<_>>()
        .join("\n");
    let actual = (start..end)
        .map(|index| format!("{:>5}: {}", index + 1, actual.get(index).unwrap_or(&"")))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "first differing line: {}\nexpected context:\n{}\nactual context:\n{}",
        line + 1,
        expected,
        actual
    )
}

fn upstream_references() -> (PathBuf, Vec<UpstreamReference>) {
    let defuddle_dir =
        upstream_dir().expect("set ORG_DEFUDDLE_DEFUDDLE_DIR to the pinned defuddle checkout");
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/defuddle-reference.mjs");
    let result = Command::new("node")
        .arg(&script)
        .arg(&defuddle_dir)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {}: {err}", script.display()));
    assert!(
        result.status.success(),
        "upstream reference runner failed:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let references = serde_json::from_slice(&result.stdout)
        .unwrap_or_else(|err| panic!("failed to parse upstream reference output: {err}"));
    (defuddle_dir, references)
}

fn upstream_fixture_names(defuddle_dir: &std::path::Path) -> Vec<String> {
    snapshot_names(&defuddle_dir.join("tests").join("fixtures"), "html")
}

fn expected_org_snapshot_names(expected_dir: &std::path::Path) -> Vec<String> {
    snapshot_names(expected_dir, "org")
}

fn snapshot_names(directory: &std::path::Path, extension: &str) -> Vec<String> {
    let mut names = std::fs::read_dir(directory)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", directory.display()))
        .map(|entry| entry.unwrap_or_else(|err| panic!("failed to read directory entry: {err}")))
        .filter_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|value| value.to_str()) == Some(extension))
                .then(|| path.file_stem()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn expected_org_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/expected-org")
}

fn parse_upstream_fixture(
    defuddle_dir: &std::path::Path,
    fixture: &str,
) -> org_defuddle_core::DefuddleOutput {
    let html_path = defuddle_dir
        .join("tests")
        .join("fixtures")
        .join(format!("{fixture}.html"));
    let html = std::fs::read_to_string(&html_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", html_path.display()));
    let url = fixture_url(&html).or_else(|| {
        Some(format!(
            "https://{}",
            Regex::new(r"^[a-z]+--").unwrap().replace(fixture, "")
        ))
    });
    parse_html_to_org(
        &html,
        DefuddleOptions {
            url,
            ..DefuddleOptions::default()
        },
    )
    .unwrap_or_else(|err| panic!("failed to parse {fixture}: {err}"))
}

fn read_expected_metadata(path: &std::path::Path) -> ExpectedMetadata {
    let markdown = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let json = markdown
        .strip_prefix("```json\n")
        .and_then(|rest| rest.split_once("\n```\n"))
        .map(|(json, _)| json)
        .unwrap_or_else(|| {
            panic!(
                "{} does not start with a JSON metadata block",
                path.display()
            )
        });
    serde_json::from_str(json)
        .unwrap_or_else(|err| panic!("failed to parse metadata in {}: {err}", path.display()))
}

fn fixture_url(html: &str) -> Option<String> {
    Regex::new(r#"<!--\s*\{\s*"url"\s*:\s*"([^"]+)""#)
        .ok()?
        .captures(html)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

#[test]
fn youtube_uses_default_audio_and_rejects_stale_inline_player_data() {
    use org_defuddle_core::{youtube_caption_info, youtube_inline_caption_info};
    let tracks = serde_json::json!([
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=en", "languageCode":"en", "kind":"asr"},
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=ja", "languageCode":"ja", "kind":"asr"},
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=ja-manual", "languageCode":"ja"}
    ]);
    let mut player = serde_json::json!({"videoDetails":{"videoId":"correct"},"captions":{"playerCaptionsTracklistRenderer":{
        "captionTracks": tracks,
        "defaultAudioTrackIndex":1,
        "audioTracks":[{"audioTrackId":"en.1","captionTrackIndices":[0]}, {"audioTrackId":"ja.4","captionTrackIndices":[1,2]}]
    }}});
    let info = youtube_caption_info(&player.to_string(), None)
        .unwrap()
        .unwrap();
    assert_eq!(info.language, "ja");
    assert!(info.caption_url.ends_with("ja-manual"));
    assert_eq!(
        youtube_caption_info(&player.to_string(), Some("en"))
            .unwrap()
            .unwrap()
            .language,
        "en"
    );
    player["captions"]["playerCaptionsTracklistRenderer"]["audioTracks"][1]
        ["defaultCaptionTrackIndex"] = serde_json::json!(1);
    assert!(youtube_caption_info(&player.to_string(), None)
        .unwrap()
        .unwrap()
        .caption_url
        .ends_with("lang=ja"));
    let html = format!("<script>var ytInitialPlayerResponse = {};</script>", player);
    assert!(youtube_inline_caption_info(
        &html,
        Some("https://www.youtube.com/watch?v=other"),
        None
    )
    .unwrap()
    .is_none());
    assert!(youtube_inline_caption_info(
        &html,
        Some("https://www.youtube.com/watch?v=correct"),
        None
    )
    .unwrap()
    .is_some());
}

#[test]
fn youtube_prefers_original_language_and_regional_english() {
    use org_defuddle_core::youtube_caption_info;
    let player = serde_json::json!({"captions":{"playerCaptionsTracklistRenderer":{"captionTracks":[
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=af", "languageCode":"af"},
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=fr", "languageCode":"fr", "kind":"asr"},
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=fr-manual", "languageCode":"fr"}
    ]}}});
    assert!(youtube_caption_info(&player.to_string(), None)
        .unwrap()
        .unwrap()
        .caption_url
        .ends_with("fr-manual"));
    let player = serde_json::json!({"captions":{"playerCaptionsTracklistRenderer":{"captionTracks":[
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=af", "languageCode":"af"},
        {"baseUrl":"https://www.youtube.com/api/timedtext?lang=en-US", "languageCode":"en-US"}
    ], "defaultCaptionTrackIndex":999}}});
    assert_eq!(
        youtube_caption_info(&player.to_string(), None)
            .unwrap()
            .unwrap()
            .language,
        "en-us"
    );
}

#[test]
fn youtube_matches_manual_language_families_and_preserves_caption_indices() {
    use org_defuddle_core::youtube_caption_info;
    let mut player = serde_json::json!({"captions":{"playerCaptionsTracklistRenderer":{
        "captionTracks":[
            {"baseUrl":"https://unrelated.example/captions", "languageCode":"af"},
            {"baseUrl":"https://www.youtube.com/api/timedtext?lang=en", "languageCode":"en", "kind":"asr"},
            {"baseUrl":"https://www.youtube.com/api/timedtext?lang=en-GB", "languageCode":"en-GB"}
        ],
        "audioTracks":[{"audioTrackId":"en.4", "captionTrackIndices":[0,1]}]
    }}});
    let selected = |player: &serde_json::Value| {
        youtube_caption_info(&player.to_string(), None)
            .unwrap()
            .unwrap()
    };
    assert!(selected(&player).caption_url.ends_with("en-GB"));
    player["captions"]["playerCaptionsTracklistRenderer"]["audioTracks"][0]
        ["defaultCaptionTrackIndex"] = serde_json::json!(1);
    assert!(selected(&player).caption_url.ends_with("lang=en"));
    player["captions"]["playerCaptionsTracklistRenderer"]["audioTracks"][0]
        ["defaultCaptionTrackIndex"] = serde_json::json!(0);
    assert!(selected(&player).caption_url.ends_with("en-GB"));
    player["captions"]["playerCaptionsTracklistRenderer"]["audioTracks"][0]["audioTrackId"] =
        serde_json::json!("unknown.4");
    assert!(selected(&player).caption_url.ends_with("en-GB"));
}

#[test]
fn ordinary_fragment_links_do_not_turn_list_items_into_footnotes() {
    let output = parse_html_to_org(
        r##"<article><h1>Internal navigation</h1>
        <p>The article contains a normal link to <a href="#chapter-one">the first chapter</a>.</p>
        <ol><li id="chapter-one">Chapter one describes the main argument and remains part of the article.</li>
        <li>Chapter two continues the discussion with further examples.</li></ol></article>"##,
        DefuddleOptions { content_selector: Some("article".to_string()), remove_content_patterns: false, remove_low_scoring: false, ..DefuddleOptions::default() },
    ).unwrap();
    assert!(output.org.contains("[[#chapter-one][the first chapter]]"));
    assert!(output
        .org
        .contains("Chapter one describes the main argument"));
    assert!(!output.org.contains("[fn:"));
}

#[test]
fn org_images_use_responsive_sources_instead_of_inline_placeholders() {
    let org = render_html_fragment_to_org(
        r#"<p><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" srcset="https://images.example.com/photo-600.jpg 600w, https://images.example.com/photo-1200.jpg 1200w" alt="Photo"></p>"#,
    );
    assert_eq!(
        org.trim(),
        "[[https://images.example.com/photo-1200.jpg][Photo]]"
    );
}

fn first_semantic_difference(expected: &str, actual: &str) -> String {
    let position = expected
        .chars()
        .zip(actual.chars())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| expected.chars().count().min(actual.chars().count()));
    let context = |text: &str| {
        text.chars()
            .skip(position.saturating_sub(60))
            .take(240)
            .collect::<String>()
    };
    format!(
        "character {position}:\nexpected {}\nactual {}",
        context(expected),
        context(actual)
    )
}

fn markdown_semantics(markdown: &str) -> String {
    static BEFORE_PUNCTUATION: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r" +([,.!?:;])").unwrap());
    static ASCII_SPACE: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"[ \t\r\n]+").unwrap());
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    // SVG is raw HTML, including currency labels such as "$0". Protect only
    // HTML parser events, so Markdown code containing SVG stays literal code.
    let mut protected = String::new();
    let mut copied = 0;
    let mut svg_start = None;
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        if let Event::Html(raw) | Event::InlineHtml(raw) = event {
            if raw.trim_start().starts_with("<svg") {
                svg_start = Some(range.start);
            }
            if raw.contains("</svg>") {
                if let Some(start) = svg_start.take() {
                    protected.push_str(&markdown[copied..start]);
                    protected.push_str(&markdown[start..range.end].replace('$', "&#36;"));
                    copied = range.end;
                }
            }
        }
    }
    protected.push_str(&markdown[copied..]);
    let events = Parser::new_ext(&protected, options).map(|event| match event {
        Event::InlineMath(text) => Event::InlineMath(math_semantics(&text).into()),
        Event::DisplayMath(text) => Event::DisplayMath(math_semantics(&text).into()),
        other => other,
    });
    let mut rendered = String::new();
    html::push_html(&mut rendered, events);
    let document = kuchiki::parse_html().one(rendered);
    // Display math visually divides prose even when one renderer keeps it
    // inside a paragraph and another emits surrounding blank lines.
    let display_math = |node: &kuchiki::NodeRef| {
        node.as_element().is_some_and(|element| {
            element
                .attributes
                .borrow()
                .get("class")
                .is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "math-display")
                })
        })
    };
    for paragraph in document
        .select("p")
        .unwrap()
        .map(|p| p.as_node().clone())
        .collect::<Vec<_>>()
    {
        if !paragraph.children().any(|child| display_math(&child)) {
            continue;
        }
        let mut chunk = paragraph.clone();
        let children = paragraph.children().collect::<Vec<_>>();
        for child in children {
            if display_math(&child) {
                chunk = paragraph.clone();
                paragraph.insert_before(child);
            } else {
                if chunk == paragraph {
                    let fragment = kuchiki::parse_html().one("<p></p>");
                    chunk = fragment.select_first("p").unwrap().as_node().clone();
                    paragraph.insert_before(chunk.clone());
                }
                chunk.append(child);
            }
        }
        paragraph.detach();
    }
    for reference in document.select("sup.footnote-reference").unwrap() {
        if let Some(previous) = reference.as_node().previous_sibling() {
            if let Some(text) = previous.as_text() {
                let trimmed = text.borrow().trim_end_matches(' ').to_string();
                *text.borrow_mut() = trimmed;
            }
        }
    }
    let block = |node: &kuchiki::NodeRef| {
        display_math(node)
            || node.as_element().is_some_and(|element| {
                matches!(
                    element.name.local.as_ref(),
                    "body"
                        | "p"
                        | "div"
                        | "blockquote"
                        | "pre"
                        | "ul"
                        | "ol"
                        | "li"
                        | "table"
                        | "thead"
                        | "tbody"
                        | "tr"
                        | "td"
                        | "th"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "hr"
                )
            })
    };
    for node in document.descendants().collect::<Vec<_>>() {
        let Some(text) = node.as_text() else {
            continue;
        };
        if node.ancestors().any(|parent| {
            parent.as_element().is_some_and(|element| {
                matches!(
                    element.name.local.as_ref(),
                    "pre" | "code" | "text" | "tspan"
                ) || element
                    .attributes
                    .borrow()
                    .get("class")
                    .is_some_and(|class| {
                        class.split_ascii_whitespace().any(|token| token == "math")
                    })
            })
        }) {
            continue;
        }
        let mut value = ASCII_SPACE.replace_all(&text.borrow(), " ").into_owned();
        value = BEFORE_PUNCTUATION.replace_all(&value, "$1").into_owned();
        let left = node.previous_sibling();
        let right = node.next_sibling();
        if left.as_ref().map_or(true, &block) {
            value = value.trim_start_matches(' ').to_string();
        }
        if right.as_ref().map_or(true, &block) {
            value = value.trim_end_matches(' ').to_string();
        }
        if value.is_empty() {
            node.detach();
        } else {
            *text.borrow_mut() = value;
        }
    }
    let body = document.select_first("body").unwrap();
    let mut bytes = Vec::new();
    body.as_node().serialize(&mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn math_semantics(text: &str) -> String {
    static TOKEN: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\\[a-zA-Z]+|\\.|[^\s]").unwrap());
    static LITERAL: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"^\\(?:text[a-z]*|mbox|hbox|vbox|url|href|operatorname|tag|label|ref|eqref)$")
            .unwrap()
    });
    let mut tokens = Vec::new();
    let mut consumed = 0;
    for token in TOKEN.find_iter(text) {
        if token.start() < consumed {
            continue;
        }
        let rest = &text[token.end()..];
        let argument = rest.trim_start();
        if LITERAL.is_match(token.as_str()) && argument.starts_with('{') {
            let mut depth = 0;
            let mut escaped = false;
            for (index, ch) in argument.char_indices() {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    consumed = token.end() + rest.len() - argument.len() + index + 1;
                    break;
                }
            }
        }
        tokens.push(&text[token.start()..consumed.max(token.end())]);
    }
    serde_json::to_string(&tokens).unwrap()
}

#[test]
fn markdown_comparison_keeps_content_code_math_footnotes_and_breaks() {
    assert_eq!(
        markdown_semantics("<svg width=\"1\" height=\"2\"></svg>"),
        markdown_semantics("<svg height=\"2\" width=\"1\"></svg>")
    );
    for (left, right) in [
        ("First paragraph.", "Different paragraph."),
        ("one two", "onetwo"),
        ("one  \ntwo", "one two"),
        ("```\na  b\n```", "```\na b\n```"),
        ("$x+y$", "$x-y$"),
        (r"$\text{a b}$", r"$\text{ab}$"),
        (r"$\sin x$", r"$\sinx$"),
        ("```rust\nx\n```", "```python\nx\n```"),
        ("<svg><text>$0</text></svg>", "<svg><text>$1</text></svg>"),
        ("$x$", "$$\nx\n$$"),
        (
            "Text[^1].\n\n[^1]: Original note.",
            "Text[^1].\n\n[^1]: Changed note.",
        ),
        ("Text[^1].\n\n[^1]: Original note.", "Text[^1]."),
    ] {
        assert_ne!(
            markdown_semantics(left),
            markdown_semantics(right),
            "{left:?} vs {right:?}"
        );
    }
}
