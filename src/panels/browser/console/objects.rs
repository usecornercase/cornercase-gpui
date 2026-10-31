use serde_json::Value;

const SPECIFIERS: &[char] = &['s', 'd', 'i', 'f', 'o', 'O', 'c'];

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(crate) fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_string()
}

fn scalar(o: &Value) -> String {
    match (o.get("value"), text(o, "description")) {
        (Some(Value::String(s)), _) => s.clone(),
        (_, Some(description)) => description.to_string(),
        (Some(value), None) => value.to_string(),
        (None, None) => String::new(),
    }
}

pub(crate) fn shown(o: &Value) -> String {
    if let Some(raw) = text(o, "unserializableValue") {
        return raw.to_string();
    }
    let subtype = text(o, "subtype");
    match text(o, "type") {
        Some("undefined") => "undefined".to_string(),
        Some("object") if subtype == Some("null") => "null".to_string(),
        Some("object") if subtype == Some("error") => first_line(text(o, "description").unwrap_or("Error")),
        Some("object") => o.get("preview").map_or_else(|| description(o), preview),
        Some("function") => first_line(text(o, "description").unwrap_or("function")),
        _ => scalar(o),
    }
}

fn description(o: &Value) -> String {
    text(o, "description").or_else(|| text(o, "className")).unwrap_or("Object").to_string()
}

fn property(p: &Value) -> String {
    match (text(p, "type"), text(p, "value")) {
        (Some("string"), Some(value)) => format!("\"{value}\""),
        (_, Some(value)) => value.to_string(),
        (Some("function"), None) => "ƒ".to_string(),
        _ => p["valuePreview"].get("description").and_then(Value::as_str).unwrap_or("…").to_string(),
    }
}

fn entry(e: &Value) -> String {
    let value = text(&e["value"], "description").unwrap_or("…");
    match text(&e["key"], "description") {
        Some(key) => format!("{key} => {value}"),
        None => value.to_string(),
    }
}

fn preview(p: &Value) -> String {
    let props = p["properties"].as_array().map(Vec::as_slice).unwrap_or_default();
    let entries = p["entries"].as_array().map(Vec::as_slice).unwrap_or_default();
    let more = if p["overflow"].as_bool().unwrap_or(false) { ", …" } else { "" };
    let name = text(p, "description").unwrap_or("Object");
    if !entries.is_empty() {
        let items: Vec<String> = entries.iter().map(entry).collect();
        return format!("{name} {{{}{more}}}", items.join(", "));
    }
    if text(p, "subtype") == Some("array") {
        let items: Vec<String> = props.iter().map(property).collect();
        return format!("[{}{more}]", items.join(", "));
    }
    let fields: Vec<String> =
        props.iter().map(|f| format!("{}: {}", text(f, "name").unwrap_or("?"), property(f))).collect();
    let name = if name == "Object" { String::new() } else { format!("{name} ") };
    format!("{name}{{{}{more}}}", fields.join(", "))
}

#[expect(clippy::cast_possible_truncation, reason = "a console's %d shows an integer as JavaScript does")]
fn integer(o: &Value) -> String {
    o["value"].as_f64().map_or_else(|| shown(o), |n| (n.trunc() as i64).to_string())
}

fn substituted(spec: char, o: &Value) -> String {
    match spec {
        'c' => String::new(),
        'd' | 'i' => integer(o),
        _ => shown(o),
    }
}

fn substitute<'a>(format: &str, mut args: &'a [Value]) -> (String, &'a [Value]) {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        let next = chars.peek().copied();
        match (c, next, args.first()) {
            ('%', Some('%'), _) => out.push('%'),
            ('%', Some(spec), Some(arg)) if SPECIFIERS.contains(&spec) => {
                out.push_str(&substituted(spec, arg));
                args = &args[1..];
            }
            _ => {
                out.push(c);
                continue;
            }
        }
        chars.next();
    }
    (out, args)
}

pub(crate) fn formatted(args: &[Value]) -> String {
    let Some(first) = args.first() else { return String::new() };
    let mut parts = Vec::new();
    let mut rest = args;
    if let (Some("string"), Some(format)) = (text(first, "type"), text(first, "value")) {
        let (head, left) = substitute(format, &args[1..]);
        parts.push(head);
        rest = left;
    }
    parts.extend(rest.iter().map(shown));
    parts.join(" ")
}

#[cfg(test)]
mod tests;
