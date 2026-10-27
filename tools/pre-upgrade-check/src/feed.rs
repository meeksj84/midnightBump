//! Small, bounded RSS/Atom reader for the two official Arch feeds.
//! No DTDs, external entities, scripts, or commands are evaluated.
use crate::runtime::Result;
use std::collections::BTreeMap;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Debug)]
pub struct Notice {
    pub title: String,
    pub date: String,
    pub link: String,
    pub body: String,
}
#[derive(Default, Debug)]
struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<Node>,
}
impl Node {
    fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.local() == name)
    }
    fn field(&self, name: &str) -> Result<String> {
        let n = self.child(name).ok_or_else(|| format!("Missing {name}"))?;
        if !n.children.is_empty() || n.text.trim().is_empty() {
            return Err(format!("Unsupported or empty {name}"));
        }
        Ok(n.text.trim().to_string())
    }
}

fn entities(s: &str, strict: bool) -> Result<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        let Some(end) = rest.find(';').filter(|end| *end <= 16) else {
            if strict {
                return Err("Invalid XML entity".into());
            }
            out.push('&');
            continue;
        };
        let value = &rest[..end];
        let ch = match value {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" if !strict => Some(' '),
            _ => value
                .strip_prefix("#x")
                .and_then(|s| u32::from_str_radix(s, 16).ok())
                .or_else(|| value.strip_prefix('#').and_then(|s| s.parse().ok()))
                .and_then(char::from_u32),
        };
        if let Some(ch) = ch {
            out.push(ch);
        } else if strict {
            return Err("Unknown XML entity (DTD/entity expansion is disabled)".into());
        } else {
            out.push('&');
            out.push_str(value);
            out.push(';');
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

// Preserve text/newlines while removing terminal control and bidi characters.
pub fn safe_text(s: &str) -> String {
    s.chars()
        .filter(|c| {
            (!c.is_control() || matches!(c, '\n' | '\t'))
                && !matches!(*c as u32, 0x202a..=0x202e | 0x2066..=0x2069)
        })
        .collect()
}
fn plain_html(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let Some(end) = rest[i..].find('>') else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let tag = rest[i + 1..i + end]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/')
            .to_ascii_lowercase();
        if matches!(
            tag.as_str(),
            "br" | "p" | "/p" | "li" | "/li" | "pre" | "/pre" | "div" | "/div"
        ) {
            out.push('\n');
        }
        rest = &rest[i + end + 1..];
    }
    out.push_str(rest);
    safe_text(&entities(&out, false).unwrap_or(out))
        .trim()
        .to_string()
}
fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_:-.".contains(&b))
}
fn start_tag(mut s: &str) -> Result<Node> {
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    let name = &s[..end];
    if !name_ok(name) {
        return Err("Invalid XML tag".into());
    }
    let mut node = Node {
        name: name.into(),
        ..Node::default()
    };
    s = &s[end..];
    while !s.trim().is_empty() {
        s = s.trim_start();
        let eq = s.find('=').ok_or("Invalid XML attribute")?;
        let key = s[..eq].trim();
        if !name_ok(key) {
            return Err("Invalid XML attribute name".into());
        }
        s = s[eq + 1..].trim_start();
        let quote = s.chars().next().ok_or("Missing attribute value")?;
        if !matches!(quote, '\'' | '"') {
            return Err("Unquoted XML attribute".into());
        }
        s = &s[1..];
        let end = s.find(quote).ok_or("Unclosed XML attribute")?;
        let value = entities(&s[..end], true)?;
        if node.attrs.insert(key.into(), value).is_some() {
            return Err("Duplicate XML attribute".into());
        }
        s = &s[end + 1..];
    }
    Ok(node)
}
fn xml(s: &str) -> Result<Node> {
    if s.len() > MAX_BYTES {
        return Err("Feed exceeds 2 MiB limit".into());
    }
    let mut stack = vec![Node::default()];
    let mut rest = s.trim_start_matches('\u{feff}');
    let mut nodes = 0;
    while !rest.is_empty() {
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            stack
                .last_mut()
                .unwrap()
                .text
                .push_str(&entities(&rest[..end], true)?);
            rest = &rest[end..];
            continue;
        }
        if let Some(tail) = rest.strip_prefix("<!--") {
            let end = tail.find("-->").ok_or("Unclosed XML comment")?;
            rest = &tail[end + 3..];
            continue;
        }
        if let Some(tail) = rest.strip_prefix("<![CDATA[") {
            let end = tail.find("]]>").ok_or("Unclosed CDATA")?;
            stack.last_mut().unwrap().text.push_str(&tail[..end]);
            rest = &tail[end + 3..];
            continue;
        }
        if let Some(tail) = rest.strip_prefix("<?") {
            let end = tail.find("?>").ok_or("Unclosed XML declaration")?;
            rest = &tail[end + 2..];
            continue;
        }
        if rest.starts_with("<!") {
            return Err(
                "DTD and XML declarations other than comments/CDATA are unsupported".into(),
            );
        }
        let mut quote = None;
        let end = rest
            .char_indices()
            .skip(1)
            .find_map(|(i, ch)| {
                if let Some(q) = quote {
                    if ch == q {
                        quote = None;
                    }
                } else if matches!(ch, '\'' | '"') {
                    quote = Some(ch);
                } else if ch == '>' {
                    return Some(i);
                }
                None
            })
            .ok_or("Unclosed XML tag")?;
        let tag = rest[1..end].trim();
        rest = &rest[end + 1..];
        if let Some(name) = tag.strip_prefix('/') {
            if stack.len() == 1 {
                return Err("Unexpected XML closing tag".into());
            }
            let n = stack.pop().unwrap();
            if n.name != name.trim() {
                return Err("Mismatched XML closing tag".into());
            }
            stack.last_mut().unwrap().children.push(n);
        } else {
            nodes += 1;
            if nodes > 20_000 || stack.len() > 32 {
                return Err("XML complexity limit exceeded".into());
            }
            let n = start_tag(tag.strip_suffix('/').unwrap_or(tag).trim_end())?;
            if tag.ends_with('/') {
                stack.last_mut().unwrap().children.push(n);
            } else {
                stack.push(n);
            }
        }
    }
    if stack.len() != 1 {
        return Err("Truncated XML feed".into());
    }
    let mut doc = stack.pop().unwrap();
    if !doc.text.trim().is_empty() || doc.children.len() != 1 {
        return Err("Expected exactly one XML feed".into());
    }
    Ok(doc.children.remove(0))
}

