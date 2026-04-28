use aidoku::{
    ContentRating, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, Manga, Result,
    Viewer,
    alloc::{String, Vec, string::ToString},
    imports::html::{Document, Element},
    prelude::*,
};

use crate::{
    consts::BASE_URL,
    util::{extract_manga_key, mewx_cover_url, normalize_space, normalized_img, push_unique},
};

pub(crate) fn parse_home(doc: &Document) -> Result<HomeLayout> {
    let mut components = Vec::new();

    if let Some(blocks) = doc.select("#centers .block") {
        for block in blocks {
            if let Some(component) = parse_home_book_component(&block) {
                components.push(component);
            }
        }
    }
    if let Some(component) = parse_home_promote_component(doc) {
        components.push(component);
    }

    if components.is_empty()
        && let Some(tables) = doc.select("#content table")
    {
        for table in tables {
            if let Some(component) = parse_home_book_component(&table) {
                components.push(component);
            }
        }
    }

    if components.is_empty()
        && let Some(blocks) =
            doc.select("#centers > div, #content > div, #content .block, #content .blockcontent")
    {
        for block in blocks {
            if let Some(component) = parse_home_book_component(&block) {
                components.push(component);
            }
        }
    }

    dedupe_home_components(&mut components);
    if components.is_empty() {
        bail!("首页解析为空");
    }
    Ok(HomeLayout { components })
}

pub(crate) fn parse_home_promote_component(doc: &Document) -> Option<HomeComponent> {
    let selectors = [
        "#centerl .block",
        "#centerm .block",
        "#content .block",
        "#centers .block",
        ".block",
        "div.main",
    ];
    for selector in selectors {
        if let Some(sections) = doc.select(selector) {
            for section in sections {
                let Some(title) = home_section_title(&section) else {
                    continue;
                };
                if !is_promote_home_title(&title) {
                    continue;
                }
                let entries = home_section_links(&section);
                if entries.is_empty() {
                    continue;
                }
                return Some(HomeComponent {
                    title: Some(title),
                    subtitle: None,
                    value: HomeComponentValue::Scroller {
                        entries,
                        listing: None,
                    },
                });
            }
        }
    }
    None
}

pub(crate) fn parse_home_book_component(section: &Element) -> Option<HomeComponent> {
    let title = home_section_title(section)?;
    if title.contains("用户登录") || title.contains("公告") || is_recent_update_home_title(&title)
    {
        return None;
    }
    let entries = home_section_links(section);
    if entries.is_empty() {
        return None;
    }
    Some(HomeComponent {
        title: Some(title.clone()),
        subtitle: None,
        value: HomeComponentValue::Scroller {
            entries,
            listing: home_listing_for_title(&title),
        },
    })
}

pub(crate) fn home_section_title(section: &Element) -> Option<String> {
    let selectors = [
        "caption",
        "th",
        "td[colspan]",
        ".title",
        ".blocktitle",
        ".blockheader",
        ".head",
    ];
    for selector in selectors {
        if let Some(title) = section
            .select_first(selector)
            .and_then(|el| el.text())
            .and_then(|text| clean_home_title(&text))
        {
            return Some(title);
        }
    }
    None
}

