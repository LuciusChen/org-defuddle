use emacs::{defun, Env, Result, Value};
use org_defuddle_core::{
    bilibili_subtitle_info, bilibili_video_info, output_json_string, output_json_string_for_html,
    output_property, parse_bilibili_api_to_org, parse_c2_wiki_json_to_org,
    parse_fxtwitter_json_to_org, parse_html_to_org, parse_x_oembed_json_to_org,
    parse_youtube_api_to_org, youtube_caption_info, youtube_inline_caption_info, DefuddleOptions,
    DefuddleOutput, IncludeReplies,
};

emacs::plugin_is_GPL_compatible!();

#[emacs::module(name = "org-defuddle-module")]
fn init(_: &Env) -> Result<()> {
    Ok(())
}

#[defun(name = "parse-json")]
fn parse_json(html: String, url: String) -> Result<String> {
    let output = parse_output(
        &html,
        url,
        true,
        true,
        String::new(),
        IncludeReplies::Extractors,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
    )?;
    output_json_string_for_html(&output, &html).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-json-with-options")]
fn parse_json_with_options(
    html: String,
    url: String,
    include_images: Value<'_>,
    remove_small_images: Value<'_>,
    content_selector: String,
    include_replies: String,
    remove_hidden_elements: Value<'_>,
    remove_exact_selectors: Value<'_>,
    remove_partial_selectors: Value<'_>,
    remove_content_patterns: Value<'_>,
    remove_low_scoring: Value<'_>,
    standardize: Value<'_>,
    debug: Value<'_>,
    profile: Value<'_>,
    frontmatter: Value<'_>,
    markdown: Value<'_>,
    separate_markdown: Value<'_>,
) -> Result<String> {
    let output = parse_output(
        &html,
        url,
        include_images.is_not_nil(),
        remove_small_images.is_not_nil(),
        content_selector,
        parse_include_replies(&include_replies)?,
        remove_hidden_elements.is_not_nil(),
        remove_exact_selectors.is_not_nil(),
        remove_partial_selectors.is_not_nil(),
        remove_content_patterns.is_not_nil(),
        remove_low_scoring.is_not_nil(),
        standardize.is_not_nil(),
        debug.is_not_nil(),
        profile.is_not_nil(),
        frontmatter.is_not_nil(),
        markdown.is_not_nil(),
        separate_markdown.is_not_nil(),
    )?;
    output_json_string_for_html(&output, &html).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-org")]
fn parse_org(html: String, url: String) -> Result<String> {
    let output = parse_output(
        &html,
        url,
        true,
        true,
        String::new(),
        IncludeReplies::Extractors,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        false,
    )?;
    Ok(output.org)
}

#[defun(name = "parse-property")]
fn parse_property(html: String, url: String, property: String) -> Result<String> {
    let output = parse_output(
        &html,
        url,
        true,
        true,
        String::new(),
        IncludeReplies::Extractors,
        true,
        true,
        true,
        true,
        true,
        true,
        false,
        false,
        false,
        false,
        property_requests_markdown(&property),
    )?;
    property_value(&output, &property)
}

#[defun(name = "parse-org-with-options")]
fn parse_org_with_options(
    html: String,
    url: String,
    include_images: Value<'_>,
    remove_small_images: Value<'_>,
    content_selector: String,
    include_replies: String,
    remove_hidden_elements: Value<'_>,
    remove_exact_selectors: Value<'_>,
    remove_partial_selectors: Value<'_>,
    remove_content_patterns: Value<'_>,
    remove_low_scoring: Value<'_>,
    standardize: Value<'_>,
    debug: Value<'_>,
    profile: Value<'_>,
    frontmatter: Value<'_>,
    markdown: Value<'_>,
    separate_markdown: Value<'_>,
) -> Result<String> {
    let output = parse_output(
        &html,
        url,
        include_images.is_not_nil(),
        remove_small_images.is_not_nil(),
        content_selector,
        parse_include_replies(&include_replies)?,
        remove_hidden_elements.is_not_nil(),
        remove_exact_selectors.is_not_nil(),
        remove_partial_selectors.is_not_nil(),
        remove_content_patterns.is_not_nil(),
        remove_low_scoring.is_not_nil(),
        standardize.is_not_nil(),
        debug.is_not_nil(),
        profile.is_not_nil(),
        frontmatter.is_not_nil(),
        markdown.is_not_nil(),
        separate_markdown.is_not_nil(),
    )?;
    Ok(output.org)
}

#[defun(name = "parse-property-with-options")]
fn parse_property_with_options(
    html: String,
    url: String,
    property: String,
    include_images: Value<'_>,
    remove_small_images: Value<'_>,
    content_selector: String,
    include_replies: String,
    remove_hidden_elements: Value<'_>,
    remove_exact_selectors: Value<'_>,
    remove_partial_selectors: Value<'_>,
    remove_content_patterns: Value<'_>,
    remove_low_scoring: Value<'_>,
    standardize: Value<'_>,
    debug: Value<'_>,
    profile: Value<'_>,
    frontmatter: Value<'_>,
    markdown: Value<'_>,
    separate_markdown: Value<'_>,
) -> Result<String> {
    let output = parse_output(
        &html,
        url,
        include_images.is_not_nil(),
        remove_small_images.is_not_nil(),
        content_selector,
        parse_include_replies(&include_replies)?,
        remove_hidden_elements.is_not_nil(),
        remove_exact_selectors.is_not_nil(),
        remove_partial_selectors.is_not_nil(),
        remove_content_patterns.is_not_nil(),
        remove_low_scoring.is_not_nil(),
        standardize.is_not_nil(),
        debug.is_not_nil(),
        profile.is_not_nil(),
        frontmatter.is_not_nil(),
        markdown.is_not_nil(),
        separate_markdown.is_not_nil() || property_requests_markdown(&property),
    )?;
    property_value(&output, &property)
}

fn property_value(output: &DefuddleOutput, property: &str) -> Result<String> {
    output_property(output, property)
        .ok_or_else(|| emacs::Error::msg(format!("Property \"{property}\" not found in response")))
}

fn property_requests_markdown(property: &str) -> bool {
    matches!(
        property,
        "contentMarkdown" | "content_markdown" | "markdown"
    )
}

#[defun(name = "parse-c2-json")]
fn parse_c2_json(json: String, url: String) -> Result<String> {
    let output = parse_c2_output(&json, url)?;
    output_json_string(&output).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-c2-org")]
fn parse_c2_org(json: String, url: String) -> Result<String> {
    let output = parse_c2_output(&json, url)?;
    Ok(output.org)
}

#[defun(name = "parse-x-oembed-json")]
fn parse_x_oembed_json(json: String, url: String) -> Result<String> {
    let output = parse_x_oembed_output(&json, url)?;
    output_json_string(&output).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-x-oembed-org")]
fn parse_x_oembed_org(json: String, url: String) -> Result<String> {
    let output = parse_x_oembed_output(&json, url)?;
    Ok(output.org)
}

#[defun(name = "parse-fxtwitter-json")]
fn parse_fxtwitter_json(json: String, url: String) -> Result<String> {
    let output = parse_fxtwitter_output(&json, url)?;
    output_json_string(&output).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-fxtwitter-org")]
fn parse_fxtwitter_org(json: String, url: String) -> Result<String> {
    let output = parse_fxtwitter_output(&json, url)?;
    Ok(output.org)
}

#[defun(name = "bilibili-video-info")]
fn bilibili_video_info_json(view_json: String, url: String) -> Result<String> {
    let info = bilibili_video_info(
        &view_json,
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))?;
    serde_json::to_string(&info).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "bilibili-subtitle-info")]
fn bilibili_subtitle_info_json(player_json: String, preferred_language: String) -> Result<String> {
    let info = bilibili_subtitle_info(
        &player_json,
        if preferred_language.trim().is_empty() {
            None
        } else {
            Some(preferred_language.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))?;
    serde_json::to_string(&info).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-bilibili-json")]
fn parse_bilibili_json(
    view_json: String,
    subtitle_json: String,
    url: String,
    language_code: String,
) -> Result<String> {
    let output = parse_bilibili_output(&view_json, &subtitle_json, url, language_code)?;
    output_json_string(&output).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-bilibili-org")]
fn parse_bilibili_org(
    view_json: String,
    subtitle_json: String,
    url: String,
    language_code: String,
) -> Result<String> {
    let output = parse_bilibili_output(&view_json, &subtitle_json, url, language_code)?;
    Ok(output.org)
}

#[defun(name = "youtube-caption-info")]
fn youtube_caption_info_json(player_json: String, preferred_language: String) -> Result<String> {
    let info = youtube_caption_info(
        &player_json,
        if preferred_language.trim().is_empty() {
            None
        } else {
            Some(preferred_language.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))?;
    serde_json::to_string(&info).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "youtube-inline-caption-info")]
fn youtube_inline_caption_info_json(
    html: String,
    url: String,
    preferred_language: String,
) -> Result<String> {
    let info = youtube_inline_caption_info(
        &html,
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
        if preferred_language.trim().is_empty() {
            None
        } else {
            Some(preferred_language.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))?;
    serde_json::to_string(&info).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-youtube-json")]
fn parse_youtube_json(
    player_json: String,
    caption_xml: String,
    chapters_json: String,
    url: String,
    language_code: String,
) -> Result<String> {
    let output = parse_youtube_output(
        &player_json,
        &caption_xml,
        &chapters_json,
        url,
        language_code,
    )?;
    output_json_string(&output).map_err(|err| emacs::Error::msg(err.to_string()))
}

#[defun(name = "parse-youtube-org")]
fn parse_youtube_org(
    player_json: String,
    caption_xml: String,
    chapters_json: String,
    url: String,
    language_code: String,
) -> Result<String> {
    let output = parse_youtube_output(
        &player_json,
        &caption_xml,
        &chapters_json,
        url,
        language_code,
    )?;
    Ok(output.org)
}

fn parse_output(
    html: &str,
    url: String,
    include_images: bool,
    remove_small_images: bool,
    content_selector: String,
    include_replies: IncludeReplies,
    remove_hidden_elements: bool,
    remove_exact_selectors: bool,
    remove_partial_selectors: bool,
    remove_content_patterns: bool,
    remove_low_scoring: bool,
    standardize: bool,
    debug: bool,
    profile: bool,
    frontmatter: bool,
    markdown: bool,
    separate_markdown: bool,
) -> Result<DefuddleOutput> {
    parse_html_to_org(
        html,
        DefuddleOptions {
            url: if url.trim().is_empty() {
                None
            } else {
                Some(url)
            },
            include_images,
            remove_small_images,
            content_selector: if content_selector.trim().is_empty() {
                None
            } else {
                Some(content_selector)
            },
            include_replies,
            remove_hidden_elements,
            remove_exact_selectors,
            remove_partial_selectors,
            remove_content_patterns,
            remove_low_scoring,
            standardize,
            debug,
            profile,
            frontmatter,
            markdown,
            separate_markdown,
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))
}

fn parse_c2_output(json: &str, url: String) -> Result<DefuddleOutput> {
    parse_c2_wiki_json_to_org(
        json,
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))
}

fn parse_x_oembed_output(json: &str, url: String) -> Result<DefuddleOutput> {
    parse_x_oembed_json_to_org(
        json,
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))
}

fn parse_fxtwitter_output(json: &str, url: String) -> Result<DefuddleOutput> {
    parse_fxtwitter_json_to_org(
        json,
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))
}

fn parse_bilibili_output(
    view_json: &str,
    subtitle_json: &str,
    url: String,
    language_code: String,
) -> Result<DefuddleOutput> {
    parse_bilibili_api_to_org(
        view_json,
        if subtitle_json.trim().is_empty() {
            None
        } else {
            Some(subtitle_json)
        },
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
        if language_code.trim().is_empty() {
            None
        } else {
            Some(language_code.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))
}

fn parse_youtube_output(
    player_json: &str,
    caption_xml: &str,
    chapters_json: &str,
    url: String,
    language_code: String,
) -> Result<DefuddleOutput> {
    parse_youtube_api_to_org(
        player_json,
        if caption_xml.trim().is_empty() {
            None
        } else {
            Some(caption_xml)
        },
        if chapters_json.trim().is_empty() {
            None
        } else {
            Some(chapters_json)
        },
        if url.trim().is_empty() {
            None
        } else {
            Some(url.as_str())
        },
        if language_code.trim().is_empty() {
            None
        } else {
            Some(language_code.as_str())
        },
    )
    .map_err(|err| emacs::Error::msg(err.to_string()))
}

fn parse_include_replies(value: &str) -> Result<IncludeReplies> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "extractors" => Ok(IncludeReplies::Extractors),
        "all" | "t" | "true" => Ok(IncludeReplies::All),
        "none" | "nil" | "false" => Ok(IncludeReplies::None),
        other => Err(emacs::Error::msg(format!(
            "invalid include replies value: {other}"
        ))),
    }
}
