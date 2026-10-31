const SCHEMES: &[&str] = &["about:", "data:", "file:", "chrome:", "view-source:", "blob:"];
const LOCAL_SUFFIXES: &[&str] = &[".localhost", ".local", ".test", ".internal", ".lan"];
const SEARCH: &str = "https://www.google.com/search?q=";

pub(crate) fn normalize(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains("://") || SCHEMES.iter().any(|s| text.starts_with(s)) {
        return Some(text.to_string());
    }
    if let Some(port) = bare_port(text) {
        return Some(format!("http://localhost:{port}"));
    }
    if text.contains(char::is_whitespace) || !looks_like_host(host_of(text)) {
        return Some(format!("{SEARCH}{}", encode(text)));
    }
    let scheme = if is_local(host_of(text)) { "http" } else { "https" };
    Some(format!("{scheme}://{text}"))
}

fn bare_port(text: &str) -> Option<u16> {
    text.strip_prefix(':').unwrap_or(text).parse().ok().filter(|p| *p > 0)
}

fn host_of(text: &str) -> &str {
    let end = text.find(['/', '?', '#']).unwrap_or(text.len());
    let host = &text[..end];
    if host.starts_with('[') {
        return host.split_once(']').map_or(host, |(h, _)| &h[1..]);
    }
    host.rsplit_once(':').filter(|(_, port)| port.parse::<u16>().is_ok()).map_or(host, |(h, _)| h)
}

fn looks_like_host(host: &str) -> bool {
    !host.is_empty() && (host == "localhost" || host.contains('.') || host.contains(':'))
}

fn is_local(host: &str) -> bool {
    host == "localhost"
        || host.contains(':')
        || LOCAL_SUFFIXES.iter().any(|s| host.ends_with(s))
        || host.split('.').all(|part| part.parse::<u8>().is_ok())
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => char::from(b).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests;