pub(crate) fn clean_home_title(text: &str) -> Option<String> {
    let title = normalize_space(text)
        .split(['(', '（'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    if title.contains("用户名")
        || title.contains("密码")
        || title.contains("新用户注册")
        || title.contains("取回密码")
    {
        return None;
    }
    Some(title)
}

pub(crate) fn is_recent_update_home_title(title: &str) -> bool {
    title.contains("最近更新")
}

pub(crate) fn is_promote_home_title(title: &str) -> bool {
    title.contains("文库轻小说推广区") || title.contains("文庫輕小說推廣區")
}

pub(crate) fn home_section_links(section: &Element) -> Vec<Link> {
    let mut entries = Vec::new();
    let selectors = [
        ".blockcontent > div > div",
        ".blockcontent div > div",
        "td > div",
        "li",
        ".item",
        ".book",
        "td",
    ];
    for selector in selectors {
        if let Some(items) = section.select(selector) {
            for item in items {
                if let Some(manga) = parse_home_card(&item) {
                    push_unique(&mut entries, manga);
                }
            }
        }
        if !entries.is_empty() {
            break;
        }
    }

    if entries.is_empty()
        && let Some(links) = section.select("a[href*='/book/'], a[href*='book/']")
    {
        for link in links {
            if let Some(manga) = parse_home_link(&link) {
                push_unique(&mut entries, manga);
            }
        }
    }

    entries.into_iter().take(30).map(Link::from).collect()
}

pub(crate) fn parse_home_card(item: &Element) -> Option<Manga> {
    let link = item
        .select_first("a[href*='/book/']")
        .or_else(|| item.select_first("a[href*='book/']"))?;
    let href = link.attr("href")?;
    let key = extract_manga_key(&href)?;
    let title = home_card_title(item, &key)?;
    Some(Manga {
        key: key.clone(),
        title,
        cover: item
            .select_first("img")
            .and_then(|el| normalized_img(&el, BASE_URL))
            .or_else(|| Some(mewx_cover_url(&key))),
        url: Some(format!("{BASE_URL}/book/{key}.htm")),
        content_rating: ContentRating::Safe,
        viewer: Viewer::Vertical,
        ..Default::default()
    })
}

pub(crate) fn parse_home_link(link: &Element) -> Option<Manga> {
    let href = link.attr("href")?;
    let key = extract_manga_key(&href)?;
    let title = link
        .attr("title")
        .and_then(|text| clean_home_manga_title(&text))
        .or_else(|| link.text().and_then(|text| clean_home_manga_title(&text)))
        .or_else(|| {
            link.parent()
                .and_then(|parent| home_card_title(&parent, &key))
        })?;
    let cover = link
        .select_first("img")
        .and_then(|el| normalized_img(&el, BASE_URL))
        .or_else(|| {
            link.parent()
                .and_then(|parent| parent.select_first("img"))
                .and_then(|el| normalized_img(&el, BASE_URL))
        })
        .or_else(|| Some(mewx_cover_url(&key)));
    Some(Manga {
        key: key.clone(),
        title,
        cover,
        url: Some(format!("{BASE_URL}/book/{key}.htm")),
        content_rating: ContentRating::Safe,
        viewer: Viewer::Vertical,
        ..Default::default()
    })
}

pub(crate) fn home_card_title(item: &Element, key: &str) -> Option<String> {
    if let Some(mut links) = item.select("a[href*='/book/'], a[href*='book/']") {
        for link in links.by_ref() {
            let href = link.attr("href").unwrap_or_default();
            if extract_manga_key(&href).as_deref() != Some(key) {
                continue;
            }
            if let Some(title) = link
                .attr("title")
                .and_then(|text| clean_home_manga_title(&text))
                .or_else(|| link.text().and_then(|text| clean_home_manga_title(&text)))
            {
                return Some(title);
            }
        }
    }
    item.select_first("img")
        .and_then(|img| {
            img.attr("alt")
                .or_else(|| img.attr("title"))
                .and_then(|text| clean_home_manga_title(&text))
        })
        .or_else(|| item.text().and_then(|text| clean_home_manga_title(&text)))
}

pub(crate) fn clean_home_manga_title(text: &str) -> Option<String> {
    let title = normalize_space(text);
    if title.is_empty()
        || title == "查看"
        || title == "更多"
        || title.contains("TOP榜")
        || title.contains("轻小说文库")
    {
        None
    } else {
        Some(title)
    }
}

pub(crate) fn home_listing_for_title(title: &str) -> Option<Listing> {
    let (id, name) = if title.contains("新书") {
        ("new", "新书一览")
    } else if title.contains("更新") {
        ("updated", "今日更新")
    } else if title.contains("动画") || title.contains("新番") {
        ("anime", "动画化作品")
    } else if title.contains("完结") || title.contains("完本") {
        ("completed", "完结全本")
    } else if title.contains("热门") || title.contains("风云") || title.contains("推荐") {
        ("popular", "热门轻小说")
    } else {
        return None;
    };
    Some(Listing {
        id: id.into(),
        name: name.into(),
        ..Default::default()
    })
}

pub(crate) fn dedupe_home_components(components: &mut Vec<HomeComponent>) {
    let mut result = Vec::new();
    for component in components.drain(..) {
        let title = component.title.clone().unwrap_or_default();
        if result
            .iter()
            .any(|entry: &HomeComponent| entry.title.as_deref() == Some(title.as_str()))
        {
            continue;
        }
        result.push(component);
    }
    *components = result;
}
