use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use org_defuddle_core::{
    output_frontmatter, output_json_string_pretty, output_json_string_pretty_for_html,
    output_property, parse_html_to_org, DefuddleOptions, DefuddleOutput, IncludeReplies,
};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use url::Url;

const MAX_FETCH_SIZE: u64 = 5 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_UA: &str = "Mozilla/5.0 (compatible; Defuddle/1.0; +https://defuddle.md)";
const BOT_UA: &str = "Mozilla/5.0 (compatible; Defuddle/1.0; +https://defuddle.md) bot";
const BOT_UA_DOMAINS: &[&str] = &["github.com"];

#[derive(Debug, Default, PartialEq, Eq)]
struct CliOptions {
    source: Option<String>,
    output: Option<PathBuf>,
    markdown: bool,
    separate_markdown: bool,
    json: bool,
    debug: bool,
    frontmatter: bool,
    property: Option<String>,
    language: Option<String>,
    user_agent: Option<String>,
}

fn main() -> ExitCode {
    match run(
        std::env::args().skip(1),
        &mut io::stdout(),
        &mut io::stderr(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run<I, S>(args: I, stdout: &mut dyn Write, _stderr: &mut dyn Write) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let options = parse_args(args)?;
    let (html, url) = read_source(&options)?;
    let mut output = parse_html_to_org(&html, defuddle_options(&options, url.clone()))
        .map_err(|err| err.to_string())?;
    output = retry_url_with_bot_user_agent(output, &options)?;
    ensure_meaningful_content(&output, &options)?;
    let rendered = render_output_with_html(&output, &options, Some(&html))?;

    if let Some(path) = &options.output {
        fs::write(path, rendered)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
        stdout
            .write_all(format!("Output written to {}\n", path.display()).as_bytes())
            .map_err(|err| err.to_string())?;
    } else {
        stdout
            .write_all(rendered.as_bytes())
            .map_err(|err| err.to_string())?;
        stdout.write_all(b"\n").map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn retry_url_with_bot_user_agent(
    output: DefuddleOutput,
    options: &CliOptions,
) -> Result<DefuddleOutput, String> {
    if output.word_count > 0 || options.user_agent.is_some() {
        return Ok(output);
    }
    let Some(source) = options
        .source
        .as_deref()
        .filter(|source| is_http_url(source))
    else {
        return Ok(output);
    };

    let Ok(bot_html) = fetch_page(source, BOT_UA, options.language.as_deref()) else {
        return Ok(output);
    };
    if let Some(markdown) = extract_raw_markdown(&bot_html).map(|raw| clean_markdown_content(&raw))
    {
        let mut bot_options = defuddle_options(options, Some(source.to_string()));
        bot_options.frontmatter = false;
        let mut bot_output =
            parse_html_to_org(&bot_html, bot_options).map_err(|err| err.to_string())?;
        apply_raw_markdown_output(&mut bot_output, &markdown, source, options.frontmatter);
        return Ok(bot_output);
    }
    let bot_output = parse_html_to_org(
        &bot_html,
        defuddle_options(options, Some(source.to_string())),
    )
    .map_err(|err| err.to_string())?;
    if bot_output.word_count > output.word_count {
        Ok(bot_output)
    } else {
        Ok(output)
    }
}

fn apply_raw_markdown_output(
    output: &mut DefuddleOutput,
    markdown: &str,
    source: &str,
    frontmatter: bool,
) {
    output.html = markdown.to_string();
    output.content_markdown = markdown.to_string();
    output.word_count = count_markdown_words(markdown);
    output.frontmatter.clear();
    output.org = markdown_to_org(markdown);
    if frontmatter {
        output.frontmatter = output_frontmatter(output, Some(source));
        if !output.frontmatter.is_empty() {
            output.org = format!("{}{}", output.frontmatter, output.org);
        }
    }
}

fn parse_args<I, S>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into).peekable();
    let Some(command) = args.next() else {
        return Err(usage());
    };
    if command == "-h" || command == "--help" {
        return Err(usage());
    }
    if command == "-V" || command == "--version" {
        return Err(env!("CARGO_PKG_VERSION").to_string());
    }
    if command != "parse" {
        return Err(format!("unknown command: {command}\n{}", usage()));
    }

    let mut options = CliOptions::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(parse_usage()),
            "-m" | "--markdown" | "--md" => options.markdown = true,
            "--separate-markdown" | "--separateMarkdown" => options.separate_markdown = true,
            "-j" | "--json" => options.json = true,
            "-f" | "--frontmatter" => options.frontmatter = true,
            "--debug" => options.debug = true,
            "-o" | "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires a file path"))?;
                options.output = Some(PathBuf::from(value));
            }
            "-p" | "--property" => {
                options.property = Some(
                    args.next()
                        .ok_or_else(|| format!("{arg} requires a property name"))?,
                );
            }
            "-l" | "--lang" => {
                options.language = Some(
                    args.next()
                        .ok_or_else(|| format!("{arg} requires a language code"))?,
                );
            }
            "-u" | "--user-agent" => {
                options.user_agent = Some(
                    args.next()
                        .ok_or_else(|| format!("{arg} requires a user agent"))?,
                );
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ => {
                if options.source.is_some() {
                    return Err(format!("unexpected extra source: {arg}"));
                }
                options.source = Some(arg);
            }
        }
    }
    Ok(options)
}

