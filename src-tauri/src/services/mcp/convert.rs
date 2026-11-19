use pulldown_cmark::{html, Options, Parser};
use rmcp::schemars;
use serde::Deserialize;

use crate::services::utils::strip_html;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContentFormat {
    #[default]
    Markdown,
    Html,
}

impl ContentFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Html => "html",
        }
    }
}

/// Convert Markdown to sanitized HTML suitable for the Tiptap editor.
pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    sanitize_html(&output)
}

/// Convert an HTML body to Markdown. This is best-effort; callers can also
/// request the raw HTML when Tiptap-specific structures must be preserved.
pub fn html_to_markdown(html: &str) -> Result<String, String> {
    htmd::convert(html).map_err(|error| format!("HTML 转 Markdown 失败: {error}"))
}

/// Sanitize HTML produced by an external AI tool before it enters the editor.
pub fn sanitize_html(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.add_generic_attributes(["data-type", "data-checked"]);
    builder.add_tags(["mark", "s", "u", "del", "ins"]);
    builder.clean(html).to_string()
}

/// Escape a caller-supplied separator so it is stored as literal text.
pub fn sanitize_text(text: &str) -> String {
    ammonia::clean_text(text)
}

/// Plain-text preview for list/search results.
pub fn preview(html: &str, limit: usize) -> String {
    let plain = strip_html(html);
    let collapsed = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return String::new();
    }
    let mut result: String = collapsed.chars().take(limit).collect();
    if collapsed.chars().count() > limit {
        result.push('…');
    }
    result
}

pub fn derive_title(html: &str) -> String {
    crate::services::document_service::extract_first_line_as_title(html)
}
