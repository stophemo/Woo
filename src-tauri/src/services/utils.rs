use chrono::FixedOffset;
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// Generate a UUID v4 (matches utils.cjs newId())
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Return current Beijing time (UTC+8) as ISO string
/// Format: YYYY-MM-DDTHH:MM:SS (matches utils.cjs nowStr())
pub fn now_str() -> String {
    let now = chrono::Utc::now();
    let beijing = now.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
    beijing.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// SHA-256 hex hash (matches utils.cjs sha256())
pub fn sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Strip HTML tags, get plain text (matches utils.cjs stripHtml())
pub fn strip_html(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }
    let text = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n")
        .replace("</tr>", "\n");
    let stripped = strip_tags(&text);
    stripped
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .trim()
        .to_string()
}

/// Strip HTML tags, keep line breaks (matches utils.cjs stripHtmlKeepLines())
pub fn strip_html_keep_lines(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }
    // Replace block-level tags with newlines
    let text = html
        .replace("<div", "\n<div")
        .replace("<p", "\n<p")
        .replace("<br", "\n<br")
        .replace("<h1", "\n<h1")
        .replace("<h2", "\n<h2")
        .replace("<h3", "\n<h3")
        .replace("<h4", "\n<h4")
        .replace("<h5", "\n<h5")
        .replace("<h6", "\n<h6")
        .replace("<li", "\n<li")
        .replace("<tr", "\n<tr")
        .replace("<blockquote", "\n<blockquote")
        .replace("<section", "\n<section")
        .replace("<article", "\n<article")
        .replace("<header", "\n<header")
        .replace("<footer", "\n<footer");
    let stripped = strip_tags(&text);
    let cleaned = stripped
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    // Collapse multiple newlines
    let re = regex::Regex::new(r"\n{3,}").unwrap();
    let result = re.replace_all(&cleaned, "\n\n");
    result.trim().to_string()
}

/// Simple HTML tag stripper
fn strip_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    result
}

#[allow(dead_code)]
/// Extract title from HTML content (first line, stripped)
pub fn extract_title(html: &str) -> String {
    let text = strip_html_keep_lines(html);
    let first_line = text.lines().next().unwrap_or("").trim().to_string();
    if first_line.is_empty() {
        "无标题".to_string()
    } else {
        first_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_id_format() {
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn test_now_str_format() {
        let s = now_str();
        assert_eq!(s.len(), 19);
        assert!(s.contains('T'));
    }

    #[test]
    fn test_sha256() {
        let hash = sha256("hello");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_strip_html() {
        let result = strip_html("<p>Hello <b>World</b></p>");
        assert_eq!(result, "Hello World");
    }
}
