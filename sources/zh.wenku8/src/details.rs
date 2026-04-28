use aidoku::{
    ContentRating, Manga, MangaStatus, Result, Viewer,
    alloc::{String, Vec, vec},
    imports::html::{Document, Element},
    prelude::*,
};

use crate::{
    consts::BASE_URL,
    util::{
        clean_description, decode_basic_entities, normalized_img, parse_status, split_tags,
        value_after_labels,
    },
};

pub(crate) fn parse_details(doc: &Document, manga: &mut Manga) -> Result<()> {
    let content = doc
        .select_first("#content")
        .ok_or_else(|| error!("未找到书籍详情"))?;

    manga.title = content
        .select_first("span b")
        .and_then(|el| el.text())
        .unwrap_or_else(|| manga.title.clone());
    manga.cover = content
        .select_first("img")
        .and_then(|el| normalized_img(&el, BASE_URL));
    manga.url = Some(format!("{BASE_URL}/book/{}.htm", manga.key));
    manga.content_rating = ContentRating::Safe;
    manga.viewer = Viewer::Vertical;

    let mut author = None;
    let mut status = MangaStatus::Unknown;
    if let Some(cells) = content.select("td") {
        for cell in cells {
            let text = cell.text().unwrap_or_default();
            if author.is_none() {
                author = value_after_labels(&text, &["小说作者：", "小說作者："]);
            }
            if let Some(value) = value_after_labels(&text, &["文章状态：", "文章狀態："])
            {
                status = parse_status(&value);
            }
        }
    }
    manga.authors = author.map(|value| vec![value]);
    manga.artists = manga.authors.clone();
    manga.status = status;

    if let Some(details) = detail_text_cell(doc, &content) {
        manga.description = detail_description(&details);
        manga.tags = detail_tags(&details);
    }

    Ok(())
}

pub(crate) fn detail_text_cell(doc: &Document, content: &Element) -> Option<Element> {
    doc.select_first("#content > div > table:nth-of-type(2) tr > td:nth-of-type(2)")
        .or_else(|| content.select_first("div > table:nth-of-type(2) tr > td:nth-of-type(2)"))
        .or_else(|| {
            content
                .select("div > table")
                .and_then(|tables| tables.get(1))
                .and_then(|table| table.select_first("tr > td:nth-of-type(2)"))
        })
        .or_else(|| {
            content
                .select("table")
                .and_then(|tables| tables.get(2))
                .and_then(|table| table.select_first("tr > td:nth-of-type(2), td:nth-of-type(2)"))
        })
}

pub(crate) fn detail_tags(details: &Element) -> Option<Vec<String>> {
    let spans = details.select("span")?;
    for span in spans {
        let text = span.text().unwrap_or_default();
        if let Some(value) = value_after_labels(&text, &["作品Tags：", "作品Tags:"]) {
            return Some(split_tags(&value));
        }
    }
    None
}

pub(crate) fn detail_description(details: &Element) -> Option<String> {
    let spans = details.select("span")?;
    if let Some(span) = spans.get(5)
        && let Some(description) = description_from_span(&span)
    {
        return Some(description);
    }

    for index in (0..spans.size()).rev() {
        if let Some(span) = spans.get(index)
            && let Some(description) = description_from_span(&span)
        {
            return Some(description);
        }
    }
    None
}

pub(crate) fn description_from_span(span: &Element) -> Option<String> {
    let text = if let Some(html) = span.html() {
        if html.contains("<br") || html.contains("<BR") {
            text_from_html(&html)
        } else {
            span.untrimmed_text()
                .or_else(|| span.text())
                .unwrap_or_default()
        }
    } else {
        span.untrimmed_text()
            .or_else(|| span.text())
            .unwrap_or_default()
    };
    clean_description(text)
}

pub(crate) fn text_from_html(html: &str) -> String {
    let mut result = String::new();
    let bytes = html.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'<' {
            if is_br_tag(&html[index..]) && !result.ends_with('\n') {
                result.push('\n');
            }
            while index < bytes.len() && bytes[index] != b'>' {
                index += 1;
            }
            index += 1;
            continue;
        }

        if let Some(ch) = html[index..].chars().next() {
            result.push(ch);
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    decode_basic_entities(&result)
}

pub(crate) fn is_br_tag(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'<' {
        return false;
    }
    if !bytes[1].eq_ignore_ascii_case(&b'b') || !bytes[2].eq_ignore_ascii_case(&b'r') {
        return false;
    }
    bytes
        .get(3)
        .is_some_and(|byte| matches!(*byte, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n'))
}
