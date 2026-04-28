use aidoku::{
    ContentRating, Manga, MangaPageResult, MangaStatus, Result, Viewer,
    alloc::{String, Vec, string::ToString, vec},
    imports::{net::Request, std::current_date},
    prelude::*,
};

use crate::{
    consts::{BASE_URL, MEWX_APP_VERSION, MEWX_RELAY_URL, MEWX_USER_AGENT, SEARCH_PAGE_SIZE},
    util::{
        clean_description, decode_basic_entities, encode_component, encode_gbk_component,
        mewx_cover_url, parse_status, push_unique, split_tags,
    },
};

pub(crate) fn search_page(search_type: &str, query: &str, page: i32) -> Result<MangaPageResult> {
    let gbk_encoded = encode_gbk_component(query);
    let relay_query = encode_component(gbk_encoded.as_bytes());
    let request = format!("action=search&searchtype={search_type}&searchkey={relay_query}");
    let xml = mewx_api(&request)?;
    parse_mewx_search_result(&xml, page)
}

pub(crate) fn mewx_api(request: &str) -> Result<String> {
    let body = mewx_form_body(request);
    let response = Request::post(MEWX_RELAY_URL)?
        .header("User-Agent", MEWX_USER_AGENT)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .timeout(20.0)
        .send()?;
    let status = response.status_code();
    let text = response.get_string()?;
    if !(200..300).contains(&status) {
        bail!("MewX relay 请求失败：HTTP {status}");
    }
    if text.contains("java.net.") {
        bail!("MewX relay 返回网络错误");
    }
    Ok(text)
}

pub(crate) fn mewx_form_body(request: &str) -> String {
    let encoded_request = base64_encode(request.as_bytes());
    let timetoken = current_date() * 1000;
    format!(
        "request={}&timetoken={timetoken}&appver={}",
        encode_component(encoded_request.as_bytes()),
        encode_component(MEWX_APP_VERSION.as_bytes())
    )
}

pub(crate) fn parse_mewx_search_result(xml: &str, page: i32) -> Result<MangaPageResult> {
    let mut entries = Vec::new();
    let mut cursor = 0;

    while let Some(start_offset) = xml[cursor..].find("<item") {
        let start = cursor + start_offset;
        let Some(open_end_offset) = xml[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_offset;
        let open_tag = &xml[start..=open_end];
        let (item_xml, next_cursor) = if open_tag.trim_end().ends_with("/>") {
            (&xml[start..=open_end], open_end + 1)
        } else if let Some(close_offset) = xml[open_end + 1..].find("</item>") {
            let end = open_end + 1 + close_offset + "</item>".len();
            (&xml[start..end], end)
        } else {
            break;
        };

        if let Some(mut manga) = parse_mewx_manga(item_xml, None) {
            if manga.title == manga.key
                && let Ok(metadata) = mewx_book_metadata(&manga.key)
            {
                manga = metadata;
            }
            push_unique(&mut entries, manga);
        }
        cursor = next_cursor;
    }

    let page = if page < 1 { 1 } else { page };
    let start = ((page - 1) as usize).saturating_mul(SEARCH_PAGE_SIZE);
    let total = entries.len();
    if start >= total {
        return Ok(MangaPageResult {
            entries: Vec::new(),
            has_next_page: false,
        });
    }
    let mut end = start + SEARCH_PAGE_SIZE;
    if end > total {
        end = total;
    }

    let mut page_entries = Vec::new();
    for (index, manga) in entries.into_iter().enumerate() {
        if index >= start && index < end {
            page_entries.push(manga);
        }
    }

    Ok(MangaPageResult {
        entries: page_entries,
        has_next_page: end < total,
    })
}

pub(crate) fn mewx_book_metadata(key: &str) -> Result<Manga> {
    let xml = mewx_api(&format!("action=book&do=meta&aid={key}&t=0"))?;
    parse_mewx_manga(&xml, Some(key)).ok_or_else(|| error!("MewX relay 未返回书籍信息：{key}"))
}

pub(crate) fn parse_mewx_manga(xml: &str, fallback_key: Option<&str>) -> Option<Manga> {
    let key = xml_attr(xml, "aid").or_else(|| fallback_key.map(String::from))?;
    if !key.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let title = xml_data_text(xml, "Title").unwrap_or_else(|| key.clone());
    let mut manga = Manga {
        key: key.clone(),
        title,
        cover: Some(mewx_cover_url(&key)),
        url: Some(format!("{BASE_URL}/book/{key}.htm")),
        content_rating: ContentRating::Safe,
        viewer: Viewer::Vertical,
        ..Default::default()
    };

    manga.authors = xml_data_attr(xml, "Author", "value").map(|author| vec![author]);
    manga.artists = manga.authors.clone();
    manga.status = xml_data_attr(xml, "BookStatus", "value")
        .map(|value| parse_status(&value))
        .unwrap_or(MangaStatus::Unknown);
    manga.tags = xml_data_attr(xml, "Tags", "value").map(|value| split_tags(&value));
    manga.description = xml_data_text(xml, "IntroPreview").and_then(clean_description);

    Some(manga)
}

pub(crate) fn xml_data_text(xml: &str, name: &str) -> Option<String> {
    let element = xml_data_element(xml, name)?;
    let open_end = element.find('>')?;
    let close_start = element.rfind("</data>")?;
    Some(xml_text_value(&element[open_end + 1..close_start]))
}

pub(crate) fn xml_data_attr(xml: &str, name: &str, attr: &str) -> Option<String> {
    let element = xml_data_element(xml, name)?;
    let open_end = element.find('>')?;
    xml_attr(&element[..=open_end], attr)
}

pub(crate) fn xml_data_element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let mut cursor = 0;
    while let Some(start_offset) = xml[cursor..].find("<data") {
        let start = cursor + start_offset;
        let open_end = start + xml[start..].find('>')?;
        let open_tag = &xml[start..=open_end];
        if xml_attr(open_tag, "name").as_deref() == Some(name) {
            if open_tag.trim_end().ends_with("/>") {
                return Some(open_tag);
            }
            let close_end = open_end + 1 + xml[open_end + 1..].find("</data>")? + "</data>".len();
            return Some(&xml[start..close_end]);
        }
        cursor = open_end + 1;
    }
    None
}

pub(crate) fn xml_attr(xml: &str, name: &str) -> Option<String> {
    let single_quote = format!("{name}='");
    if let Some(value) = xml_attr_after(xml, &single_quote, '\'') {
        return Some(value);
    }
    let double_quote = format!("{name}=\"");
    xml_attr_after(xml, &double_quote, '"')
}

pub(crate) fn xml_attr_after(xml: &str, marker: &str, quote: char) -> Option<String> {
    let start = xml.find(marker)? + marker.len();
    let value = xml[start..].split(quote).next()?;
    Some(decode_basic_entities(value))
}

pub(crate) fn xml_text_value(raw: &str) -> String {
    let text = raw.trim();
    if let Some(inner) = text
        .strip_prefix("<![CDATA[")
        .and_then(|value| value.strip_suffix("]]>"))
    {
        inner.trim().to_string()
    } else {
        decode_basic_entities(text).trim().to_string()
    }
}

pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut index = 0;
    while index < input.len() {
        let b0 = input[index];
        let b1 = *input.get(index + 1).unwrap_or(&0);
        let b2 = *input.get(index + 2).unwrap_or(&0);

        result.push(TABLE[(b0 >> 2) as usize] as char);
        result.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if index + 1 < input.len() {
            result.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if index + 2 < input.len() {
            result.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            result.push('=');
        }

        index += 3;
    }
    result
}
