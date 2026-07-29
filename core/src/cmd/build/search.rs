use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SearchEntry {
    pub title: String,
    pub permalink: String,
    pub description: String,
    pub body: String,
}

pub fn extract_entries(pages: &HashMap<String, String>) -> Vec<SearchEntry> {
    let mut entries = Vec::with_capacity(pages.len());
    for (url, html) in pages {
        entries.push(extract_entry(url, html));
    }
    entries
}

fn extract_entry(permalink: &str, html: &str) -> SearchEntry {
    let title = extract_title(html);
    let description = extract_description(html);
    let body = strip_html(extract_main(html));
    SearchEntry {
        title,
        permalink: permalink.to_string(),
        description,
        body,
    }
}

fn extract_main(html: &str) -> &str {
    if let Some(start) = html.find("<main") {
        let after = &html[start..];
        if let Some(end) = after.find("</main>") {
            return &after[..end + 7];
        }
    }
    html
}

fn extract_title(html: &str) -> String {
    if let Some(start) = html.find("<title>") {
        let after = &html[start + 7..];
        if let Some(end) = after.find("</title>") {
            return unescape_html(&after[..end]);
        }
    }
    String::new()
}

fn extract_description(html: &str) -> String {
    let prefix = r#"<meta name="description" content=""#;
    if let Some(start) = html.find(prefix) {
        let after = &html[start + prefix.len()..];
        if let Some(end) = after.find(r#"""#) {
            return unescape_html(&after[..end]);
        }
    }
    String::new()
}

fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut chars = html.char_indices().peekable();

    while let Some((_, c)) = chars.next() {
        if in_script {
            if c == '<' {
                loop { match chars.next() { Some((_, '>')) | None => break, _ => {} } }
                in_script = false;
            }
            continue;
        }
        if in_style {
            if c == '<' {
                loop { match chars.next() { Some((_, '>')) | None => break, _ => {} } }
                in_style = false;
            }
            continue;
        }
        if c == '<' {
            in_tag = true;
            // check for <script and <style
            let peek: String = chars.clone().take(6).map(|(_, c)| c).collect();
            if peek.starts_with("script") || peek.starts_with("script ") {
                in_script = true;
            } else if peek.starts_with("style") || peek.starts_with("style ") {
                in_style = true;
            }
            continue;
        }
        if c == '>' && in_tag {
            in_tag = false;
            continue;
        }
        if !in_tag {
            result.push(c);
        }
    }

    // Collapse whitespace
    let mut cleaned = String::with_capacity(result.len());
    let mut prev_space = false;
    for c in result.chars() {
        if c.is_whitespace() {
            if !prev_space {
                cleaned.push(' ');
                prev_space = true;
            }
        } else {
            cleaned.push(c);
            prev_space = false;
        }
    }

    cleaned.trim().to_string()
}

fn unescape_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}
