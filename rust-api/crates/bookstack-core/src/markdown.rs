use pulldown_cmark::{html, Options, Parser};

/// Render user markdown to sanitized HTML.
pub fn render(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);
    let mut raw = String::with_capacity(markdown.len() * 3 / 2);
    html::push_html(&mut raw, parser);

    sanitize(&raw)
}

/// Sanitize untrusted HTML for storage/serving.
pub fn sanitize(html: &str) -> String {
    ammonia::Builder::default()
        .add_generic_attributes(&["class", "id"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .add_tags(&["input"])
        .clean(html)
        .to_string()
}

/// Extract readable plain text from markdown (for plaintext exports).
pub fn to_plaintext(markdown: &str) -> String {
    use pulldown_cmark::{Event, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut out = String::with_capacity(markdown.len());
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Text(text) | Event::Code(text) => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Start(Tag::Item) => out.push_str("- "),
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item)
            | Event::End(TagEnd::CodeBlock | TagEnd::BlockQuote(_) | TagEnd::TableRow) => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Event::End(TagEnd::TableCell) => out.push('\t'),
            Event::Rule => out.push_str("\n---\n"),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Escape text for safe embedding in HTML.
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown() {
        let html = render("# Title\n\nSome **bold** text.");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn renders_tables_and_tasklists() {
        let html = render("| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done");
        assert!(html.contains("<table>"));
        assert!(html.contains("checkbox"));
    }

    #[test]
    fn strips_scripts() {
        let html = render("hello <script>alert(1)</script> world");
        assert!(!html.contains("<script>"));
        assert!(html.contains("hello"));
    }

    #[test]
    fn escapes() {
        assert_eq!(escape_html("<b>&"), "&lt;b&gt;&amp;");
    }
}