fn read_source(options: &CliOptions) -> Result<(String, Option<String>), String> {
    match options.source.as_deref() {
        Some("-") | None => {
            if io::stdin().is_terminal() {
                return Err(
                    "No input source provided. Pass a file path or pipe HTML to stdin.".to_string(),
                );
            }
            let mut html = String::new();
            io::stdin()
                .read_to_string(&mut html)
                .map_err(|err| err.to_string())?;
            Ok((html, None))
        }
        Some(source) if is_http_url(source) => fetch_url_source(source, options),
        Some(path) => fs::read_to_string(path)
            .map(|html| (html, None))
            .map_err(|err| format!("failed to read {path}: {err}")),
    }
}

fn is_http_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn fetch_url_source(
    source: &str,
    options: &CliOptions,
) -> Result<(String, Option<String>), String> {
    let user_agent = options
        .user_agent
        .as_deref()
        .unwrap_or_else(|| initial_user_agent(source));
    let html = fetch_page(source, user_agent, options.language.as_deref())?;
    Ok((html, Some(source.to_string())))
}

fn initial_user_agent(source: &str) -> &'static str {
    let Ok(url) = Url::parse(source) else {
        return DEFAULT_UA;
    };
    let Some(host) = url.host_str() else {
        return DEFAULT_UA;
    };
    if BOT_UA_DOMAINS
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    {
        BOT_UA
    } else {
        DEFAULT_UA
    }
}

fn fetch_page(source: &str, user_agent: &str, language: Option<&str>) -> Result<String, String> {
    let env = |name: &str| std::env::var(name).ok();
    fetch_page_with_env(source, user_agent, language, &env)
}

fn fetch_page_with_env(
    source: &str,
    user_agent: &str,
    language: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let mut builder = ureq::AgentBuilder::new()
        .timeout(FETCH_TIMEOUT)
        .redirects(10)
        .try_proxy_from_env(false);
    if let Some(proxy) = proxy_for_source(source, env) {
        if let Ok(proxy) = ureq::Proxy::new(&proxy) {
            builder = builder.proxy(proxy);
        }
    }
    let agent = builder.build();
    let mut request = agent
        .get(source)
        .set("User-Agent", user_agent)
        .set("Accept", "text/html,application/xhtml+xml");
    if let Some(language) = language.filter(|language| !language.trim().is_empty()) {
        request = request.set("Accept-Language", language.trim());
    }

    let response = request.call().map_err(fetch_error_message)?;
    validate_fetch_response(&response)?;
    let content_type = response.header("content-type").unwrap_or("").to_string();

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_FETCH_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read response body: {err}"))?;
    if bytes.len() as u64 > MAX_FETCH_SIZE {
        return Err(format!(
            "Page too large (>{}MB)",
            MAX_FETCH_SIZE / 1024 / 1024
        ));
    }

    decode_html(&bytes, &content_type)
}

fn proxy_for_source(source: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let url = Url::parse(source).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if no_proxy_matches(
        &host,
        env("NO_PROXY").or_else(|| env("no_proxy")).as_deref(),
    ) {
        return None;
    }

    let keys: &[&str] = if url.scheme() == "https" {
        &[
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
        ]
    } else {
        &["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"]
    };

    keys.iter().find_map(|key| {
        let raw = env(key)?;
        let raw = raw.trim();
        (!raw.is_empty() && Url::parse(raw).is_ok()).then(|| raw.to_string())
    })
}

fn no_proxy_matches(host: &str, no_proxy: Option<&str>) -> bool {
    let Some(no_proxy) = no_proxy else {
        return false;
    };
    no_proxy.split(',').map(str::trim).any(|pattern| {
        let pattern = pattern.to_ascii_lowercase();
        if pattern.is_empty() {
            return false;
        }
        if pattern == "*" {
            return true;
        }
        if let Some(suffix) = pattern.strip_prefix('.') {
            host == suffix || host.ends_with(&pattern)
        } else {
            host == pattern || host.ends_with(&format!(".{pattern}"))
        }
    })
}

fn fetch_error_message(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, response) => {
            format!("Failed to fetch: {code} {}", response.status_text())
        }
        ureq::Error::Transport(transport) => transport.to_string(),
    }
}

