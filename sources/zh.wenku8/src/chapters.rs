use aidoku::{
    Chapter, Result,
    alloc::{String, Vec, string::ToString},
    imports::html::Element,
    prelude::*,
};

use crate::{
    consts::BASE_URL,
    net::request_html,
    util::{extract_chapter_key, normalized_img, resolve_url},
};

pub(crate) fn parse_chapters(manga_key: &str) -> Result<Vec<Chapter>> {
    let url = catalog_url(manga_key)?;
    let doc = request_html(&url)?;
    let cells = doc
        .select("table.css td")
        .or_else(|| doc.select("td"))
        .ok_or_else(|| error!("未找到目录"))?;

    let mut chapters = Vec::new();
    let mut current_volume_number = 0.0;
    let mut fallback_volume_number = 0.0;
    let mut chapter_number_in_volume = 0.0;

    for cell in cells {
        if cell
            .attr("class")
            .is_some_and(|class| class.contains("vcss"))
        {
            fallback_volume_number += 1.0;
            current_volume_number = volume_number_from_cell(&cell, fallback_volume_number);
            chapter_number_in_volume = 0.0;
            continue;
        }

        if !cell
            .attr("class")
            .is_some_and(|class| class.contains("ccss"))
        {
            continue;
        }

        let Some(links) = cell.select("a") else {
            continue;
        };
        for link in links {
            let raw_title = link.text().unwrap_or_default();
            let href = link.attr("href").unwrap_or_default();
            let key = extract_chapter_key(&href);
            if key.is_empty() || raw_title.trim().is_empty() {
                continue;
            }
            chapter_number_in_volume += 1.0;
            let title = clean_catalog_chapter_title(&raw_title);
            chapters.push(Chapter {
                key: key.clone(),
                title,
                chapter_number: Some(chapter_number_in_volume),
                volume_number: if current_volume_number > 0.0 {
                    Some(current_volume_number)
                } else {
                    None
                },
                url: Some(resolve_url(&href, &url)),
                language: Some("zh".into()),
                ..Default::default()
            });
        }
    }

    if chapters.is_empty() {
        bail!("目录为空");
    }
    chapters.reverse();
    Ok(chapters)
}

pub(crate) fn volume_number_from_cell(cell: &Element, fallback: f32) -> f32 {
    cell.text()
        .and_then(|text| parse_number_between(&text, '第', &['卷']))
        .unwrap_or(fallback)
}

pub(crate) fn clean_catalog_chapter_title(raw: &str) -> Option<String> {
    let title = raw.trim();
    if let Some(rest) = title.strip_prefix('【').and_then(|text| {
        text.split_once('】')
            .and_then(|(label, rest)| is_chapter_label(label).then_some(rest))
    }) {
        let cleaned = trim_title_separator(rest);
        return (!cleaned.is_empty()).then(|| cleaned.to_string());
    }

    if let Some((_number, rest)) = split_numbered_title(title, &['章', '节', '節', '话', '話'])
    {
        let cleaned = trim_title_separator(rest);
        return (!cleaned.is_empty()).then(|| cleaned.to_string());
    }
    (!title.is_empty()).then(|| title.to_string())
}

pub(crate) fn is_chapter_label(label: &str) -> bool {
    split_numbered_title(label.trim(), &['章', '节', '節', '话', '話']).is_some()
        || matches!(
            label.trim(),
            "序章"
                | "终章"
                | "終章"
                | "最终话"
                | "最終話"
                | "尾声"
                | "尾聲"
                | "后记"
                | "後記"
                | "插图"
                | "插圖"
        )
}

pub(crate) fn split_numbered_title<'a>(
    title: &'a str,
    suffixes: &[char],
) -> Option<(f32, &'a str)> {
    let after_prefix = title.strip_prefix('第')?;
    for (index, ch) in after_prefix.char_indices() {
        if suffixes.contains(&ch) {
            let number_text = &after_prefix[..index];
            let number = parse_number_text(number_text)?;
            let rest = &after_prefix[index + ch.len_utf8()..];
            return Some((number, rest));
        }
    }
    None
}

pub(crate) fn parse_number_between(text: &str, prefix: char, suffixes: &[char]) -> Option<f32> {
    let start = text.find(prefix)?;
    let after_prefix = &text[start + prefix.len_utf8()..];
    for (index, ch) in after_prefix.char_indices() {
        if suffixes.contains(&ch) {
            return parse_number_text(&after_prefix[..index]);
        }
    }
    None
}

pub(crate) fn parse_number_text(text: &str) -> Option<f32> {
    parse_numeric(text).or_else(|| parse_chinese_number(text))
}

pub(crate) fn parse_numeric(text: &str) -> Option<f32> {
    let mut normalized = String::new();
    for ch in text.trim().chars() {
        match ch {
            '0'..='9' | '.' => normalized.push(ch),
            '０'..='９' => {
                let digit = (ch as u32) - ('０' as u32);
                normalized.push(char::from(b'0' + digit as u8));
            }
            _ if ch.is_whitespace() => {}
            _ => return None,
        }
    }
    if normalized.is_empty() {
        None
    } else {
        normalized.parse::<f32>().ok()
    }
}

pub(crate) fn parse_chinese_number(text: &str) -> Option<f32> {
    let mut total = 0;
    let mut current = 0;
    let mut matched = false;

    for ch in text.trim().chars() {
        if ch.is_whitespace() {
            continue;
        }
        if let Some(value) = chinese_digit(ch) {
            current = value;
            matched = true;
        } else if let Some(unit) = chinese_unit(ch) {
            if current == 0 {
                current = 1;
            }
            total += current * unit;
            current = 0;
            matched = true;
        } else {
            return None;
        }
    }

    if matched {
        Some((total + current) as f32)
    } else {
        None
    }
}

pub(crate) fn chinese_digit(ch: char) -> Option<i32> {
    match ch {
        '零' | '〇' => Some(0),
        '一' => Some(1),
        '二' | '两' | '兩' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        _ => None,
    }
}

pub(crate) fn chinese_unit(ch: char) -> Option<i32> {
    match ch {
        '十' => Some(10),
        '百' => Some(100),
        '千' => Some(1000),
        _ => None,
    }
}

pub(crate) fn trim_title_separator(text: &str) -> &str {
    text.trim()
        .trim_start_matches([':', '：', '-', '－', '—', ' ', '\t', '\u{3000}'])
        .trim()
}

pub(crate) fn image_urls(content: &Element, base_url: &str) -> Vec<String> {
    let mut images = Vec::new();
    if let Some(items) = content.select("img") {
        for item in items {
            if let Some(src) = normalized_img(&item, base_url) {
                images.push(src);
            }
        }
    }
    images
}

pub(crate) fn clean_chapter_text(raw: String) -> String {
    let normalized = raw
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{a0}', " ");
    let mut result = String::new();
    for line in normalized.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !result.is_empty() {
            result.push_str("\n\n");
        }
        result.push_str(line);
    }
    result
}

pub(crate) fn catalog_url(manga_key: &str) -> Result<String> {
    let id = manga_key
        .parse::<i32>()
        .map_err(|_| error!("书籍 ID 无效：{manga_key}"))?;
    Ok(format!(
        "{BASE_URL}/novel/{}/{manga_key}/index.htm",
        id / 1000
    ))
}

pub(crate) fn chapter_url(manga_key: &str, chapter_key: &str) -> String {
    let group = manga_key.parse::<i32>().unwrap_or_default() / 1000;
    format!("{BASE_URL}/novel/{group}/{manga_key}/{chapter_key}.htm")
}
