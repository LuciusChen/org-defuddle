use super::FetchOptions;
use org_defuddle_core::{
    bilibili_subtitle_info, bilibili_video_info, parse_bilibili_api_to_org,
    parse_c2_wiki_json_to_org, parse_fxtwitter_json_to_org, parse_html_to_org,
    parse_x_oembed_json_to_org, parse_youtube_api_to_org, youtube_caption_info,
    youtube_inline_caption_info, ApiSource, DefuddleOptions, DefuddleOutput,
};
use serde_json::{json, Value};
use url::Url;

type Fetch<'a> = dyn FnMut(FetchOptions) -> Result<String, String> + 'a;

fn get(url: impl Into<String>) -> FetchOptions {
    FetchOptions {
        url: Some(url.into()),
        method: "GET".into(),
        ..Default::default()
    }
}

fn player_request(video_id: &str, client: &str, version: &str, endpoint: &str) -> FetchOptions {
    FetchOptions {
        url: Some(format!("https://www.youtube.com/youtubei/v1/{endpoint}?prettyPrint=false")),
        method: "POST".into(),
        headers: vec![("Content-Type".into(), "application/json".into())],
        data: Some(json!({"context":{"client":{"clientName":client,"clientVersion":version}},"videoId":video_id}).to_string()),
        ..Default::default()
    }
}

pub(super) fn extract(
    source: ApiSource,
    url: &str,
    language: Option<&str>,
    fetch: &mut Fetch<'_>,
) -> Result<(DefuddleOutput, String), String> {
    let output = match source {
        ApiSource::C2 { title } => {
            let mut endpoint = Url::parse("https://c2.com/wiki/remodel/pages/").unwrap();
            endpoint
                .path_segments_mut()
                .unwrap()
                .pop_if_empty()
                .push(&title);
            let json = fetch(get(endpoint.to_string()))?;
            parse_c2_wiki_json_to_org(&json, Some(url)).map_err(|err| err.to_string())?
        }
        ApiSource::X { user, id } => {
            let primary = fetch(get(format!("https://api.fxtwitter.com/{user}/status/{id}")))
                .and_then(|json| {
                    parse_fxtwitter_json_to_org(&json, Some(url)).map_err(|err| err.to_string())
                });
            match primary {
                Ok(output) => output,
                Err(primary_error) => {
                    let mut endpoint = Url::parse("https://publish.twitter.com/oembed").unwrap();
                    endpoint
                        .query_pairs_mut()
                        .append_pair("url", url)
                        .append_pair("omit_script", "true");
                    let json = fetch(get(endpoint.to_string())).map_err(|err| {
                        format!("X API failed: {primary_error}; oEmbed failed: {err}")
                    })?;
                    parse_x_oembed_json_to_org(&json, Some(url)).map_err(|err| {
                        format!("X API failed: {primary_error}; oEmbed failed: {err}")
                    })?
                }
            }
        }
        ApiSource::Bilibili { bvid } => bilibili(url, &bvid, language, fetch)?,
        ApiSource::Youtube { video_id } => return youtube(url, &video_id, language, fetch),
    };
    Ok((output, String::new()))
}

fn bilibili(
    url: &str,
    bvid: &str,
    language: Option<&str>,
    fetch: &mut Fetch<'_>,
) -> Result<DefuddleOutput, String> {
    let view = fetch(get(format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={bvid}"
    )))?;
    let info = bilibili_video_info(&view, Some(url)).map_err(|err| err.to_string())?;
    if info.aid > 0 && info.cid > 0 {
        let queries = [
            format!(
                "wbi/v2?bvid={}&aid={}&cid={}",
                info.bvid, info.aid, info.cid
            ),
            format!("v2?bvid={}&cid={}", info.bvid, info.cid),
            format!("v2?aid={}&cid={}", info.aid, info.cid),
        ];
        let mut received_player = false;
        let mut failures = Vec::new();
        for query in queries {
            let player = match fetch(get(format!("https://api.bilibili.com/x/player/{query}"))) {
                Ok(player) => player,
                Err(err) => {
                    failures.push(err);
                    continue;
                }
            };
            let track = bilibili_subtitle_info(&player, language).map_err(|err| err.to_string())?;
            received_player = true;
            if let Some(track) = track {
                let subtitle = fetch(get(track.subtitle_url))?;
                return parse_bilibili_api_to_org(
                    &view,
                    Some(&subtitle),
                    Some(url),
                    Some(&track.language),
                )
                .map_err(|err| err.to_string());
            }
        }
        if !received_player {
            return Err(format!(
                "Bilibili player requests failed: {}",
                failures.join("; ")
            ));
        }
    }
    parse_bilibili_api_to_org(&view, None, Some(url), None).map_err(|err| err.to_string())
}

