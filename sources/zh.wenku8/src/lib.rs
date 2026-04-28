#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, HashMap, Home,
    HomeLayout, ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page,
    PageContent, Result, Source, WebLoginHandler,
    alloc::{String, Vec, vec},
    imports::net::{Request, TimeUnit, set_rate_limit},
    prelude::*,
};

mod chapters;
mod consts;
mod details;
mod filters;
mod home;
mod listing;
mod mewx;
mod net;
mod util;

use chapters::{chapter_url, clean_chapter_text, image_urls, parse_chapters};
use consts::{BASE_URL, USER_AGENT};
use details::parse_details;
use filters::{parse_search_filters, tag_page, tags_filter};
use home::parse_home;
use listing::list_page;
use mewx::search_page;
use net::{cookie_header, cookie_names, has_auth_cookie, request_html};
use util::{extract_chapter_keys, extract_manga_key};

struct Wenku8;

impl Source for Wenku8 {
    fn new() -> Self {
        set_rate_limit(12, 60, TimeUnit::Seconds);
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        if let Some(key) = query.as_deref().and_then(extract_manga_key) {
            let manga = self.get_manga_update(
                Manga {
                    key,
                    ..Default::default()
                },
                true,
                false,
            )?;
            return Ok(MangaPageResult {
                entries: vec![manga],
                has_next_page: false,
            });
        }

        let (author, tag, search_type) = parse_search_filters(filters);
        if let Some(author) = author {
            return search_page("author", &author, page);
        }
        if let Some(tag) = tag {
            return tag_page(&tag, page);
        }

        let query = query.unwrap_or_default();
        if query.trim().is_empty() {
            return list_page("all", page);
        }
        search_page(search_type, &query, page)
    }

    fn get_manga_update(
        &self,
        mut manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        if needs_details {
            let doc = request_html(&format!("{BASE_URL}/book/{}.htm", manga.key))?;
            parse_details(&doc, &mut manga)?;
        }

        if needs_chapters {
            manga.chapters = Some(parse_chapters(&manga.key)?);
        }

        Ok(manga)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let url = chapter
            .url
            .unwrap_or_else(|| chapter_url(&manga.key, &chapter.key));
        let doc = request_html(&url)?;

        let content = doc
            .select_first("#content")
            .ok_or_else(|| error!("未找到章节内容"))?;
        if let Some(items) = content.select("#contentdp, ul#contentdp") {
            items.remove();
        }

        let title = doc
            .select_first("#title")
            .and_then(|el| el.text())
            .unwrap_or_else(|| chapter.title.unwrap_or_default());
        let text = clean_chapter_text(content.untrimmed_text().unwrap_or_default());
        let images = image_urls(&content, &url);

        let mut pages = Vec::new();
        if !text.is_empty() {
            let markdown = if title.is_empty() || text.starts_with(&title) {
                text
            } else {
                format!("# {title}\n\n{text}")
            };
            pages.push(Page {
                content: PageContent::text(markdown),
                ..Default::default()
            });
        }
        for image in images {
            pages.push(Page {
                content: PageContent::url(image),
                ..Default::default()
            });
        }

        if pages.is_empty() {
            bail!("章节没有可读内容");
        }
        Ok(pages)
    }
}

impl ListingProvider for Wenku8 {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        list_page(&listing.id, page)
    }
}

impl Home for Wenku8 {
    fn get_home(&self) -> Result<HomeLayout> {
        let doc = request_html(&format!("{BASE_URL}/index.php"))?;
        parse_home(&doc)
    }
}

impl DynamicFilters for Wenku8 {
    fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
        Ok(vec![tags_filter()])
    }
}

impl ImageRequestProvider for Wenku8 {
    fn get_image_request(
        &self,
        url: String,
        _context: Option<aidoku::PageContext>,
    ) -> Result<Request> {
        let (cookie, _) = cookie_header();
        let mut request = Request::get(url)?
            .header("User-Agent", USER_AGENT)
            .header("Referer", BASE_URL);
        if !cookie.is_empty() {
            request = request.header("Cookie", cookie.as_str());
        }
        Ok(request)
    }
}

impl DeepLinkHandler for Wenku8 {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        if let Some(key) = extract_manga_key(&url) {
            return Ok(Some(DeepLinkResult::Manga { key }));
        }

        if let Some((manga_key, chapter_key)) = extract_chapter_keys(&url) {
            return Ok(Some(DeepLinkResult::Chapter {
                manga_key,
                key: chapter_key,
            }));
        }

        Ok(None)
    }
}

impl WebLoginHandler for Wenku8 {
    fn handle_web_login(&self, key: String, cookies: HashMap<String, String>) -> Result<bool> {
        if key != "login" {
            bail!("无效登录项：{key}");
        }
        println!(
            "[Wenku8] web login cookies: {}; auth={}",
            cookie_names(&cookies),
            has_auth_cookie(&cookies)
        );
        Ok(has_auth_cookie(&cookies))
    }
}

register_source!(
    Wenku8,
    Home,
    ListingProvider,
    DynamicFilters,
    ImageRequestProvider,
    DeepLinkHandler,
    WebLoginHandler
);