pub fn parse(s: &str, atom: bool) -> Result<Vec<Notice>> {
    let root = xml(s)?;
    let parent = if atom && root.local() == "feed" {
        &root
    } else if !atom && root.local() == "rss" {
        root.child("channel").ok_or("RSS channel missing")?
    } else {
        return Err(
            "Response is not the expected RSS/Atom feed (possibly a login or challenge page)"
                .into(),
        );
    };
    let mut notices = vec![];
    for item in parent
        .children
        .iter()
        .filter(|n| n.local() == if atom { "entry" } else { "item" })
    {
        let title = safe_text(&item.field("title")?);
        let date = safe_text(&item.field(if atom { "published" } else { "pubDate" })?);
        let link = if atom {
            item.children
                .iter()
                .filter(|n| n.local() == "link")
                .find(|n| n.attrs.get("rel").map_or(true, |v| v == "alternate"))
                .and_then(|n| n.attrs.get("href"))
                .ok_or("Missing Atom article link")?
                .clone()
        } else {
            item.field("link")?
        };
        let prefix = if atom {
            "https://security.archlinux.org/"
        } else {
            "https://archlinux.org/news/"
        };
        if !link.starts_with(prefix) || link.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err("Unexpected article link; review the source manually".into());
        }
        let body = plain_html(&item.field(if atom { "content" } else { "description" })?);
        if body.is_empty() {
            return Err("Empty article body".into());
        }
        notices.push(Notice {
            title,
            date,
            link,
            body,
        });
    }
    if notices.is_empty() {
        return Err("Feed contains no articles; coverage is unknown".into());
    }
    Ok(notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub const NEWS: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><item><title>linux &amp; boot: manual intervention</title><pubDate>Wed, 21 Oct 2026 00:00:00 +0000</pubDate><link>https://archlinux.org/news/test/</link><description>&lt;p&gt;Read &lt;code&gt;version &amp;gt;= 2&lt;/code&gt; first.&lt;/p&gt;</description></item></channel></rss>"#;
    pub const SECURITY: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>[ASA-test] linux: test advisory</title><published>2026-10-21T00:00:00+00:00</published><content type="html"><![CDATA[<pre>Version &gt;= 2<br/>Review the fix.</pre>]]></content><link rel="alternate" href="https://security.archlinux.org/ASA-test"/></entry></feed>"#;
    #[test]
    fn official_feed_shapes_entities_and_cdata() {
        let n = parse(NEWS, false).unwrap();
        assert_eq!(n[0].title, "linux & boot: manual intervention");
        assert_eq!(n[0].body, "Read version >= 2 first.");
        let s = parse(SECURITY, true).unwrap();
        assert_eq!(s[0].body, "Version >= 2\nReview the fix.");
    }
    #[test]
    fn failed_or_unsupported_responses_are_not_empty_successes() {
        for text in [
            "<html>challenge</html>",
            "<rss><channel></channel></rss>",
            &NEWS[..NEWS.len() - 6],
            "<!DOCTYPE rss><rss/>",
            "<rss><channel></rss>",
        ] {
            assert!(parse(text, false).is_err());
        }
        assert!(parse(&NEWS.replace("&amp;", "&unknown;"), false).is_err());
        assert!(parse(
            &NEWS.replace("https://archlinux.org/", "https://example.com/"),
            false
        )
        .is_err());
        assert!(parse(&"x".repeat(MAX_BYTES + 1), false).is_err());
    }
    #[test]
    fn remote_text_cannot_emit_terminal_controls() {
        assert_eq!(safe_text("a\u{1b}[2J\u{202e}b\n"), "a[2Jb\n");
    }
}

#[cfg(test)]
pub use tests::{NEWS as TEST_NEWS, SECURITY as TEST_SECURITY};