fn validate_fetch_response(response: &ureq::Response) -> Result<(), String> {
    let content_type = response.header("content-type").unwrap_or("");
    if !is_html_content_type(content_type) {
        return Err(format!("Not an HTML page (content-type: {content_type})"));
    }
    if let Some(length) = response
        .header("content-length")
        .and_then(|length| length.parse::<u64>().ok())
    {
        if length > MAX_FETCH_SIZE {
            return Err(format!(
                "Page too large ({}MB, max {}MB)",
                (length + 1024 * 1024 - 1) / 1024 / 1024,
                MAX_FETCH_SIZE / 1024 / 1024
            ));
        }
    }
    Ok(())
}

fn is_html_content_type(content_type: &str) -> bool {
    let lower = content_type.to_ascii_lowercase();
    lower.contains("text/html") || lower.contains("application/xhtml+xml")
}

fn decode_html(bytes: &[u8], content_type: &str) -> Result<String, String> {
    let charset = detect_charset(content_type, bytes);
    let encoding = Encoding::for_label(charset.as_bytes()).unwrap_or(UTF_8);
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors && encoding == UTF_8 && looks_like_windows_1252(bytes) {
        let (decoded, _, _) = WINDOWS_1252.decode(bytes);
        return Ok(decoded.into_owned());
    }
    Ok(decoded.into_owned())
}

fn detect_charset(content_type: &str, bytes: &[u8]) -> String {
    if let Some(charset) = header_charset(content_type) {
        return charset;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    if let Some(charset) = meta_charset(&head) {
        return charset;
    }
    if looks_like_windows_1252(bytes) {
        return "windows-1252".to_string();
    }
    "utf-8".to_string()
}

fn header_charset(content_type: &str) -> Option<String> {
    content_type.split(';').map(str::trim).find_map(|part| {
        let lower = part.to_ascii_lowercase();
        lower
            .strip_prefix("charset=")
            .map(|charset| charset.trim_matches(['"', '\'']).to_ascii_lowercase())
    })
}

fn meta_charset(head: &str) -> Option<String> {
    let lower = head.to_ascii_lowercase();
    find_after(&lower, "charset=\"")
        .or_else(|| find_after(&lower, "charset='"))
        .or_else(|| find_after(&lower, "charset="))
        .map(|value| {
            value
                .chars()
                .take_while(|ch| !matches!(ch, '"' | '\'' | ';' | '>' | '/') && !ch.is_whitespace())
                .collect::<String>()
        })
        .filter(|charset| !charset.is_empty())
}

fn find_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack
        .find(needle)
        .map(|index| &haystack[index + needle.len()..])
}

fn looks_like_windows_1252(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .take(8192)
        .any(|byte| matches!(byte, 0x80..=0x9f))
        || std::str::from_utf8(bytes).is_err()
}

fn extract_raw_markdown(html: &str) -> Option<String> {
    let body = html_body_inner(html)?;
    let body = remove_element_blocks(&body, &["script", "style", "noscript"]);
    let text = strip_html_tags(&body).trim().to_string();
    (!text.is_empty() && is_markdown_content(&text)).then_some(text)
}

fn html_body_inner(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let body_start = lower.find("<body")?;
    let body_open_end = lower[body_start..].find('>')? + body_start + 1;
    let body_end = lower[body_open_end..]
        .find("</body>")
        .map(|index| body_open_end + index)
        .unwrap_or(html.len());
    Some(html[body_open_end..body_end].to_string())
}

fn remove_element_blocks(input: &str, tags: &[&str]) -> String {
    let mut output = input.to_string();
    for tag in tags {
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(start) = lower.find(&format!("<{tag}")) else {
                break;
            };
            let Some(open_end) = lower[start..].find('>').map(|index| start + index + 1) else {
                output.truncate(start);
                break;
            };
            let close = format!("</{tag}>");
            let end = lower[open_end..]
                .find(&close)
                .map(|index| open_end + index + close.len())
                .unwrap_or(open_end);
            output.replace_range(start..end, "");
        }
    }
    output
}

fn strip_html_tags(input: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn is_markdown_content(content: &str) -> bool {
    let mut signals = 0usize;
    if content.lines().any(|line| markdown_heading(line).is_some()) {
        signals += 1;
    }
    if content.contains("**") {
        signals += 1;
    }
    if content.contains("](") && content.contains('[') {
        signals += 1;
    }
    if content.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ")
    }) {
        signals += 1;
    }
    if content.lines().any(|line| {
        let trimmed = line.trim_start();
        let digits = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
        digits > 0 && trimmed[digits..].starts_with(". ")
    }) {
        signals += 1;
    }
    if content
        .lines()
        .any(|line| line.trim_start().starts_with("> "))
    {
        signals += 1;
    }
    if content.contains("```") {
        signals += 1;
    }
    signals >= 2
}

