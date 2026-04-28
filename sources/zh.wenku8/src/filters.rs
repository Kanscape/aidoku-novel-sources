use aidoku::{
    Filter, FilterValue, MangaPageResult, Result, SelectFilter,
    alloc::{String, Vec, borrow::Cow, string::ToString, vec},
    imports::html::Document,
    prelude::*,
};

use crate::{
    consts::{BASE_URL, KNOWN_TAGS},
    listing::{list_page, parse_manga_list},
    net::{request_html, request_html_without_webview},
    util::{decode_basic_entities, encode_gbk_component, normalize_space, push_unique_string},
};

pub(crate) fn parse_search_filters(
    filters: Vec<FilterValue>,
) -> (Option<String>, Option<String>, &'static str) {
    let mut author = None;
    let mut tag = None;
    let mut search_type = "articlename";

    for filter in filters {
        match filter {
            FilterValue::Text { id, value } if id == "author" && !value.trim().is_empty() => {
                author = Some(value);
            }
            FilterValue::Select { id, value }
                if id == "search_type" && value.trim() == "author" =>
            {
                search_type = "author";
            }
            FilterValue::Select { id, value } if is_tag_filter(&id) && !value.trim().is_empty() => {
                tag = Some(value);
            }
            FilterValue::MultiSelect { id, included, .. } if is_tag_filter(&id) => {
                tag = included.into_iter().find(|value| !value.trim().is_empty());
            }
            _ => {}
        }
    }

    (author, tag, search_type)
}

pub(crate) fn is_tag_filter(id: &str) -> bool {
    matches!(id, "genre" | "tag" | "tags" | "Tags")
}

pub(crate) fn tag_page(tag: &str, page: i32) -> Result<MangaPageResult> {
    let tag = tag.trim();
    if tag.is_empty() {
        return list_page("all", page);
    }
    let encoded_tag = encode_gbk_component(tag);
    let url = format!("{BASE_URL}/modules/article/tags.php?t={encoded_tag}&page={page}");
    let doc = request_html(&url)?;
    parse_manga_list(&doc, page)
}

pub(crate) fn tags_filter() -> Filter {
    let tags = match fetch_tags() {
        Ok(tags) if !tags.is_empty() => tags,
        Ok(_) => known_tags(),
        Err(err) => {
            println!("[Wenku8] tags filter uses bundled list: {err:?}");
            known_tags()
        }
    };

    let mut options: Vec<Cow<'static, str>> = vec!["全部".into()];
    let mut ids: Vec<Cow<'static, str>> = vec!["".into()];
    for tag in tags {
        options.push(Cow::Owned(tag.clone()));
        ids.push(Cow::Owned(tag));
    }

    SelectFilter {
        id: "genre".into(),
        title: Some("标签".into()),
        is_genre: true,
        uses_tag_style: true,
        options,
        ids: Some(ids),
        default: Some("".into()),
        ..Default::default()
    }
    .into()
}

pub(crate) fn fetch_tags() -> Result<Vec<String>> {
    let doc = request_html_without_webview(&format!("{BASE_URL}/modules/article/tags.php"))?;
    let mut tags = Vec::new();
    collect_tag_links(&doc, "a[href*='tags.php?t=']", &mut tags);
    collect_tag_links(&doc, "a[href*='tags.php?tag=']", &mut tags);
    collect_tag_links(&doc, "#content a[href*='t=']", &mut tags);

    if tags.is_empty() {
        bail!("Tags 解析为空");
    }
    Ok(tags)
}

pub(crate) fn collect_tag_links(doc: &Document, selector: &str, tags: &mut Vec<String>) {
    if let Some(links) = doc.select(selector) {
        for link in links {
            let href = link.attr("href").unwrap_or_default();
            if !href.contains("tags.php") && !href.contains("t=") {
                continue;
            }
            let Some(text) = link.text() else {
                continue;
            };
            let tag = normalize_space(&decode_basic_entities(&text));
            if tag.is_empty() || tag.contains("Tags") || tag.contains("推荐") {
                continue;
            }
            push_unique_string(tags, tag);
        }
    }
}

pub(crate) fn known_tags() -> Vec<String> {
    KNOWN_TAGS.iter().map(|tag| (*tag).to_string()).collect()
}
