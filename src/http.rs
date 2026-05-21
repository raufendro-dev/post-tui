use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};

use crate::models::{AuthConfig, BodyConfig, Header, RequestModel, ResponseModel};

pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("post-tui/0.1")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .context("failed to create HTTP client")?;
        Ok(Self { client })
    }

    pub async fn send(&self, request: &RequestModel) -> ResponseModel {
        match self.try_send(request).await {
            Ok(response) => response,
            Err(error) => ResponseModel {
                error: Some(error.to_string()),
                ..ResponseModel::default()
            },
        }
    }

    async fn try_send(&self, request: &RequestModel) -> Result<ResponseModel> {
        if request.url.trim().is_empty() {
            return Err(anyhow!("Enter a URL before sending the request."));
        }

        let mut url = Url::parse(request.url.trim())
            .map_err(|_| anyhow!("That URL does not look valid. Include http:// or https://."))?;

        {
            let mut pairs = url.query_pairs_mut();
            for param in request.query_params.iter().filter(|param| param.enabled) {
                if !param.key.trim().is_empty() {
                    pairs.append_pair(param.key.trim(), param.value.as_str());
                }
            }
        }

        let mut builder = self.client.request(request.method.into(), url);

        for header in request.headers.iter().filter(|header| header.enabled) {
            if !header.key.trim().is_empty() {
                builder = builder.header(header.key.trim(), header.value.as_str());
            }
        }

        builder = match &request.auth {
            AuthConfig::None => builder,
            AuthConfig::Bearer { token } if !token.trim().is_empty() => {
                builder.bearer_auth(token.trim())
            }
            AuthConfig::Basic { username, password } => {
                builder.basic_auth(username, Some(password))
            }
            AuthConfig::ApiKeyHeader { key, value } if !key.trim().is_empty() => {
                builder.header(key.trim(), value.as_str())
            }
            _ => builder,
        };

        if let BodyConfig::Raw { content_type, body } = &request.body {
            if !body.is_empty() {
                if !content_type.trim().is_empty() {
                    builder = builder.header("content-type", content_type.trim());
                }
                builder = builder.body(body.clone());
            }
        }

        let started = Instant::now();
        let response = builder
            .send()
            .await
            .map_err(|error| anyhow!("Network request failed: {error}"))?;
        let elapsed_ms = started.elapsed().as_millis();

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let headers = response
            .headers()
            .iter()
            .map(|(key, value)| Header {
                key: key.to_string(),
                value: value.to_str().unwrap_or("<binary header>").to_string(),
                enabled: true,
            })
            .collect::<Vec<_>>();

        let bytes = response
            .bytes()
            .await
            .map_err(|error| anyhow!("Failed reading response body: {error}"))?;
        let size_bytes = bytes.len();
        let body = String::from_utf8_lossy(&bytes).to_string();
        let pretty_body = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|json| serde_json::to_string_pretty(&json).ok());
        let json_tree_body = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .map(|json| json_tree_text(&json));
        let html_body = if content_type.contains("text/html") || looks_like_html(&body) {
            Some(html_to_text(&body))
        } else {
            None
        };

        Ok(ResponseModel {
            status: Some(status.as_u16()),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            elapsed_ms,
            size_bytes,
            headers,
            body,
            pretty_body,
            json_tree_body,
            html_body,
            error: None,
        })
    }
}

fn json_tree_text(value: &serde_json::Value) -> String {
    let mut out = String::new();
    write_json_tree(value, 0, "root", &mut out);
    out.trim_end().to_string()
}

fn write_json_tree(value: &serde_json::Value, depth: usize, label: &str, out: &mut String) {
    let indent = "  ".repeat(depth);
    match value {
        serde_json::Value::Object(map) if map.is_empty() => {
            out.push_str(&format!("{indent}{label}: {{}}\n"));
        }
        serde_json::Value::Object(map) => {
            out.push_str(&format!("{indent}{label}: object ({})\n", map.len()));
            for (key, value) in map {
                write_json_tree(value, depth + 1, key, out);
            }
        }
        serde_json::Value::Array(items) if items.is_empty() => {
            out.push_str(&format!("{indent}{label}: []\n"));
        }
        serde_json::Value::Array(items) => {
            out.push_str(&format!("{indent}{label}: array ({})\n", items.len()));
            for (index, value) in items.iter().enumerate() {
                write_json_tree(value, depth + 1, &format!("[{index}]"), out);
            }
        }
        serde_json::Value::String(value) => {
            out.push_str(&format!("{indent}{label}: \"{}\"\n", truncate(value, 120)));
        }
        serde_json::Value::Number(value) => {
            out.push_str(&format!("{indent}{label}: {value}\n"));
        }
        serde_json::Value::Bool(value) => {
            out.push_str(&format!("{indent}{label}: {value}\n"));
        }
        serde_json::Value::Null => {
            out.push_str(&format!("{indent}{label}: null\n"));
        }
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!("{}...", value.chars().take(max).collect::<String>())
    }
}

