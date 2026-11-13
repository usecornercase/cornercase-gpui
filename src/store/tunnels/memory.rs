use std::collections::BTreeMap;

use anyhow::Result;

use crate::prefs;

const FILE: &str = "remembered-tunnels.json";
const MOST: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Remembered {
    pub(crate) port: u16,
    pub(crate) local: Option<u16>,
}

type Ports = BTreeMap<String, Vec<Remembered>>;

fn read(text: &str) -> Ports {
    let Ok(serde_json::Value::Object(entries)) = serde_json::from_str::<serde_json::Value>(text) else {
        return Ports::new();
    };
    entries.into_iter().map(|(destination, list)| (destination, port_list(&list))).collect()
}

fn valid(value: Option<&serde_json::Value>) -> Option<u16> {
    u16::try_from(value?.as_u64()?).ok().filter(|port| *port != 0)
}

fn entry(item: &serde_json::Value) -> Option<Remembered> {
    match item {
        serde_json::Value::Object(fields) => {
            Some(Remembered { port: valid(fields.get("port"))?, local: valid(fields.get("local")) })
        }
        number => Some(Remembered { port: valid(Some(number))?, local: None }),
    }
}

fn tidy(list: &mut Vec<Remembered>) {
    list.sort_by_key(|entry| entry.port);
    list.dedup_by(|later, kept| {
        let same = later.port == kept.port;
        if same {
            kept.local = kept.local.or(later.local);
        }
        same
    });
    list.truncate(MOST);
}

fn port_list(list: &serde_json::Value) -> Vec<Remembered> {
    let mut entries: Vec<Remembered> =
        list.as_array().map(|items| items.iter().filter_map(entry).collect()).unwrap_or_default();
    tidy(&mut entries);
    entries
}

fn written(entry: Remembered) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    fields.insert("port".to_string(), entry.port.into());
    if let Some(local) = entry.local {
        fields.insert("local".to_string(), local.into());
    }
    serde_json::Value::Object(fields)
}

fn write(ports: &Ports) -> String {
    let kept: BTreeMap<&String, Vec<serde_json::Value>> = ports
        .iter()
        .filter(|(_, list)| !list.is_empty())
        .map(|(d, l)| (d, l.iter().copied().map(written).collect()))
        .collect();
    serde_json::to_string_pretty(&kept).unwrap_or_default() + "\n"
}

pub(crate) fn ports(text: &str, destination: &str) -> Vec<u16> {
    read(text).remove(destination).unwrap_or_default().into_iter().map(|entry| entry.port).collect()
}

pub(crate) fn local_in(text: &str, destination: &str, port: u16) -> Option<u16> {
    read(text).remove(destination)?.into_iter().find(|entry| entry.port == port)?.local
}

pub(crate) fn remember(text: &str, destination: &str, port: u16) -> String {
    let mut all = read(text);
    let list = all.entry(destination.to_string()).or_default();
    list.push(Remembered { port, local: None });
    tidy(list);
    write(&all)
}

pub(crate) fn forget(text: &str, destination: &str, port: u16) -> String {
    let mut all = read(text);
    if let Some(list) = all.get_mut(destination) {
        list.retain(|entry| entry.port != port);
    }
    write(&all)
}

pub(crate) fn note_local(text: &str, destination: &str, port: u16, local: u16) -> Option<String> {
    let mut all = read(text);
    let found = all.get_mut(destination)?.iter_mut().find(|entry| entry.port == port)?;
    if found.local == Some(local) {
        return None;
    }
    found.local = Some(local);
    Some(write(&all))
}

fn path() -> std::path::PathBuf {
    prefs::beside(&prefs::path(), FILE)
}

fn current() -> String {
    std::fs::read_to_string(path()).unwrap_or_default()
}

pub(crate) fn remembered(destination: &str) -> Vec<u16> {
    ports(&current(), destination)
}

pub(crate) fn remembered_local(destination: &str, port: u16) -> Option<u16> {
    local_in(&current(), destination, port)
}

pub(crate) fn save_noted(change: impl FnOnce(&str) -> Option<String>) -> Result<()> {
    match change(&current()) {
        Some(text) => prefs::write_atomic(&path(), &text),
        None => Ok(()),
    }
}

pub(crate) fn save(change: impl FnOnce(&str) -> String) -> Result<()> {
    prefs::write_atomic(&path(), &change(&current()))
}

#[cfg(test)]
mod tests;
