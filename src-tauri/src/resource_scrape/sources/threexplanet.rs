//! 3xplanet.com 数据源解析器
//!
//! WordPress 站点，内容区 .tdb_single_content 中：
//! - 每个字段独立 <p> 标签，英文+日文混合标签
//! - 英文：Starring, Studio, Tags
//! - 日文：品番, 発売日, 収録時間, 監督, メーカー, レーベル, ジャンル, 出演者
//! - 封面：img 含 _cover
//! - 预览图：img 含 /screens/
//! - 下载区在 <h1 class="postdownload"> 之后，需要截断

use super::{SearchResult, Source};
use scraper::{ElementRef, Html, Selector};

pub struct ThreeXPlanet;

impl Source for ThreeXPlanet {
    fn name(&self) -> &str {
        "3xplanet"
    }

    fn build_url(&self, code: &str) -> String {
        format!("https://3xplanet.com/{}", code.to_lowercase())
    }

    fn parse(&self, html: &str, code: &str) -> Option<SearchResult> {
        let doc = Html::parse_document(html);

        // 标题
        let title = select_text(&doc, "h1.tdb-title-text")
            .map(|t| {
                t.replace(code, "")
                    .replace(&code.to_uppercase(), "")
                    .replace(&code.to_lowercase(), "")
                    .trim_start_matches(|c: char| c == '-' || c == ' ' || c == '\u{3000}')
                    .to_string()
            })
            .unwrap_or_default();

        // 提取内容区 <p> 标签文本，在 postdownload 之前截断
        let paragraphs = extract_content_paragraphs(&doc);

        // 从内容区提取所有图片
        let content_images = select_all_attr(&doc, ".tdb_single_content img", "src");

        // 封面图
        let cover_url = content_images
            .iter()
            .find(|u| u.contains("_cover") || u.contains("cover"))
            .cloned()
            .or_else(|| select_attr(&doc, r#"meta[property="og:image"]"#, "content"))
            .unwrap_or_default();

        // 预览截图
        let screenshots: Vec<String> = content_images
            .iter()
            .filter(|u| (u.contains("_s.") || u.contains("/screens/")) && !u.contains("_cover"))
            .map(|u| u.replace("/s200/", "/s0/").replace("/s100/", "/s0/"))
            .collect();

        // 演员：日文 出演者 优先
        let actors = find_field(
            &paragraphs,
            &[
                "出演者:",
                "出演者：",
                "Starring:",
                "Starring：",
                "Actress:",
                "Actress：",
                "Cast:",
                "Cast：",
            ],
        )
        .unwrap_or_default();

        // 制作商：日文 メーカー 优先（真正的制作公司名）
        let studio = find_field(
            &paragraphs,
            &[
                "\u{30e1}\u{30fc}\u{30ab}\u{30fc}:",
                "\u{30e1}\u{30fc}\u{30ab}\u{30fc}\u{ff1a}",
                "Maker:",
                "Maker\u{ff1a}",
                "Studio:",
                "Studio\u{ff1a}",
            ],
        )
        .unwrap_or_default();

        // 发行日期：日文 発売日 优先
        let premiered = find_field(
            &paragraphs,
            &[
                "\u{767a}\u{58f2}\u{65e5}:",
                "\u{767a}\u{58f2}\u{65e5}\u{ff1a}",
                "\u{914d}\u{4fe1}\u{958b}\u{59cb}\u{65e5}:",
                "\u{914d}\u{4fe1}\u{958b}\u{59cb}\u{65e5}\u{ff1a}",
                "Release Date:",
                "Release Date\u{ff1a}",
                "Release:",
                "Release\u{ff1a}",
            ],
        )
        .map(|d| d.replace('/', "-"))
        .unwrap_or_default();

        // 时长：日文 収録時間 优先
        let duration = find_field(
            &paragraphs,
            &[
                "\u{53ce}\u{9332}\u{6642}\u{9593}:",
                "\u{53ce}\u{9332}\u{6642}\u{9593}\u{ff1a}",
                "Duration:",
                "Duration\u{ff1a}",
                "Runtime:",
                "Runtime\u{ff1a}",
            ],
        )
        .map(|d| {
            let d = d.trim();
            if d.ends_with("\u{5206}") {
                format!("{}\u{949f}", d)
            } else {
                d.to_string()
            }
        })
        .unwrap_or_default();

        // 导演：日文 監督 优先
        let director = find_field(
            &paragraphs,
            &[
                "\u{76e3}\u{7763}:",
                "\u{76e3}\u{7763}\u{ff1a}",
                "Director:",
                "Director\u{ff1a}",
            ],
        )
        .unwrap_or_default();

        // 类型：日文 ジャンル 优先
        let tags = find_field(
            &paragraphs,
            &[
                "\u{30b8}\u{30e3}\u{30f3}\u{30eb}:",
                "\u{30b8}\u{30e3}\u{30f3}\u{30eb}\u{ff1a}",
                "Genre:",
                "Genre\u{ff1a}",
            ],
        )
        .unwrap_or_default();

        if title.is_empty() && cover_url.is_empty() {
            return None;
        }

        Some(SearchResult {
            code: code.to_uppercase(),
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

fn select_all_attr(doc: &Html, selector_str: &str, attr: &str) -> Vec<String> {
    let sel = match Selector::parse(selector_str) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    doc.select(&sel)
        .filter_map(|el| el.value().attr(attr).map(|s| s.to_string()))
        .collect()
}

/// 提取内容区中 postdownload 之前的所有 <p> 标签文本
fn extract_content_paragraphs(doc: &Html) -> Vec<String> {
    // 先找到内容区容器
    let content_sel = match Selector::parse(".tdb_single_content") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let content_el = match doc.select(&content_sel).next() {
        Some(el) => el,
        None => return vec![],
    };

    let p_sel = match Selector::parse("p") {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    // 收集所有 <p> 文本，遇到含 "~~DOWNLOAD~~" 的就停止（下载区标记）
    let mut paragraphs = Vec::new();
    for p in content_el.select(&p_sel) {
        let text = collect_element_text(&p);
        if text.contains("~~DOWNLOAD~~") || text.contains("~~Download~~") {
            break;
        }
        if !text.is_empty() {
            paragraphs.push(text);
        }
    }
    paragraphs
}

/// 收集元素的纯文本，合并空白
fn collect_element_text(el: &ElementRef) -> String {
    let text: String = el.text().collect::<Vec<_>>().join("");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 从段落列表中查找指定标签的值（先遍历 labels 按优先级，再遍历段落）
fn find_field(paragraphs: &[String], labels: &[&str]) -> Option<String> {
    for label in labels {
        for para in paragraphs {
            if para.starts_with(label) {
                let value = para[label.len()..].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}
