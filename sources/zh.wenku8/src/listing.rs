use aidoku::{
    ContentRating, Manga, MangaPageResult, Result, Viewer,
    alloc::{Vec, vec},
    imports::html::{Document, Element},
    prelude::*,
};

use crate::{
    consts::BASE_URL,
    details::parse_details,
    net::request_html,
    util::{extract_manga_key, normalized_img, parse_status, push_unique, value_after_labels},
};

pub(crate) fn list_page(id: &str, page: i32) -> Result<MangaPageResult> {
    let url = match id {
        "all" => format!("{BASE_URL}/modules/article/articlelist.php?page={page}"),
        "completed" => {
            format!("{BASE_URL}/modules/article/articlelist.php?fullflag=1&page={page}")
        }
        "popular" => format!("{BASE_URL}/modules/article/toplist.php?sort=allvisit&page={page}"),
        "updated" => format!("{BASE_URL}/modules/article/toplist.php?sort=lastupdate&page={page}"),
        "new" => format!("{BASE_URL}/modules/article/toplist.php?sort=postdate&page={page}"),
        "anime" => format!("{BASE_URL}/modules/article/toplist.php?sort=anime&page={page}"),
        _ => bail!("未知列表：{id}"),
    };
    let doc = request_html(&url)?;
    parse_manga_list(&doc, page)
}

pub(crate) fn parse_manga_list(doc: &Document, page: i32) -> Result<MangaPageResult> {
    let mut entries = Vec::new();

    if let Some(items) = doc.select("#content div[style*=width]") {
        for item in items {
            if let Some(manga) = parse_manga_card(&item) {
                push_unique(&mut entries, manga);
            }
        }
    }

    if entries.is_empty()
        && let Some(items) = doc.select("#content table td > div")
    {
        for item in items {
            if let Some(manga) = parse_manga_card(&item) {
                push_unique(&mut entries, manga);
            }
        }
    }

    if entries.is_empty()
        && let Some(mut manga) = parse_single_result(doc)
    {
        if manga.title.is_empty() {
            let _ = parse_details(doc, &mut manga);
        }
        entries.push(manga);
    }

    let has_next_page = has_next_page(doc, page);
    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}

pub(crate) fn parse_manga_card(item: &Element) -> Option<Manga> {
    let link = item
        .select_first("a[href*='/book/']")
        .or_else(|| item.select_first("a[href*='book/']"))?;
    let href = link.attr("href")?;
    let key = extract_manga_key(&href)?;
    let title = link
        .attr("title")
        .or_else(|| item.select_first("b a").and_then(|el| el.text()))
        .or_else(|| link.text())
        .unwrap_or_default();
    if title.trim().is_empty() {
        return None;
    }

    let mut manga = Manga {
        key: key.clone(),
        title,
        cover: item
            .select_first("img")
            .and_then(|el| normalized_img(&el, BASE_URL)),
        url: Some(format!("{BASE_URL}/book/{key}.htm")),
        content_rating: ContentRating::Safe,
        viewer: Viewer::Vertical,
        ..Default::default()
    };

    if let Some(info) = item.select("p").and_then(|els| els.text()) {
        manga.authors =
            value_after_labels(&info, &["小说作者：", "作者："]).map(|value| vec![value]);
        manga.status = parse_status(&info);
    }
    Some(manga)
}

pub(crate) fn parse_single_result(doc: &Document) -> Option<Manga> {
    let href = doc
        .select_first("a[href*='/book/']")
        .and_then(|el| el.attr("href"))
        .or_else(|| {
            doc.select_first("a[href*='bid=']")
                .and_then(|el| el.attr("href"))
        })
        .or_else(|| {
            doc.select_first("a[href*='/novel/'][href$='/index.htm']")
                .and_then(|el| el.attr("href"))
        })?;
    let key = extract_manga_key(&href)?;
    let title = doc
        .select_first("#content span b")
        .and_then(|el| el.text())
        .unwrap_or_default();
    Some(Manga {
        key: key.clone(),
        title,
        cover: doc
            .select_first("#content img")
            .and_then(|el| normalized_img(&el, BASE_URL)),
        url: Some(format!("{BASE_URL}/book/{key}.htm")),
        content_rating: ContentRating::Safe,
        viewer: Viewer::Vertical,
        ..Default::default()
    })
}

pub(crate) fn has_next_page(doc: &Document, page: i32) -> bool {
    if doc.select_first("#pagelink a.next, a.next").is_some() {
        return true;
    }
    doc.select_first(".last")
        .and_then(|el| el.text())
        .and_then(|text| text.trim().parse::<i32>().ok())
        .is_some_and(|last| last > page)
}
