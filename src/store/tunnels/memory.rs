use std::collections::BTreeMap;

use anyhow::Result;

use crate::prefs;

const FILE: &str = "remembered-tunnels.json";
const MOST: usize = 64;

type Ports = BTreeMap<String, Vec<u16>>;

fn read(text: &str) -> Ports {
    let Ok(serde_json::Value::Object(entries)) = serde_json::from_str::<serde_json::Value>(text) else {
        return Ports::new();
    };
    entries.into_iter().map(|(destination, list)| (destination, port_list(&list))).collect()
}

fn port_list(list: &serde_json::Value) -> Vec<u16> {
    let mut ports: Vec<u16> = list
        .as_array()
        .map(|items| items.iter().filter_map(|item| u16::try_from(item.as_u64()?).ok()).filter(|p| *p != 0).collect())
        .unwrap_or_default();
    ports.sort_unstable();
    ports.dedup();
    ports.truncate(MOST);
    ports
}

fn write(ports: &Ports) -> String {
    let kept: Ports = ports.iter().filter(|(_, list)| !list.is_empty()).map(|(d, l)| (d.clone(), l.clone())).collect();
    serde_json::to_string_pretty(&kept).unwrap_or_default() + "\n"
}

pub(crate) fn ports(text: &str, destination: &str) -> Vec<u16> {
    read(text).remove(destination).unwrap_or_default()
}

pub(crate) fn remember(text: &str, destination: &str, port: u16) -> String {
    let mut all = read(text);
    let list = all.entry(destination.to_string()).or_default();
    list.push(port);
    list.sort_unstable();
    list.dedup();
    list.truncate(MOST);
    write(&all)
}

pub(crate) fn forget(text: &str, destination: &str, port: u16) -> String {
    let mut all = read(text);
    if let Some(list) = all.get_mut(destination) {
        list.retain(|p| *p != port);
    }
    write(&all)
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

pub(crate) fn save(change: impl FnOnce(&str) -> String) -> Result<()> {
    prefs::write_atomic(&path(), &change(&current()))
}

#[cfg(test)]
mod tests;
