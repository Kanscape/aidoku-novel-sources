use aidoku::{
    Result,
    alloc::{String, string::ToString},
    imports::{
        defaults::defaults_get_map,
        html::{Document, Html},
        js::WebView,
        net::{Request, Response},
    },
    prelude::*,
};
use encoding_rs::GBK;

use crate::consts::{BASE_URL, REQUEST_BASE_URL, USER_AGENT};

pub(crate) fn request_html_without_webview(url: &str) -> Result<Document> {
    let response = request(url)?;
    let status = response.status_code();
    let data = response.get_data()?;
    let (decoded, _, _) = GBK.decode(&data);
    let html = decoded.into_owned();

    if status == 403
        || response
            .get_header("cf-mitigated")
            .as_deref()
            .is_some_and(|value| value.contains("challenge"))
        || looks_like_cloudflare(&html)
        || looks_like_login_page(&html)
    {
        bail!("Tags 页面不可直接读取");
    }

    Ok(Html::parse_with_url(html.as_bytes(), url)?)
}

pub(crate) fn request_html(url: &str) -> Result<Document> {
    let response = request(url)?;
    let status = response.status_code();
    let data = response.get_data()?;
    let (decoded, _, _) = GBK.decode(&data);
    let html = decoded.into_owned();

    if status == 403
        || response
            .get_header("cf-mitigated")
            .as_deref()
            .is_some_and(|value| value.contains("challenge"))
        || looks_like_cloudflare(&html)
    {
        return request_html_webview(url).map_err(|_| error!("Wenku8 返回 Cloudflare 验证"));
    }
    if looks_like_login_page(&html) {
        if let Ok(doc) = request_html_webview(url) {
            return Ok(doc);
        }
        bail!("Wenku8 要求登录");
    }

    Ok(Html::parse_with_url(html.as_bytes(), url)?)
}

pub(crate) fn request(url: &str) -> Result<Response> {
    let cookie = cookie_header();
    let request_url = append_charset(&network_url(url));
    let mut request = Request::get(request_url)?
        .header("User-Agent", USER_AGENT)
        .header("Referer", BASE_URL)
        .timeout(20.0);
    if !cookie.is_empty() {
        request = request.header("Cookie", cookie.as_str());
    }
    Ok(request.send()?)
}

pub(crate) fn request_html_webview(url: &str) -> Result<Document> {
    let cookie = cookie_header();
    let request_url = append_charset(&network_url(url));
    let mut request = Request::get(request_url)?
        .header("User-Agent", USER_AGENT)
        .header("Referer", BASE_URL)
        .timeout(20.0);
    if !cookie.is_empty() {
        request = request.header("Cookie", cookie.as_str());
    }

    let webview = WebView::new();
    webview.load_blocking(request)?;
    let html = webview.eval("document.documentElement.outerHTML")?;

    if looks_like_cloudflare(&html) {
        bail!("Wenku8 返回 Cloudflare 验证");
    }
    if looks_like_login_page(&html) {
        bail!("Wenku8 要求登录");
    }

    Ok(Html::parse_with_url(html.as_bytes(), url)?)
}

pub(crate) fn cookie_header() -> String {
    let cookies = defaults_get_map("login").unwrap_or_default();

    let mut header = String::new();
    for (key, value) in cookies {
        if !header.is_empty() {
            header.push_str("; ");
        }
        header.push_str(&key);
        header.push('=');
        header.push_str(&value);
    }
    header
}

pub(crate) fn has_auth_cookie(cookies: &aidoku::HashMap<String, String>) -> bool {
    cookies
        .get("jieqiUserInfo")
        .is_some_and(|value| !value.trim().is_empty())
        || cookies
            .get("jieqiVisitInfo")
            .is_some_and(|value| value.contains("jieqiUserId"))
        || cookies
            .get("PHPSESSID")
            .is_some_and(|value| !value.trim().is_empty())
}

pub(crate) fn append_charset(url: &str) -> String {
    if url.contains("charset=") {
        url.to_string()
    } else if url.contains('?') {
        format!("{url}&charset=gbk")
    } else {
        format!("{url}?charset=gbk")
    }
}

pub(crate) fn network_url(url: &str) -> String {
    if let Some(path) = url.strip_prefix(BASE_URL) {
        format!("{REQUEST_BASE_URL}{path}")
    } else {
        url.to_string()
    }
}

pub(crate) fn looks_like_login_page(html: &str) -> bool {
    looks_like_login_form(html) || looks_like_login_notice(html)
}

pub(crate) fn looks_like_login_form(html: &str) -> bool {
    html.contains("jieqi_username")
        || html.contains("jieqi_password")
        || (html.contains("login.php") && html.contains("name=\"username\""))
        || (html.contains("login.php") && html.contains("name=\"password\""))
}

pub(crate) fn looks_like_login_notice(html: &str) -> bool {
    html.contains("请先登录")
        || html.contains("請先登錄")
        || html.contains("请先登陆")
        || html.contains("您尚未登录")
}

pub(crate) fn looks_like_cloudflare(html: &str) -> bool {
    html.contains("cf-mitigated")
        || html.contains("window._cf_chl_opt")
        || html.contains("Just a moment")
        || html.contains("Attention Required! | Cloudflare")
}