fn transcript(
    player: &str,
    caption_url: String,
    selected_language: &str,
    url: &str,
    video_id: &str,
    fetch: &mut Fetch<'_>,
) -> Result<DefuddleOutput, String> {
    let caption = fetch(get(caption_url))?;
    // Chapters are optional; their absence must not discard a valid transcript.
    let chapters = fetch(player_request(video_id, "WEB", "2.20240101.00.00", "next")).ok();
    let chapters = chapters
        .as_deref()
        .filter(|json| serde_json::from_str::<Value>(json).is_ok());
    parse_youtube_api_to_org(
        player,
        Some(&caption),
        chapters,
        Some(url),
        Some(selected_language),
    )
    .map_err(|err| err.to_string())
}

fn has_transcript(output: &DefuddleOutput) -> bool {
    output
        .variables
        .as_ref()
        .and_then(|vars| vars.get("transcript"))
        .is_some_and(|text| !text.is_empty())
}

fn youtube(
    url: &str,
    video_id: &str,
    language: Option<&str>,
    fetch: &mut Fetch<'_>,
) -> Result<(DefuddleOutput, String), String> {
    let mut failures = Vec::new();
    for (client, version) in [
        ("IOS", "20.10.3"),
        ("ANDROID", "20.10.38"),
        ("WEB", "2.20240101.00.00"),
    ] {
        let mut request = player_request(video_id, client, version, "player");
        if client == "ANDROID" {
            request.user_agent =
                Some("com.google.android.youtube/20.10.38 (Linux; U; Android 14)".into());
        }
        let player = match fetch(request) {
            Ok(player) => player,
            Err(err) => {
                failures.push(format!("{client}: {err}"));
                continue;
            }
        };
        let track = match youtube_caption_info(&player, language) {
            Ok(track) => track,
            Err(err) => {
                failures.push(format!("{client}: {err}"));
                continue;
            }
        };
        if let Some(track) = track {
            match transcript(
                &player,
                track.caption_url,
                &track.language,
                url,
                video_id,
                fetch,
            ) {
                Ok(output) if has_transcript(&output) => return Ok((output, String::new())),
                Ok(_) => failures.push(format!("{client}: empty transcript")),
                Err(err) => failures.push(format!("{client}: {err}")),
            }
        }
    }
    // The page can expose a working caption track even when Innertube clients do not.
    let html = fetch(get(url))
        .map_err(|err| format!("YouTube page failed: {err}; {}", failures.join("; ")))?;
    if let Some(track) =
        youtube_inline_caption_info(&html, Some(url), language).map_err(|err| err.to_string())?
    {
        let output = transcript(
            &track.player_json,
            track.caption_url,
            &track.language,
            url,
            video_id,
            fetch,
        )?;
        if has_transcript(&output) {
            return Ok((output, html));
        }
    }
    let output = parse_html_to_org(
        &html,
        DefuddleOptions {
            url: Some(url.into()),
            ..Default::default()
        },
    )
    .map_err(|err| err.to_string())?;
    Ok((output, html))
}

#[cfg(test)]
mod tests {
    use super::*;
    use org_defuddle_core::api_source;

