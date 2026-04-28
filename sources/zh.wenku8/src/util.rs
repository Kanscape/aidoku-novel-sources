use aidoku::{
    Manga, MangaStatus,
    alloc::{String, Vec, string::ToString},
    imports::html::Element,
    prelude::*,
};
use encoding_rs::GBK;

use crate::consts::{BASE_URL, IMG_URL, PIC_URL};

pub(crate) fn mewx_cover_url(key: &str) -> String {
    let group = key.parse::<i32>().unwrap_or_default() / 1000;
    format!("{IMG_URL}/image/{group}/{key}/{key}s.jpg")
}

pub(crate) fn decode_basic_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
}

pub(crate) fn clean_description(raw: String) -> Option<String> {
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut result = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !result.is_empty() && !result.ends_with("\n\n") {
                result.push('\n');
            }
            continue;
        }
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(line);
    }

    let result = result.trim().to_string();
    if result.is_empty() || is_detail_metadata(&result) {
        None
    } else {
        Some(result)
    }
}

pub(crate) fn is_detail_metadata(text: &str) -> bool {
    let text = text.trim();
    let labels = [
        "作品Tags",
        "点击数",
        "點擊數",
        "总点击",
        "總點擊",
        "月点击",
        "月點擊",
        "周点击",
        "周點擊",
        "日点击",
        "日點擊",
        "推荐数",
        "推薦數",
        "总推荐",
        "總推薦",
        "月推荐",
        "月推薦",
        "周推荐",
        "周推薦",
        "日推荐",
        "日推薦",
        "文章状态",
        "文章狀態",
        "最后更新",
        "最後更新",
        "全文长度",
        "全文長度",
        "小说作者",
        "小說作者",
        "文库分类",
        "文庫分類",
        "动画化",
        "動畫化",
    ];
    labels.iter().any(|label| text.starts_with(label))
}

pub(crate) fn parse_status(text: &str) -> MangaStatus {
    let text = text.trim();
    if text == "1" || text.contains("已完成") || text.contains("已完结") || text.contains("已完結")
    {
        MangaStatus::Completed
    } else if text == "0" || text.contains("连载") || text.contains("連載") {
        MangaStatus::Ongoing
    } else {
        MangaStatus::Unknown
    }
}

pub(crate) fn value_after_labels(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        if let Some((_, value)) = text.split_once(label) {
            let value = value.split('/').next().unwrap_or(value).trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(crate) fn split_tags(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| tag.trim().to_string())
        .collect()
}

pub(crate) fn push_unique(entries: &mut Vec<Manga>, manga: Manga) {
    if !entries.iter().any(|entry| entry.key == manga.key) {
        entries.push(manga);
    }
}

pub(crate) fn push_unique_string(entries: &mut Vec<String>, value: String) {
    if !entries.iter().any(|entry| entry == &value) {
        entries.push(value);
    }
}

pub(crate) fn encode_gbk_component(input: &str) -> String {
    let (encoded, _, _) = GBK.encode(input);
    encode_component(encoded.as_ref())
}

pub(crate) fn encode_component(input: &[u8]) -> String {
    let mut result = String::new();
    for byte in input {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~') {
            result.push(*byte as char);
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

pub(crate) fn normalize_space(text: &str) -> String {
    let mut result = String::new();
    for part in text.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        result.push_str(part);
    }
    result
}

pub(crate) fn normalized_img(element: &Element, base_url: &str) -> Option<String> {
    let raw = element.attr("src").unwrap_or_default();
    if raw.trim().is_empty() {
        return None;
    }
    Some(normalize_image_url(&raw, element.attr("abs:src"), base_url))
}

pub(crate) fn normalize_image_url(raw: &str, abs: Option<String>, base_url: &str) -> String {
    let mut src = raw.trim().to_string();
    if src.starts_with("//") {
        src = format!("https:{src}");
    } else if src.starts_with("/image/") {
        src = format!("{IMG_URL}{src}");
    } else if src.starts_with('/') {
        src = format!("{PIC_URL}{src}");
    } else if !src.starts_with("http") {
        src = abs.unwrap_or_else(|| resolve_url(&src, base_url));
    }

    if src.starts_with("http://") {
        src = src.replacen("http://", "https://", 1);
    }
    src.replace("https://pic.777743.xyz", PIC_URL)
}

pub(crate) fn resolve_url(href: &str, base_url: &str) -> String {
    if href.starts_with("http") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("{BASE_URL}{href}")
    } else {
        let base = base_url
            .rsplit_once('/')
            .map(|(prefix, _)| prefix)
            .unwrap_or(BASE_URL);
        format!("{base}/{href}")
    }
}

pub(crate) fn extract_manga_key(input: &str) -> Option<String> {
    let value = input.trim();
    if value.chars().all(|ch| ch.is_ascii_digit()) && !value.is_empty() {
        return Some(value.to_string());
    }
    if let Some(rest) = value.split("/book/").nth(1)
        && let Some(key) = rest.split(".htm").next()
        && key.chars().all(|ch| ch.is_ascii_digit())
    {
        return Some(key.to_string());
    }
    if let Some(rest) = value.split("bid=").nth(1) {
        let key = rest.split('&').next().unwrap_or(rest);
        if key.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(key.to_string());
        }
    }
    if let Some(rest) = value.split("aid=").nth(1) {
        let key = rest.split('&').next().unwrap_or(rest);
        if key.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(key.to_string());
        }
    }
    if let Some(rest) = value.split("/novel/").nth(1) {
        let mut parts = rest.split('/');
        let _group = parts.next();
        if let Some(key) = parts.next()
            && key.chars().all(|ch| ch.is_ascii_digit())
        {
            return Some(key.to_string());
        }
    }
    None
}

pub(crate) fn extract_chapter_keys(input: &str) -> Option<(String, String)> {
    let rest = input.split("/novel/").nth(1)?;
    let mut parts = rest.split('/');
    let _group = parts.next()?;
    let manga_key = parts.next()?.to_string();
    let chapter = parts.next()?.split(".htm").next()?.to_string();
    if manga_key.is_empty() || chapter.is_empty() {
        None
    } else {
        Some((manga_key, chapter))
    }
}

pub(crate) fn extract_chapter_key(href: &str) -> String {
    if let Some(rest) = href.split("cid=").nth(1) {
        return rest.split('&').next().unwrap_or(rest).to_string();
    }
    href.split('/')
        .next_back()
        .unwrap_or(href)
        .split(".htm")
        .next()
        .unwrap_or_default()
        .to_string()
}
