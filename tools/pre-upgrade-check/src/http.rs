use crate::{
    feed,
    runtime::{App, Result},
};
use std::collections::BTreeMap;

pub struct Response {
    pub body: String,
    pub headers: BTreeMap<String, String>,
}
pub fn parse(mut raw: &str, limit: usize) -> Result<Response> {
    let mut headers = BTreeMap::new();
    let mut status = 0;
    while raw.starts_with("HTTP/") {
        let (block, rest) = raw
            .split_once("\r\n\r\n")
            .or_else(|| raw.split_once("\n\n"))
            .ok_or("Incomplete HTTP headers")?;
        let mut lines = block.lines();
        status = lines
            .next()
            .and_then(|s| s.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or("Invalid HTTP status")?;
        headers.clear();
        for line in lines {
            let (key, val) = line.split_once(':').ok_or("Invalid HTTP header")?;
            let key = key.to_ascii_lowercase();
            // Pagination headers must be unambiguous.
            if headers.insert(key.clone(), val.trim().into()).is_some() && key.starts_with("x-") {
                return Err("Duplicate HTTP metadata header".into());
            }
        }
        raw = rest.trim_start_matches(['\r', '\n']);
    }
    if !(200..300).contains(&status) {
        return Err(format!("Missing/successless HTTP response: {status}"));
    }
    if raw.len() > limit {
        return Err("Response exceeds configured size limit".into());
    }
    Ok(Response {
        body: raw.into(),
        headers,
    })
}
pub fn get(app: &mut App, url: &str, limit: usize) -> Result<Response> {
    if !app.have("curl") {
        return Err("curl is missing; online data could not be downloaded".into());
    }
    app.line(format!("Fetching {url}"));
    let r = app.get(
        "curl",
        &[
            "-q",
            "--fail",
            "--silent",
            "--show-error",
            "--include",
            "--location",
            "--max-redirs",
            "3",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-filesize",
            &limit.to_string(),
            "--url",
            url,
        ],
    );
    if !r.ok() || !r.err.trim().is_empty() {
        return Err(format!("Download failed: {}", feed::safe_text(&r.detail())));
    }
    parse(&r.out, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_use_final_headers_and_reject_partial_responses() {
        let r = parse("HTTP/1.1 200 Connection established\r\n\r\nHTTP/2 302\r\nx-page: 99\r\n\r\nHTTP/2 200\r\nx-page: 1\r\nx-next-page: 2\r\n\r\n[]", 100).unwrap();
        assert_eq!(r.headers["x-page"], "1");
        assert_eq!(r.body, "[]");
        assert!(parse("[]", 100).is_err());
        assert!(parse("HTTP/2 403\r\n\r\n[]", 100).is_err());
        assert!(parse("HTTP/2 200\r\n\r\n[]", 1).is_err());
    }
}
