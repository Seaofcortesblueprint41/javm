//! javbus.com 数据源解析器
//!
//! 页面结构：
//! - 封面：.bigImage img src
//! - 标题：h3 文本
//! - 番号/日期/时长等：.info p 中的 span 标签
//! - 类别：.genre a[href*="genre"] 文本
//! - 女优：.star-name a 文本
//! - 预览图：.sample-box a href

use scraper::{Html, Selector};
use super::{Source, SearchResult};

pub struct Javbus;

impl Source for Javbus {
    fn name(&self) -> &str { "javbus" }

    fn build_url(&self, code: &str) -> String {
        format!("https://www.javbus.com/{}", code)
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = Html::parse_document(html);

        // 封面：优先取 a.bigImage 的 href（dmm 大图，不防盗链），fallback 到 img src
        let cover_url = select_attr(&doc, "a.bigImage", "href")
            .or_else(|| select_attr(&doc, ".bigImage img", "src"))
            .map(|u| {
                if u.starts_with("http") { u }
                else { format!("https://www.javbus.com{}", u) }
            })
            .unwrap_or_default();

        // 标题
        let title = select_text(&doc, "h3")
            .map(|t| t.replace(code, "").trim().to_string())
            .unwrap_or_default();

        // 信息区域：解析 .info 下的所有 p 标签
        let info_text = select_text(&doc, ".info").unwrap_or_default();

        // 发行日期
        let premiered = extract_field(&info_text, &["發行日期:", "发行日期:"])
            .unwrap_or_default();

        // 时长
        let duration_raw = extract_field(&info_text, &["長度:", "长度:"])
            .unwrap_or_default();
        let duration = if duration_raw.is_empty() {
            String::new()
        } else {
            // "120分鐘" -> "120分钟"
            duration_raw.replace("分鐘", "分钟")
        };

        // 制作商
        let studio = extract_field(&info_text, &["製作商:", "制作商:"])
            .unwrap_or_default();

        // 导演
        let director = extract_field(&info_text, &["導演:", "导演:"])
            .unwrap_or_default();

        // 类别（只选 href 包含 /genre/ 的链接，排除演员链接）
        let tags = select_all_text_by_href(&doc, "span.genre a", "/genre/")
            .join(", ");

        // 女优
        let actors = select_all_text(&doc, ".star-name a").join(", ");

        // 预览截图：a.sample-box 的 href 指向 dmm 大图
        let screenshots = select_all_attr(&doc, "a.sample-box", "href")
            .into_iter()
            .map(|u| {
                if u.starts_with("http") { u }
                else { format!("https://www.javbus.com{}", u) }
            })
            .collect();

        if title.is_empty() && cover_url.is_empty() {
            return None;
        }

        Some(SearchResult {
            code: code.to_string(),
            title,
            actors,
            duration,
            studio,
            source: self.name().to_string(),
            cover_url,
            director,
            tags,
            premiered,
            rating: None,
            
            screenshots,
            remote_cover_url: None,
        })
    }
}

// ============ 辅助函数 ============

fn select_text(doc: &Html, selector_str: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    let el = doc.select(&sel).next()?;
    let text: String = el.text().collect::<Vec<_>>().join(" ");
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() { None } else { Some(cleaned) }
}

fn select_all_text(doc: &Html, selector_str: &str) -> Vec<String> {
    let sel = match Selector::parse(selector_str) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| {
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if cleaned.is_empty() { None } else { Some(cleaned) }
        })
        .collect()
}

/// 选择所有匹配元素中 href 包含指定路径的文本
fn select_all_text_by_href(doc: &Html, selector_str: &str, href_contains: &str) -> Vec<String> {
    let sel = match Selector::parse(selector_str) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| {
            let href = el.value().attr("href").unwrap_or("");
            if !href.contains(href_contains) {
                return None;
            }
            let text: String = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if cleaned.is_empty() { None } else { Some(cleaned) }
        })
        .collect()
}

fn select_attr(doc: &Html, selector_str: &str, attr: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    let el = doc.select(&sel).next()?;
    el.value().attr(attr).map(|s| s.to_string())
}

fn select_all_attr(doc: &Html, selector_str: &str, attr: &str) -> Vec<String> {
    let sel = match Selector::parse(selector_str) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| el.value().attr(attr).map(|s| s.to_string()))
        .collect()
}

/// 从信息文本中提取指定字段的值
fn extract_field(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        if let Some(pos) = text.find(label) {
            let after = &text[pos + label.len()..];
            let value = after.trim().split_whitespace().next()?;
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}