fn looks_like_html(body: &str) -> bool {
    let sample = body.trim_start().to_ascii_lowercase();
    sample.starts_with("<!doctype html")
        || sample.starts_with("<html")
        || sample.contains("<body")
        || sample.contains("<p>")
}

fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(16_384));
    let mut tag = String::new();
    let mut in_tag = false;
    let mut skip_until: Option<&'static str> = None;
    let mut entity = String::new();
    let mut in_entity = false;

    for ch in html.chars() {
        if let Some(end_tag) = skip_until {
            if ch == '<' {
                in_tag = true;
                tag.clear();
            } else if in_tag && ch == '>' {
                let normalized = tag.trim().to_ascii_lowercase();
                if normalized.starts_with(end_tag) || normalized.starts_with(&format!("/{end_tag}"))
                {
                    skip_until = None;
                }
                in_tag = false;
            } else if in_tag {
                tag.push(ch);
            }
            continue;
        }

        if in_tag {
            if ch == '>' {
                apply_tag(&mut out, tag.trim());
                let normalized = tag.trim().to_ascii_lowercase();
                if normalized.starts_with("script") {
                    skip_until = Some("/script");
                } else if normalized.starts_with("style") {
                    skip_until = Some("/style");
                }
                tag.clear();
                in_tag = false;
            } else {
                tag.push(ch);
            }
            continue;
        }

        if in_entity {
            if ch == ';' {
                out.push_str(decode_entity(&entity).unwrap_or("?"));
                entity.clear();
                in_entity = false;
            } else if entity.len() < 16 {
                entity.push(ch);
            } else {
                out.push('&');
                out.push_str(&entity);
                entity.clear();
                in_entity = false;
            }
            continue;
        }

        match ch {
            '<' => {
                flush_entity(&mut out, &mut entity, &mut in_entity);
                in_tag = true;
                tag.clear();
            }
            '&' => {
                in_entity = true;
                entity.clear();
            }
            _ => out.push(ch),
        }
    }

    flush_entity(&mut out, &mut entity, &mut in_entity);
    normalize_text(&out)
}

fn apply_tag(out: &mut String, tag: &str) {
    let normalized = tag
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('/')
        .to_ascii_lowercase();
    let closing = tag.trim_start().starts_with('/');

    match normalized.as_str() {
        "h1" | "h2" | "h3" if !closing => {
            ensure_blank_line(out);
            out.push_str("# ");
        }
        "h4" | "h5" | "h6" if !closing => {
            ensure_blank_line(out);
            out.push_str("## ");
        }
        "p" | "div" | "section" | "article" | "header" | "footer" if closing => {
            ensure_blank_line(out)
        }
        "br" => out.push('\n'),
        "li" if !closing => {
            ensure_line(out);
            out.push_str("- ");
        }
        "tr" if closing => out.push('\n'),
        "td" | "th" if closing => out.push_str("  "),
        "title" if !closing => ensure_blank_line(out),
        _ => {}
    }
}

fn decode_entity(entity: &str) -> Option<&'static str> {
    match entity {
        "amp" => Some("&"),
        "lt" => Some("<"),
        "gt" => Some(">"),
        "quot" => Some("\""),
        "apos" | "#39" => Some("'"),
        "nbsp" => Some(" "),
        "copy" => Some("(c)"),
        "reg" => Some("(r)"),
        _ => None,
    }
}

fn flush_entity(out: &mut String, entity: &mut String, in_entity: &mut bool) {
    if *in_entity {
        out.push('&');
        out.push_str(entity);
        entity.clear();
        *in_entity = false;
    }
}

fn ensure_line(out: &mut String) {
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
}

fn ensure_blank_line(out: &mut String) {
    while out.ends_with(" \n") {
        out.pop();
        out.pop();
        out.push('\n');
    }
    if out.is_empty() {
        return;
    }
    if !out.ends_with("\n\n") {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
}

fn normalize_text(input: &str) -> String {
    let mut output = String::new();
    let mut blank_lines = 0;

    for line in input.lines() {
        let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.is_empty() {
            blank_lines += 1;
            if blank_lines <= 1 && !output.is_empty() {
                output.push('\n');
            }
        } else {
            blank_lines = 0;
            output.push_str(&compact);
            output.push('\n');
        }
    }

    output.trim().to_string()
}