    fn extract_url(
        url: &str,
        language: Option<&str>,
        fetch: &mut Fetch<'_>,
    ) -> Result<DefuddleOutput, String> {
        extract(api_source(url).unwrap(), url, language, fetch).map(|(output, _)| output)
    }

    fn player() -> String {
        json!({"videoDetails":{"videoId":"test123","title":"CLI video","author":"Video author","shortDescription":"Video description."},
            "captions":{"playerCaptionsTracklistRenderer":{"captionTracks":[{"languageCode":"en","baseUrl":"https://www.youtube.com/api/timedtext?v=test123&lang=en"}]}}}).to_string()
    }

    #[test]
    fn c2_requests_the_validated_page_and_keeps_metadata() {
        let mut output = extract_url("https://wiki.c2.com/?SmallTalk", None, &mut |request| {
            assert_eq!(
                request.url.as_deref(),
                Some("https://c2.com/wiki/remodel/pages/SmallTalk")
            );
            assert_eq!(request.method, "GET");
            Ok(
                json!({"text":"A useful wiki page about programming.","date":"November 20, 2026"})
                    .to_string(),
            )
        })
        .unwrap();
        assert_eq!(output.title, "Small Talk");
        assert!(output.org.contains("A useful wiki page about programming."));
        let options = DefuddleOptions {
            url: Some("https://wiki.c2.com/?SmallTalk".into()),
            separate_markdown: true,
            frontmatter: true,
            ..Default::default()
        };
        org_defuddle_core::apply_output_options(&mut output, &options);
        assert!(output
            .content_markdown
            .contains("A useful wiki page about programming."));
        assert!(output.frontmatter.contains(r#"title: "Small Talk""#));
        let org = output.org.clone();
        org_defuddle_core::apply_output_options(&mut output, &options);
        assert_eq!(output.org, org);
    }

    #[test]
    fn x_uses_oembed_after_a_failed_primary_request_and_reports_both_errors() {
        let url = "https://x.com/alice/status/12345";
        let mut calls = 0;
        let output = extract_url(url, None, &mut |request| {
            calls += 1;
            if calls == 1 {
                assert_eq!(
                    request.url.as_deref(),
                    Some("https://api.fxtwitter.com/alice/status/12345")
                );
                return Err("primary blocked".into());
            }
            let endpoint = Url::parse(request.url.as_deref().unwrap()).unwrap();
            assert_eq!(endpoint.host_str(), Some("publish.twitter.com"));
            assert!(endpoint
                .query_pairs()
                .any(|(key, value)| key == "url" && value == url));
            Ok(json!({
                "author_name": "Alice", "author_url": "https://twitter.com/alice",
                "html": "<blockquote><p>CLI fallback post.</p></blockquote>"
            })
            .to_string())
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert!(output.org.contains("CLI fallback post."));
        let error = extract_url(url, None, &mut |_| Err("unavailable".into())).unwrap_err();
        assert!(error.contains("X API failed") && error.contains("oEmbed failed"));
    }

    #[test]
    fn bilibili_selects_the_requested_part_and_subtitle_language() {
        let mut calls = Vec::new();
        let output = extract_url(
            "https://www.bilibili.com/video/BVtest123?p=2", Some("zh-cn"), &mut |request| {
                let endpoint = request.url.unwrap();
                calls.push(endpoint.clone());
                if endpoint.contains("web-interface/view") {
                    return Ok(json!({"code": 0, "data": {
                        "bvid": "BVtest123", "aid": 42, "cid": 11,
                        "title": "CLI Bilibili", "desc": "Video description.", "owner": {"name": "Uploader"},
                        "pages": [{"page": 1, "cid": 11, "part": "One"}, {"page": 2, "cid": 22, "part": "Two"}]
                    }}).to_string());
                }
                if endpoint.contains("/x/player/") {
                    assert!(endpoint.contains("cid=22"));
                    if calls.len() < 4 { return Ok(json!({"code": 0, "data": {}}).to_string()); }
                    return Ok(json!({"code": 0, "data": {"subtitle": {"subtitles": [
                        {"lan": "en", "subtitle_url": "https://aisubtitle.hdslb.com/en.json"},
                        {"lan": "zh-CN", "subtitle_url": "https://aisubtitle.hdslb.com/zh.json"}
                    ]}}}).to_string());
                }
                assert_eq!(endpoint, "https://aisubtitle.hdslb.com/zh.json");
                Ok(json!({"body": [{"from": 0, "to": 6, "content": "字幕正文。"}]}).to_string())
            }
        ).unwrap();
        assert_eq!(calls.len(), 5);
        assert!(calls[1].contains("/wbi/v2?bvid="));
        assert!(calls[2].contains("/v2?bvid="));
        assert!(calls[3].contains("/v2?aid="));
        assert_eq!(output.title, "CLI Bilibili");
        assert!(output.org.contains("字幕正文。"));
    }

    #[test]
    fn youtube_retries_empty_captions_with_the_next_client() {
        let mut players = 0;
        let mut captions = 0;
        let output = extract_url("https://www.youtube.com/watch?v=test123", Some("en"), &mut |request| {
            let endpoint = request.url.unwrap();
            if endpoint.contains("/player?") {
                players += 1;
                let body: Value = serde_json::from_str(request.data.as_deref().unwrap()).unwrap();
                assert_eq!(request.method, "POST");
                assert_eq!(body["videoId"], "test123");
                assert_eq!(body["context"]["client"]["clientName"], if players == 1 { "IOS" } else { "ANDROID" });
                return Ok(player());
            }
            if endpoint.contains("timedtext") {
                captions += 1;
                return Ok(if captions == 1 {
                    "<transcript/>"
                } else {
                    "<transcript><text start=\"0\" dur=\"6\">CLI transcript is preserved.</text></transcript>"
                }.into());
            }
            assert!(endpoint.contains("/next?"));
            Ok("{}".into())
        }).unwrap();
        assert_eq!((players, captions), (2, 2));
        assert_eq!(output.title, "CLI video");
        assert!(output.org.contains("CLI transcript is preserved."));
        assert!(has_transcript(&output));
    }

    #[test]
    fn youtube_uses_valid_inline_tracks_after_innertube_failure() {
        let mut calls = Vec::new();
        let output = extract_url("https://youtu.be/test123", None, &mut |request| {
            let endpoint = request.url.unwrap();
            calls.push(endpoint.clone());
            if endpoint.contains("/player?") { return Err("client unavailable".into()); }
            if endpoint == "https://youtu.be/test123" {
                return Ok(format!("<script>ytInitialPlayerResponse={};</script>", player()));
            }
            if endpoint.contains("timedtext") {
                return Ok("<transcript><text start=\"0\" dur=\"5\">Inline transcript.</text></transcript>".into());
            }
            Ok("{}".into())
        }).unwrap();
        assert_eq!(
            calls.iter().filter(|url| url.contains("/player?")).count(),
            3
        );
        assert!(has_transcript(&output));
        assert!(output.org.contains("Inline transcript."));
        let mut failures = 0;
        let error = extract_url("https://youtu.be/test123", None, &mut |_| {
            failures += 1;
            Err("unavailable".into())
        })
        .unwrap_err();
        assert_eq!(failures, 4);
        assert!(
            error.contains("YouTube page failed")
                && error.contains("IOS:")
                && error.contains("WEB:")
        );
    }

    #[test]
    fn unsupported_and_lookalike_hosts_do_not_trigger_api_requests() {
        for url in [
            "https://youtube.com.evil.test/watch?v=test123",
            "https://bilibili.com.evil.test/video/BVtest123",
            "ftp://wiki.c2.com/?SmallTalk",
            "https://example.com/?v=test123",
        ] {
            assert!(api_source(url).is_none(), "{url}");
        }
        assert!(
            matches!(api_source("https://www.youtube.com/live/test123"),Some(ApiSource::Youtube { video_id }) if video_id=="test123")
        );
    }
}
