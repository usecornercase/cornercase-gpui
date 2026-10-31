use std::io::Cursor;

use super::*;

#[test]
fn a_masked_text_frame_reads_back_as_its_text() {
    let bytes = encode(TEXT, b"{\"id\":1}", Some([1, 2, 3, 4]));
    let message = read_message(&mut Cursor::new(bytes)).expect("reads");
    assert_eq!(message, Message::Text("{\"id\":1}".into()));
}

#[test]
fn a_long_unmasked_frame_carries_its_length_in_eight_bytes() {
    let payload = "x".repeat(70_000);
    let bytes = encode(TEXT, payload.as_bytes(), None);
    assert_eq!(bytes[1], 127);
    let message = read_message(&mut Cursor::new(bytes)).expect("reads");
    assert_eq!(message, Message::Text(payload));
}

#[test]
fn a_medium_frame_carries_its_length_in_two_bytes() {
    let bytes = encode(TEXT, &[b'a'; 300], None);
    assert_eq!((bytes[1], u16::from_be_bytes([bytes[2], bytes[3]])), (126, 300));
}

#[test]
fn fragments_join_into_one_message() {
    let mut first = encode(TEXT, b"hel", None);
    first[0] &= 0x7f;
    let rest = encode(CONTINUATION, b"lo", None);
    let message = read_message(&mut Cursor::new([first, rest].concat())).expect("reads");
    assert_eq!(message, Message::Text("hello".into()));
}

#[test]
fn a_ping_is_handed_back_to_be_answered() {
    let message = read_message(&mut Cursor::new(encode(PING, b"hi", None))).expect("reads");
    assert_eq!(message, Message::Ping(b"hi".to_vec()));
}

#[test]
fn pongs_and_binary_frames_are_skipped() {
    let bytes = [encode(PONG, b"", None), encode(0x2, b"bin", None), encode(TEXT, b"ok", None)].concat();
    assert_eq!(read_message(&mut Cursor::new(bytes)).expect("reads"), Message::Text("ok".into()));
}

#[test]
fn a_close_frame_ends_the_stream() {
    assert_eq!(read_message(&mut Cursor::new(encode(CLOSE, b"", None))).expect("reads"), Message::Close);
}

#[test]
fn an_address_splits_into_host_and_path() {
    let at = address("ws://127.0.0.1:9222/devtools/browser/abc").expect("parses");
    assert_eq!((at.host.as_str(), at.path.as_str()), ("127.0.0.1:9222", "/devtools/browser/abc"));
}

#[test]
fn an_address_without_ws_is_refused() {
    assert!(address("http://127.0.0.1:9222").is_err());
}
