use org_defuddle_core::{parse_reddit_atom_to_org, reddit_sources, IncludeReplies};

const FEED: &str = include_str!("../../../tests/fixtures/reddit.atom");
const URL: &str = "https://www.reddit.com/r/emacs/comments/example/topic/";

#[test]
fn atom_fallback_preserves_post_comments_and_decodes_html_once() {
    let output = parse_reddit_atom_to_org(FEED, URL, true, IncludeReplies::Extractors).unwrap();
    assert_eq!(output.extractor_type.as_deref(), Some("reddit"));
    assert_eq!(output.title, "A & B discussion");
    assert_eq!(output.author, "alice");
    assert_eq!(output.site, "r/emacs");
    assert!(output.org.contains("Post with & entities"));
    assert!(output
        .org
        .contains("[[https://www.reddit.com/r/emacs/wiki][wiki]]"));
    assert!(output.org.contains("Helpful reply from Bob."));
    assert!(output.org.contains("2026-10-02"));
    assert!(output.org.contains("https://example.com/photo.jpg"));
    assert!(!output.html.contains("<script"));
    assert!(!output.html.contains("javascript:"));
    let post_only = parse_reddit_atom_to_org(FEED, URL, false, IncludeReplies::None).unwrap();
    assert!(!post_only.org.contains("Helpful reply"));
    assert!(!post_only.html.contains("<img"));
    assert!(post_only.org.contains("Post with & entities"));
}

#[test]
fn atom_fallback_rejects_invalid_or_unrelated_responses() {
    for body in [
        "<html>Login required</html>",
        "<feed>",
        "<feed><entry><id>t1_reply</id></entry></feed>",
    ] {
        assert!(parse_reddit_atom_to_org(body, URL, true, IncludeReplies::Extractors).is_err());
    }
}

#[test]
fn reddit_sources_require_comments_urls_and_preserve_query() {
    let sources = reddit_sources(&format!("{URL}?sort=top#reply")).unwrap();
    assert_eq!(
        sources.old_url,
        "https://old.reddit.com/r/emacs/comments/example/topic/?sort=top"
    );
    assert_eq!(
        sources.feed_url,
        "https://www.reddit.com/r/emacs/comments/example/topic/.rss?sort=top"
    );
    assert!(reddit_sources(&sources.old_url).is_some());
    for url in [
        "https://reddit.com.evil.test/r/emacs/comments/example/",
        "https://reddit.com/r/emacs/",
        "ftp://reddit.com/r/emacs/comments/example/",
    ] {
        assert!(reddit_sources(url).is_none());
    }
}
