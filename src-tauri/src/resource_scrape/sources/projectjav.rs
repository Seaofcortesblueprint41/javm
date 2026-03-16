//! projectjav.com 数据源解析器
//!
//! 搜索型网站，URL 格式：https://projectjav.com/?searchTerm={CODE}
//! 请求后 reqwest 自动跟随 HTTP 重定向到详情页：/movie/{code}-{id}
//! 详情页结构：
//! - 封面：.movie-detail img[src*="covers"]
//! - 标题：h1
//! - 信息：.row > .col-3（标签） + .col-9（值）的行式布局
//! - 演员：a[href*="/actress/"]
//! - 标签：.badge-info a[href*="/tag/"]

use super::{SearchResult, Source};
use scraper::{Html, Selector};

pub struct ProjectJav;

impl Source for ProjectJav {
    fn name(&self) -> &str {
        "projectjav"
    }

    fn build_url(&self, code: &str) -> String {
        format!("https://projectjav.com/?searchTerm={}", code.to_lowercase())
    }

    /// 从搜索结果页提取精确匹配番号的详情页 URL
    fn extract_detail_url(&self, html: &str, code: &str) -> Option<String> {
        let doc = Html::parse_document(html);
        let code_lower = code.to_lowercase();
        // 匹配 href="/movie/{code}-{id}" 格式的链接
        let prefix = format!("/movie/{}-", code_lower);
        let sel = Selector::parse("a[href]").ok()?;
        for el in doc.select(&sel) {
            let href = el.value().attr("href").unwrap_or("");
            if href.starts_with(&prefix) {
                return Some(format!("https://projectjav.com{}", href));
            }
        }
        None
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = Html::parse_document(html);
        let code_upper = code.to_uppercase();
        let code_lower = code.to_lowercase();

        // 封面图：优先找 img[src*="covers"]，再 fallback
        let cover_url = select_cover_img(&doc)
            .or_else(|| select_attr(&doc, r#"meta[property="og:image"]"#, "content"))
            .or_else(|| find_cover_image(&doc, &code_upper, &code_lower))
            .unwrap_or_default();

        // 标题：h1 标签（详情页格式）
        let raw_title = select_text(&doc, "h1")
            .or_else(|| select_attr(&doc, r#"meta[property="og:title"]"#, "content"))
            .or_else(|| select_text(&doc, "title"))
            .unwrap_or_default();

        // 清理标题：去掉番号、网站名等
        let title = raw_title
            .replace(&code_upper, "")
            .replace(&code_lower, "")
            .replace("ProjectJav", "")
            .replace("- High Speed Jav Torrent", "")
            .replace("jav torrents", "")
            .trim_start_matches(|c: char| c == '-' || c == ' ' || c == '　')
            .trim()
            .to_string();

        // 从行式布局 .row > .col-3 + .col-9 提取字段
        let fields = extract_row_fields(&doc);

        // 制作商
        let studio = fields
            .get("Publisher")
            .or_else(|| fields.get("Studio"))
            .or_else(|| fields.get("Maker"))
            .cloned()
            .unwrap_or_default();

        // 发行日期（格式 DD/MM/YYYY，需转换为 YYYY-MM-DD）
        let raw_date = fields
            .get("Date added")
            .or_else(|| fields.get("Release Date"))
            .cloned()
            .unwrap_or_default();
        let premiered = normalize_date(&raw_date);

        // 演员：.actress-item a 中的文本
        let actors = select_actress_names(&doc).join(", ");

        // 标签：.badge-info a 中的文本
        let tags = select_badge_tags(&doc).join(", ");

        if title.is_empty() && cover_url.is_empty() {
            return None;
        }

        Some(SearchResult {
            code: code_upper,
            title,
            actors,
            duration: String::new(),
            studio,
            source: self.name().to_string(),
            cover_url,
            poster_url: String::new(),
            director: String::new(),
            tags,
            premiered,
            rating: None,
            remote_cover_url: None,
            ..Default::default()
        })
    }
}

// ============ 辅助函数 ============

/// 从详情页查找封面图：img[src*="covers"] 或 img.mw-100
fn select_cover_img(doc: &Html) -> Option<String> {
    // 优先找 src 包含 "covers" 的 img
    let sel = Selector::parse("img").ok()?;
    for el in doc.select(&sel) {
        let src = el.value().attr("src").unwrap_or("");
        if src.contains("/covers/") {
            return Some(src.to_string());
        }
    }
    // fallback: img.mw-100
    let sel2 = Selector::parse("img.mw-100").ok()?;
    doc.select(&sel2)
        .next()
        .and_then(|el| el.value().attr("src"))
        .map(|s| s.to_string())
}

/// 提取演员名：.actress-item a 的文本
fn select_actress_names(doc: &Html) -> Vec<String> {
    let sel = match Selector::parse(".actress-item a") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| {
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        })
        .collect()
}

/// 提取标签：.badge-info a 的文本
fn select_badge_tags(doc: &Html) -> Vec<String> {
    let sel = match Selector::parse(".badge-info a") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| {
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        })
        .collect()
}

/// 提取行式布局字段：.row 中 .col-3 为标签，.col-9 为值
fn extract_row_fields(doc: &Html) -> std::collections::HashMap<String, String> {
    let mut fields = std::collections::HashMap::new();
    let row_sel = match Selector::parse(".row") {
        Ok(s) => s,
        Err(_) => return fields,
    };
    let col3_sel = Selector::parse(".col-3").unwrap();
    let col9_sel = Selector::parse(".col-9").unwrap();

    for row in doc.select(&row_sel) {
        let label = row
            .select(&col3_sel)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string());
        let value = row
            .select(&col9_sel)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string());
        if let (Some(l), Some(v)) = (label, value) {
            if !l.is_empty() && !v.is_empty() {
                fields.insert(l, v);
            }
        }
    }
    fields
}

/// 将 DD/MM/YYYY 格式转换为 YYYY-MM-DD
fn normalize_date(raw: &str) -> String {
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.len() == 3 {
        // DD/MM/YYYY -> YYYY-MM-DD
        format!(
            "{}-{}-{}",
            parts[2].trim(),
            parts[1].trim(),
            parts[0].trim()
        )
    } else {
        raw.to_string()
    }
}

fn select_text(doc: &Html, selector_str: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    let el = doc.select(&sel).next()?;
    let text: String = el.text().collect::<Vec<_>>().join(" ");
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn select_attr(doc: &Html, selector_str: &str, attr: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    let el = doc.select(&sel).next()?;
    el.value().attr(attr).map(|s| s.to_string())
}

/// 在页面图片中查找与番号相关的封面图（fallback）
fn find_cover_image(doc: &Html, code_upper: &str, code_lower: &str) -> Option<String> {
    let sel = Selector::parse("img").ok()?;
    for el in doc.select(&sel) {
        let src = el.value().attr("src").unwrap_or("");
        let alt = el.value().attr("alt").unwrap_or("");
        if src.contains(code_upper)
            || src.contains(code_lower)
            || alt.contains(code_upper)
            || alt.contains(code_lower)
        {
            return Some(src.to_string());
        }
    }
    None
}
