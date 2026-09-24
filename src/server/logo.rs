use super::app::App;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use regex::Regex;
use reqwest::Client;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use url::Url;

const MAX_ICON_BYTES: usize = 256 * 1024;
const MAX_PAGE_BYTES: usize = 256 * 1024;
pub struct Logo {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

pub fn client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(4))
        .user_agent("webdash-rs/0.1")
        .build()
}

pub async fn service_logo(Path(key): Path<String>, State(app): State<Arc<App>>) -> Response {
    let logo = match app.logo_url(&key).await {
        Some(url) => fetch_logo(&app.logo_client, &url).await,
        None => None,
    };
    match logo {
        Some(logo) => (
            [
                (header::CONTENT_TYPE, logo.mime),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            logo.bytes,
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            [(header::CACHE_CONTROL, "public, max-age=60")],
        )
            .into_response(),
    }
}

async fn fetch_logo(client: &Client, service_url: &str) -> Option<Logo> {
    let service = Url::parse(service_url).ok()?;
    let mut candidates = self_declared_icons(client, &service).await;
    candidates.extend(
        [
            "/favicon.ico",
            "/Content/Images/Icons/favicon-32x32.png",
            "/web/favicon.ico",
        ]
        .into_iter()
        .filter_map(|path| service.join(path).ok()),
    );

    for candidate in candidates {
        if let Some(logo) = fetch_image(client, candidate).await {
            return Some(logo);
        }
    }
    None
}

async fn self_declared_icons(client: &Client, service: &Url) -> Vec<Url> {
    if let Ok(page) = client.get(service.clone()).send().await {
        if page.status().is_success() && page.url().origin() == service.origin() {
            let page_url = page.url().clone();
            if let Some(bytes) = read_limited(page, MAX_PAGE_BYTES).await {
                return icon_links(&String::from_utf8_lossy(&bytes), &page_url, service);
            }
        }
    }
    Vec::new()
}

async fn fetch_image(client: &Client, url: Url) -> Option<Logo> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = read_limited(response, MAX_ICON_BYTES).await?;
    let mime = image_mime(&bytes)?;
    Some(Logo { bytes, mime })
}

async fn read_limited(mut response: reqwest::Response, limit: usize) -> Option<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > limit {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    Some(bytes)
}

fn icon_links(html: &str, page: &Url, service: &Url) -> Vec<Url> {
    static LINKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<link\b[^>]*>").unwrap());
    let mut icons = Vec::new();
    for tag in LINKS.find_iter(html) {
        if let Some(url) = icon_from_tag(tag.as_str(), page, service) {
            if !icons.contains(&url) {
                icons.push(url);
            }
        }
    }
    icons
}

fn icon_from_tag(tag: &str, page: &Url, service: &Url) -> Option<Url> {
    static ATTRIBUTES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)([a-z][a-z0-9-]*)\s*=\s*["']([^"']*)["']"#).unwrap());
    let mut rel = None;
    let mut href = None;
    for attribute in ATTRIBUTES.captures_iter(tag) {
        match &attribute[1].to_ascii_lowercase()[..] {
            "rel" => rel = Some(attribute[2].to_ascii_lowercase()),
            "href" => href = Some(attribute[2].to_string()),
            _ => {}
        }
    }
    let is_icon = rel?
        .split_ascii_whitespace()
        .any(|token| matches!(token, "icon" | "apple-touch-icon"));
    if !is_icon {
        return None;
    }
    let url = page.join(&href?).ok()?;
    (url.origin() == service.origin()).then_some(url)
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_icon_from_redirected_homepage() {
        let service = Url::parse("https://jellyfin.example.com/").unwrap();
        let page = Url::parse("https://jellyfin.example.com/web/").unwrap();
        let html = r#"<link rel="shortcut icon" href="favicon.123.ico">"#;
        let icons = icon_links(html, &page, &service);
        assert_eq!(
            icons.iter().map(Url::as_str).collect::<Vec<_>>(),
            ["https://jellyfin.example.com/web/favicon.123.ico"]
        );
    }

    #[test]
    fn extracts_declared_icons_in_order_without_duplicates() {
        let service = Url::parse("https://example.com/").unwrap();
        let page = Url::parse("https://example.com/app/").unwrap();
        let html = r#"
            <link HREF='/favicon.ico' REL='ICON'>
            <link rel="apple-touch-icon" href="touch.png">
            <link rel="shortcut icon" href="/favicon.ico">
            <link rel="stylesheet" href="style.css">
            <link rel="iconography" href="not-an-icon.png">
        "#;

        let icons = icon_links(html, &page, &service);
        assert_eq!(
            icons.iter().map(Url::as_str).collect::<Vec<_>>(),
            [
                "https://example.com/favicon.ico",
                "https://example.com/app/touch.png"
            ]
        );
    }

    #[test]
    fn ignores_icons_outside_the_service_origin() {
        let service = Url::parse("https://example.com/").unwrap();
        let html = r#"
            <link rel="icon" href="https://other.example.com/icon.png">
            <link rel="icon" href="//example.com:8443/icon.png">
            <link rel="icon" href="javascript:alert(1)">
            <link rel="icon" href="/safe.png">
        "#;

        let icons = icon_links(html, &service, &service);
        assert_eq!(
            icons.iter().map(Url::as_str).collect::<Vec<_>>(),
            ["https://example.com/safe.png"]
        );
    }

    #[test]
    fn rejects_non_image_responses() {
        assert_eq!(image_mime(b"<html>login</html>"), None);
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
    }
}
