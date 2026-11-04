use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::gather;
use crate::cli::locate::closed;

#[test]
fn gathering_stops_at_the_closing_mark() {
    let (tx, rx) = mpsc::channel();
    for line in ["noise", "cornercase-gpui-path-mark", "/bin", "cornercase-gpui-path-mark", "later"] {
        tx.send(line.to_string()).expect("the channel is open");
    }
    let lines = gather(&rx, Instant::now() + Duration::from_secs(60), closed);
    assert_eq!(lines.len(), 4);
}

#[test]
fn gathering_ends_with_the_channel() {
    let (tx, rx) = mpsc::channel();
    tx.send("one".to_string()).expect("the channel is open");
    drop(tx);
    assert_eq!(gather(&rx, Instant::now() + Duration::from_secs(60), closed), ["one"]);
}