fn clean_markdown_content(content: &str) -> String {
    let mut markdown = decode_markdown_entities(content).trim().to_string();
    if let Some(rest) = markdown.strip_prefix("# ") {
        if let Some(line_end) = rest.find('\n') {
            markdown = rest[line_end + 1..].trim_start_matches('\n').to_string();
        }
    }
    collapse_blank_lines(&markdown).trim().to_string()
}

fn decode_markdown_entities(content: &str) -> String {
    content
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn collapse_blank_lines(input: &str) -> String {
    let mut output = String::new();
    let mut newline_count = 0usize;
    for ch in input.chars() {
        if ch == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                output.push(ch);
            }
        } else {
            newline_count = 0;
            output.push(ch);
        }
    }
    output
}

fn markdown_to_org(markdown: &str) -> String {
    let mut output = String::new();
    let mut in_code = false;
    let mut in_quote = false;

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if let Some(language) = trimmed.strip_prefix("```") {
            if in_quote {
                output.push_str("#+end_quote\n\n");
                in_quote = false;
            }
            if in_code {
                output.push_str("#+end_src\n\n");
                in_code = false;
            } else {
                output.push_str("#+begin_src");
                let language = language.trim();
                if !language.is_empty() {
                    output.push(' ');
                    output.push_str(language);
                }
                output.push('\n');
                in_code = true;
            }
            continue;
        }

        if in_code {
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if let Some(quoted) = trimmed.strip_prefix('>') {
            if !in_quote {
                output.push_str("#+begin_quote\n");
                in_quote = true;
            }
            output.push_str(&markdown_inline_to_org(quoted.trim_start()));
            output.push('\n');
            continue;
        } else if in_quote {
            output.push_str("#+end_quote\n\n");
            in_quote = false;
        }

        if let Some((level, title)) = markdown_heading(line) {
            output.push_str(&"*".repeat(level));
            output.push(' ');
            output.push_str(&markdown_inline_to_org(title));
            output.push('\n');
        } else {
            output.push_str(&markdown_inline_to_org(line));
            output.push('\n');
        }
    }

    if in_code {
        output.push_str("#+end_src\n");
    }
    if in_quote {
        output.push_str("#+end_quote\n");
    }

    collapse_blank_lines(output.trim()).trim().to_string() + "\n"
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
        Some((level, trimmed[level + 1..].trim()))
    } else {
        None
    }
}

fn markdown_inline_to_org(line: &str) -> String {
    markdown_links_to_org(line)
        .replace("**", "*")
        .replace("__", "*")
}

fn markdown_links_to_org(input: &str) -> String {
    let mut output = String::new();
    let mut index = 0usize;
    while index < input.len() {
        let rest = &input[index..];
        let (is_image, label_start) = if rest.starts_with("![") {
            (true, index + 2)
        } else if rest.starts_with('[') {
            (false, index + 1)
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
            continue;
        };

        let Some(close_label) = input[label_start..]
            .find("](")
            .map(|offset| label_start + offset)
        else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
            continue;
        };
        let url_start = close_label + 2;
        let Some(close_url) = input[url_start..]
            .find(')')
            .map(|offset| url_start + offset)
        else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
            continue;
        };

        let label = input[label_start..close_label].trim();
        let url = input[url_start..close_url].trim();
        if url.is_empty() {
            output.push_str(&input[index..=close_url]);
        } else if is_image || label.is_empty() {
            output.push_str("[[");
            output.push_str(url);
            output.push_str("]]");
        } else {
            output.push_str("[[");
            output.push_str(url);
            output.push_str("][");
            output.push_str(label);
            output.push_str("]]");
        }
        index = close_url + 1;
    }
    output
}

fn count_markdown_words(markdown: &str) -> usize {
    markdown
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .count()
}

fn ensure_meaningful_content(output: &DefuddleOutput, options: &CliOptions) -> Result<(), String> {
    if !strip_html_tags(&output.html).trim().is_empty() {
        return Ok(());
    }
    Err(format!(
        "No content could be extracted from {}",
        source_label(options)
    ))
}

fn source_label(options: &CliOptions) -> &str {
    options
        .source
        .as_deref()
        .filter(|source| !source.is_empty() && *source != "-")
        .unwrap_or("stdin")
}

fn defuddle_options(options: &CliOptions, url: Option<String>) -> DefuddleOptions {
    DefuddleOptions {
        url,
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
        debug: options.debug,
        profile: options.debug,
        frontmatter: options.frontmatter,
        markdown: wants_markdown_output(options),
        separate_markdown: options.separate_markdown,
    }
}

