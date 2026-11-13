use serde::Deserialize;

use super::TunnelSeen;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct TunnelList {
    pub tunnels: Vec<TunnelSeen>,
    pub windows: usize,
    #[serde(default)]
    pub takers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Order {
    Open(u16),
    Close(u16),
    Forget(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OrdersLine {
    Taking(TunnelList),
    Order(Order),
}

pub(crate) fn orders_args() -> [String; 2] {
    ["tunnel".to_string(), "orders".to_string()]
}

pub(crate) fn failed_args(port: u16, reason: &str) -> [String; 5] {
    let one_line = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    ["tunnel".to_string(), "failed".to_string(), "--".to_string(), port.to_string(), one_line]
}

#[derive(Deserialize)]
struct OrderLine {
    order: String,
    port: u16,
}

pub(crate) fn orders_line(line: &str) -> Option<OrdersLine> {
    let line = line.trim();
    if let Ok(asked) = serde_json::from_str::<OrderLine>(line) {
        let port = asked.port;
        return match asked.order.as_str() {
            _ if port == 0 => None,
            "open" => Some(OrdersLine::Order(Order::Open(port))),
            "close" => Some(OrdersLine::Order(Order::Close(port))),
            "forget" => Some(OrdersLine::Order(Order::Forget(port))),
            _ => None,
        };
    }
    serde_json::from_str::<TunnelList>(line).ok().map(OrdersLine::Taking)
}

#[cfg(test)]
mod tests;