#[cfg(test)]
fn render_output(output: &DefuddleOutput, options: &CliOptions) -> Result<String, String> {
    render_output_with_html(output, options, None)
}

fn render_output_with_html(
    output: &DefuddleOutput,
    options: &CliOptions,
    source_html: Option<&str>,
) -> Result<String, String> {
    if let Some(property) = &options.property {
        return output_property(output, property)
            .ok_or_else(|| format!("Property \"{property}\" not found in response"));
    }
    if options.json {
        if let Some(source_html) = source_html {
            return output_json_string_pretty_for_html(output, source_html)
                .map_err(|err| err.to_string());
        }
        return output_json_string_pretty(output).map_err(|err| err.to_string());
    }
    if options.markdown {
        if options.frontmatter && !output.frontmatter.is_empty() {
            return Ok(format!("{}{}", output.frontmatter, output.content_markdown));
        }
        return Ok(output.content_markdown.clone());
    }
    Ok(output.org.clone())
}

fn wants_markdown_output(options: &CliOptions) -> bool {
    options.markdown
        || options.separate_markdown
        || options.json
        || options
            .property
            .as_deref()
            .is_some_and(property_requests_markdown)
}

fn property_requests_markdown(property: &str) -> bool {
    matches!(
        property,
        "contentMarkdown" | "content_markdown" | "markdown"
    )
}

fn usage() -> String {
    "Usage: org-defuddle parse [source] [options]".to_string()
}

fn parse_usage() -> String {
    [
        "Usage: org-defuddle parse [source] [options]",
        "",
        "Options:",
        "  -o, --output <file>       Write output to a file",
        "  -j, --json                Output JSON with metadata and content",
        "  -f, --frontmatter         Prepend YAML frontmatter",
        "  -p, --property <name>     Extract a specific property",
        "  -m, --markdown, --md      Output Markdown content",
        "      --separate-markdown   Also populate contentMarkdown",
        "      --debug               Include debug/profile fields",
        "  -l, --lang <code>         Accepted for defuddle CLI compatibility",
        "  -u, --user-agent <value>  Accepted for defuddle CLI compatibility",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::net::TcpListener;
    use std::sync::mpsc::{self, Receiver};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn sample_output() -> DefuddleOutput {
        DefuddleOutput {
            title: "Hello".to_string(),
            description: "Description".to_string(),
            author: "Ada".to_string(),
            published: String::new(),
            site: "Example".to_string(),
            url: String::new(),
            domain: "example.com".to_string(),
            favicon: String::new(),
            image: String::new(),
            language: "en".to_string(),
            word_count: 12,
            parse_time: 3,
            html: "<article>Hello</article>".to_string(),
            org: "* Hello\n\nBody\n".to_string(),
            content_markdown: "# Hello\n\nBody\n".to_string(),
            frontmatter: String::new(),
            extractor_type: Some("youtube".to_string()),
            variables: Some(HashMap::from([(
                "transcript".to_string(),
                "Hello transcript".to_string(),
            )])),
            debug: None,
            profile: None,
        }
    }

    #[test]
    fn parses_upstream_like_parse_options() {
        let options = parse_args([
            "parse",
            "page.html",
            "--json",
            "--markdown",
            "--separate-markdown",
            "--frontmatter",
            "--property",
            "title",
            "-o",
            "out.txt",
            "--debug",
            "--lang",
            "en",
            "--user-agent",
            "agent",
        ])
        .unwrap();

        assert_eq!(options.source.as_deref(), Some("page.html"));
        assert_eq!(options.property.as_deref(), Some("title"));
        assert_eq!(options.output, Some(PathBuf::from("out.txt")));
        assert!(options.json);
        assert!(options.markdown);
        assert!(options.separate_markdown);
        assert!(options.frontmatter);
        assert!(options.debug);
        assert_eq!(options.language.as_deref(), Some("en"));
        assert_eq!(options.user_agent.as_deref(), Some("agent"));
    }

    #[test]
    fn parses_separate_markdown_camel_case_alias() {
        let options = parse_args(["parse", "page.html", "--separateMarkdown"]).unwrap();

        assert_eq!(options.source.as_deref(), Some("page.html"));
        assert!(options.separate_markdown);
        assert!(wants_markdown_output(&options));
    }

    #[test]
    fn renders_property_and_json() {
        let output = sample_output();
        let property = CliOptions {
            property: Some("wordCount".to_string()),
            ..CliOptions::default()
        };
        assert_eq!(render_output(&output, &property).unwrap(), "12");

        let json_options = CliOptions {
            json: true,
            ..CliOptions::default()
        };
        let rendered = render_output(&output, &json_options).unwrap();
        assert!(rendered.contains("\"wordCount\": 12"));
        assert!(rendered.contains("\"word_count\": 12"));
        assert!(rendered.contains("\"parseTime\": 3"));
        assert!(rendered.contains("\"parse_time\": 3"));
        assert!(rendered.contains("\"content\": \"<article>Hello</article>\""));
        assert!(rendered.contains("\"html\": \"<article>Hello</article>\""));
        assert!(rendered.contains("\"org\": \"* Hello\\n\\nBody\\n\""));
        assert!(rendered.contains("\"contentMarkdown\": \"# Hello\\n\\nBody\\n\""));
        assert!(rendered.contains("\"content_markdown\": \"# Hello\\n\\nBody\\n\""));
        assert!(rendered.contains("\"extractorType\": \"youtube\""));
        assert!(rendered.contains("\"transcript\": \"Hello transcript\""));

        let source_html = r#"<!doctype html>
        <html>
          <head>
            <meta name="description" content="CLI meta description">
            <script type="application/ld+json">{"@type":"Article","headline":"CLI schema headline"}</script>
          </head>
          <body><article>Hello</article></body>
        </html>"#;
        let rendered_with_html =
            render_output_with_html(&output, &json_options, Some(source_html)).unwrap();
        assert!(rendered_with_html.contains("\"schemaOrgData\""));
        assert!(rendered_with_html.contains("\"headline\": \"CLI schema headline\""));
        assert!(rendered_with_html.contains("\"metaTags\""));
        assert!(rendered_with_html.contains("\"content\": \"CLI meta description\""));

        let markdown_options = CliOptions {
            markdown: true,
            separate_markdown: false,
            ..CliOptions::default()
        };
        assert_eq!(
            render_output(&output, &markdown_options).unwrap(),
            "# Hello\n\nBody\n"
        );
    }

    #[test]
    fn output_file_reports_written_path() {
        let input_path = unique_temp_path("input", "html");
        let output_path = unique_temp_path("output", "org");
        let html = "<!doctype html><html><head><title>File Output</title></head><body><article><h1>File Output</h1><p>Rendered body content goes to the requested file.</p></article></body></html>";
        fs::write(&input_path, html).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = run(
            vec![
                "parse".to_string(),
                input_path.display().to_string(),
                "--output".to_string(),
                output_path.display().to_string(),
            ],
            &mut stdout,
            &mut stderr,
        );

        let _ = fs::remove_file(&input_path);
        assert!(result.is_ok());
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            format!("Output written to {}\n", output_path.display())
        );
        let rendered = fs::read_to_string(&output_path).unwrap();
        let _ = fs::remove_file(&output_path);
        assert!(rendered.contains("* File Output"));
        assert!(rendered.contains("Rendered body content"));
    }

    #[test]
    fn fetches_url_source_and_forwards_headers() {
        let body = b"<!doctype html><html><head><title>Fetched Title</title></head><body><article><h1>Fetched Title</h1><p>Fetched body content.</p></article></body></html>".to_vec();
        let (url, request) = serve_once("HTTP/1.1 200 OK", &[("Content-Type", "text/html")], body);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run(
            vec![
                "parse".to_string(),
                url,
                "--property".to_string(),
                "title".to_string(),
                "--lang".to_string(),
                "fr".to_string(),
                "--user-agent".to_string(),
                "TestAgent".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(String::from_utf8(stdout).unwrap(), "Fetched Title\n");
        let request = request.recv_timeout(Duration::from_secs(1)).unwrap();
        let request = request.to_ascii_lowercase();
        assert!(request.contains("user-agent: testagent"));
        assert!(request.contains("accept-language: fr"));
        assert!(request.contains("accept: text/html,application/xhtml+xml"));
    }

    #[test]
    fn selects_proxy_from_environment_like_defuddle() {
        let env = env_from(&[
            ("HTTP_PROXY", "http://proxy.local:8080"),
            ("HTTPS_PROXY", "http://secure-proxy.local:8443"),
            ("ALL_PROXY", "http://all-proxy.local:7070"),
        ]);
        assert_eq!(
            proxy_for_source("http://example.com/post", &env).as_deref(),
            Some("http://proxy.local:8080")
        );
        assert_eq!(
            proxy_for_source("https://example.com/post", &env).as_deref(),
            Some("http://secure-proxy.local:8443")
        );

        let fallback_env = env_from(&[("ALL_PROXY", "http://all-proxy.local:7070")]);
        assert_eq!(
            proxy_for_source("https://example.com/post", &fallback_env).as_deref(),
            Some("http://all-proxy.local:7070")
        );

        let invalid_env = env_from(&[("HTTP_PROXY", "not a url")]);
        assert!(proxy_for_source("http://example.com/post", &invalid_env).is_none());
    }

    #[test]
    fn no_proxy_skips_matching_hosts() {
        let env = env_from(&[
            ("HTTP_PROXY", "http://proxy.local:8080"),
            ("NO_PROXY", "example.com, .internal"),
        ]);
        assert!(proxy_for_source("http://example.com/post", &env).is_none());
        assert!(proxy_for_source("http://sub.example.com/post", &env).is_none());
        assert!(proxy_for_source("http://api.internal/post", &env).is_none());
        assert!(proxy_for_source("http://elsewhere.test/post", &env).is_some());

        let wildcard_env =
            env_from(&[("HTTP_PROXY", "http://proxy.local:8080"), ("NO_PROXY", "*")]);
        assert!(proxy_for_source("http://elsewhere.test/post", &wildcard_env).is_none());
    }

    #[test]
    fn fetch_page_uses_http_proxy_from_environment() {
        let body = b"<!doctype html><html><head><title>Proxy Title</title></head><body><article><h1>Proxy Title</h1><p>Proxy body content.</p></article></body></html>".to_vec();
        let (proxy_url, request) =
            serve_proxy_once("HTTP/1.1 200 OK", &[("Content-Type", "text/html")], body);
        let env = |key: &str| (key == "HTTP_PROXY").then(|| proxy_url.clone());

        let html =
            fetch_page_with_env("http://example.test/article", DEFAULT_UA, None, &env).unwrap();

        assert!(html.contains("Proxy Title"));
        let request = request.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(request.starts_with("GET http://example.test/article HTTP/1.1"));
        assert!(request.to_ascii_lowercase().contains("host: example.test"));
    }

    #[test]
    fn retries_empty_url_extraction_with_bot_user_agent() {
        let client_body =
            b"<!doctype html><html><head><title>Client Shell</title></head><body><div id=\"app\"></div></body></html>"
                .to_vec();
        let bot_body = b"<!doctype html><html><head><title>Bot Title</title></head><body><article><h1>Bot Title</h1><p>Bot body content appears in server-rendered HTML.</p></article></body></html>"
            .to_vec();
        let (url, requests) = serve_responses(vec![
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                client_body,
            ),
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                bot_body,
            ),
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run(
            vec![
                "parse".to_string(),
                url,
                "--property".to_string(),
                "title".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(String::from_utf8(stdout).unwrap(), "Bot Title\n");
        let first = requests
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .to_ascii_lowercase();
        let second = requests
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .to_ascii_lowercase();
        assert!(first
            .contains("user-agent: mozilla/5.0 (compatible; defuddle/1.0; +https://defuddle.md)"));
        assert!(!first.contains(
            "user-agent: mozilla/5.0 (compatible; defuddle/1.0; +https://defuddle.md) bot"
        ));
        assert!(second.contains(
            "user-agent: mozilla/5.0 (compatible; defuddle/1.0; +https://defuddle.md) bot"
        ));
    }

    #[test]
    fn custom_user_agent_suppresses_bot_retry() {
        let client_body =
            b"<!doctype html><html><head><title>Client Shell</title></head><body><div id=\"app\"></div></body></html>"
                .to_vec();
        let bot_body = b"<!doctype html><html><head><title>Unexpected Bot</title></head><body><article><h1>Unexpected Bot</h1><p>Bot content should not be fetched.</p></article></body></html>"
            .to_vec();
        let (url, requests) = serve_responses(vec![
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                client_body,
            ),
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                bot_body,
            ),
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let err = run(
            vec![
                "parse".to_string(),
                url,
                "--property".to_string(),
                "title".to_string(),
                "--user-agent".to_string(),
                "CustomAgent".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        )
        .unwrap_err();

        assert_eq!(stdout, b"");
        let first = requests
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .to_ascii_lowercase();
        assert!(first.contains("user-agent: customagent"));
        assert!(requests.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(err.contains("No content could be extracted from http://"));
    }

    #[test]
    fn errors_when_url_retry_still_has_no_content() {
        let shell =
            b"<!doctype html><html><head><title>Client Shell</title></head><body><div id=\"app\"></div></body></html>"
                .to_vec();
        let (url, requests) = serve_responses(vec![
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                shell.clone(),
            ),
            TestResponse::new("HTTP/1.1 200 OK", &[("Content-Type", "text/html")], shell),
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let err = run(vec!["parse".to_string(), url], &mut stdout, &mut stderr).unwrap_err();

        assert_eq!(stdout, b"");
        assert!(err.contains("No content could be extracted from http://"));
        assert!(requests.recv_timeout(Duration::from_secs(1)).is_ok());
        assert!(requests.recv_timeout(Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn bot_retry_extracts_raw_markdown_before_dom_parsing() {
        let client_body =
            b"<!doctype html><html><head><title>Client Shell</title></head><body><div id=\"app\"></div></body></html>"
                .to_vec();
        let bot_body = br#"<!doctype html><html><head><title>Raw Markdown</title></head><body>
# Raw Markdown

## Section

This is **bold** prose with a [link](https://example.com).

- First item
- Second item

```rust
let answer = 42;
```
</body></html>"#
            .to_vec();
        let (url, _requests) = serve_responses(vec![
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                client_body,
            ),
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                bot_body,
            ),
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run(
            vec!["parse".to_string(), url, "--markdown".to_string()],
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        let rendered = String::from_utf8(stdout).unwrap();
        assert!(!rendered.contains("# Raw Markdown"));
        assert!(rendered.contains("## Section"));
        assert!(rendered.contains("This is **bold** prose"));
        assert!(rendered.contains("```rust\nlet answer = 42;\n```"));
    }

    #[test]
    fn bot_retry_raw_markdown_default_output_is_org() {
        let client_body =
            b"<!doctype html><html><head><title>Client Shell</title></head><body><div id=\"app\"></div></body></html>"
                .to_vec();
        let bot_body = br#"<!doctype html><html><head><title>Raw Markdown</title></head><body>
# Raw Markdown

## Section

This is **bold** prose with a [link](https://example.com).

> Quoted text.

```rust
let answer = 42;
```
</body></html>"#
            .to_vec();
        let (url, _requests) = serve_responses(vec![
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                client_body,
            ),
            TestResponse::new(
                "HTTP/1.1 200 OK",
                &[("Content-Type", "text/html")],
                bot_body,
            ),
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run(vec!["parse".to_string(), url], &mut stdout, &mut stderr).unwrap();

        let rendered = String::from_utf8(stdout).unwrap();
        assert!(rendered.contains("** Section"));
        assert!(rendered.contains("*bold* prose with a [[https://example.com][link]]"));
        assert!(rendered.contains("#+begin_quote\nQuoted text.\n#+end_quote"));
        assert!(rendered.contains("#+begin_src rust\nlet answer = 42;\n#+end_src"));
    }

    #[test]
    fn rejects_non_html_url_response() {
        let (url, _request) = serve_once(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "application/json")],
            br#"{"ok":true}"#.to_vec(),
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let err = run(vec!["parse".to_string(), url], &mut stdout, &mut stderr).unwrap_err();

        assert!(err.contains("Not an HTML page"));
    }

    #[test]
    fn decodes_url_response_charset() {
        let mut body = b"<!doctype html><html><head><title>Caf".to_vec();
        body.push(0xe9);
        body.extend_from_slice(
            b"</title></head><body><article><h1>Caf</h1><p>Enough content for extraction.</p></article></body></html>",
        );
        let (url, _request) = serve_once(
            "HTTP/1.1 200 OK",
            &[("Content-Type", "text/html; charset=windows-1252")],
            body,
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run(
            vec![
                "parse".to_string(),
                url,
                "--property".to_string(),
                "title".to_string(),
            ],
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(stdout, vec![b'C', b'a', b'f', 0xc3, 0xa9, b'\n']);
    }

    fn serve_once(
        status_line: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> (String, Receiver<String>) {
        serve_responses(vec![TestResponse::new(status_line, headers, body)])
    }

    struct TestResponse {
        status_line: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl TestResponse {
        fn new(status_line: &str, headers: &[(&str, &str)], body: Vec<u8>) -> Self {
            Self {
                status_line: status_line.to_string(),
                headers: headers
                    .iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
                body,
            }
        }
    }

    fn serve_responses(responses: Vec<TestResponse>) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/article");
        let rx = serve_responses_on_listener(listener, responses);
        (url, rx)
    }

    fn serve_proxy_once(
        status_line: &str,
        headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}");
        let rx = serve_responses_on_listener(
            listener,
            vec![TestResponse::new(status_line, headers, body)],
        );
        (url, rx)
    }

    fn serve_responses_on_listener(
        listener: TcpListener,
        responses: Vec<TestResponse>,
    ) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                tx.send(String::from_utf8_lossy(&request).into_owned())
                    .unwrap();

                let mut header = format!("{}\r\nConnection: close\r\n", response.status_line);
                if !response
                    .headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                {
                    header.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
                }
                for (name, value) in response.headers {
                    header.push_str(&name);
                    header.push_str(": ");
                    header.push_str(&value);
                    header.push_str("\r\n");
                }
                header.push_str("\r\n");
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(&response.body).unwrap();
            }
        });

        rx
    }

    fn env_from<'a>(entries: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            entries
                .iter()
                .find(|(entry_key, _)| *entry_key == key)
                .map(|(_, value)| value.to_string())
        }
    }

    fn unique_temp_path(prefix: &str, extension: &str) -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "org-defuddle-cli-{prefix}-{}-{id}.{extension}",
            std::process::id()
        ))
    }
}
