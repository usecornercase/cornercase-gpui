use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail};
use base64::Engine as _;

const TEXT: u8 = 0x1;
const CLOSE: u8 = 0x8;
const PING: u8 = 0x9;
const PONG: u8 = 0xA;
const CONTINUATION: u8 = 0x0;
const LONGEST: u64 = 256 << 20;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Message {
    Text(String),
    Ping(Vec<u8>),
    Close,
}

pub(crate) struct Writer {
    stream: TcpStream,
    seed: u32,
}

pub(crate) struct Reader {
    stream: BufReader<TcpStream>,
}

pub(crate) struct Address {
    pub(crate) host: String,
    pub(crate) path: String,
}

pub(crate) fn address(url: &str) -> Result<Address> {
    let rest = url.strip_prefix("ws://").with_context(|| format!("not a ws:// address: {url}"))?;
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
    if host.is_empty() {
        bail!("no host in {url}");
    }
    Ok(Address { host: host.to_string(), path })
}

pub(crate) fn connect(url: &str) -> Result<(Writer, Reader)> {
    let at = address(url)?;
    let stream = TcpStream::connect(&at.host).with_context(|| format!("could not reach {}", at.host))?;
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut writer = Writer { stream: stream.try_clone()?, seed: seed() };
    let mut reader = Reader { stream: BufReader::with_capacity(1 << 20, stream) };
    let key = writer.key();
    writer.stream.write_all(handshake(&at, &key).as_bytes())?;
    reader.upgraded()?;
    Ok((writer, reader))
}

fn handshake(at: &Address, key: &str) -> String {
    format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        at.path, at.host
    )
}

fn seed() -> u32 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    nanos ^ std::process::id().rotate_left(16) | 1
}

impl Writer {
    fn next(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed
    }

    fn key(&mut self) -> String {
        let bytes: Vec<u8> = (0..4).flat_map(|_| self.next().to_be_bytes()).collect();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    pub(crate) fn text(&mut self, text: &str) -> Result<()> {
        self.send(TEXT, text.as_bytes())
    }

    pub(crate) fn pong(&mut self, payload: &[u8]) -> Result<()> {
        self.send(PONG, payload)
    }

    fn send(&mut self, opcode: u8, payload: &[u8]) -> Result<()> {
        let mask = self.next().to_be_bytes();
        self.stream.write_all(&encode(opcode, payload, Some(mask)))?;
        Ok(())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

impl Reader {
    fn upgraded(&mut self) -> Result<()> {
        let mut status = String::new();
        self.stream.read_line(&mut status)?;
        if status.split_whitespace().nth(1) != Some("101") {
            bail!("the browser refused the connection: {}", status.trim());
        }
        loop {
            let mut line = String::new();
            if self.stream.read_line(&mut line)? == 0 {
                bail!("the browser closed the connection");
            }
            if line.trim().is_empty() {
                return Ok(());
            }
        }
    }

    pub(crate) fn message(&mut self) -> Result<Message> {
        read_message(&mut self.stream)
    }
}

pub(crate) fn encode(opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 14);
    out.push(0x80 | opcode);
    let bit = if mask.is_some() { 0x80 } else { 0 };
    let len = payload.len();
    if len < 126 {
        out.push(bit | u8::try_from(len).unwrap_or(0));
    } else if let Ok(short) = u16::try_from(len) {
        out.push(bit | 0x7e);
        out.extend(short.to_be_bytes());
    } else {
        out.push(bit | 0x7f);
        out.extend(u64::try_from(len).unwrap_or(u64::MAX).to_be_bytes());
    }
    match mask {
        Some(mask) => {
            out.extend(mask);
            out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        }
        None => out.extend_from_slice(payload),
    }
    out
}

struct Frame {
    fin: bool,
    opcode: u8,
    payload: Vec<u8>,
}

fn read_frame(stream: &mut impl Read) -> Result<Frame> {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head)?;
    let len = match head[1] & 0x7f {
        126 => u64::from(u16::from_be_bytes(read_array(stream)?)),
        127 => u64::from_be_bytes(read_array(stream)?),
        short => u64::from(short),
    };
    if len > LONGEST {
        bail!("a message of {len} bytes is too long");
    }
    let mask: Option<[u8; 4]> = if head[1] & 0x80 == 0 { None } else { Some(read_array(stream)?) };
    let mut payload = vec![0u8; usize::try_from(len)?];
    stream.read_exact(&mut payload)?;
    if let Some(mask) = mask {
        payload.iter_mut().enumerate().for_each(|(i, b)| *b ^= mask[i % 4]);
    }
    Ok(Frame { fin: head[0] & 0x80 != 0, opcode: head[0] & 0x0f, payload })
}

fn read_array<const N: usize>(stream: &mut impl Read) -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn read_message(stream: &mut impl Read) -> Result<Message> {
    let mut text: Vec<u8> = Vec::new();
    loop {
        let frame = read_frame(stream)?;
        match frame.opcode {
            PING => return Ok(Message::Ping(frame.payload)),
            CLOSE => return Ok(Message::Close),
            CONTINUATION | TEXT => text.extend(frame.payload),
            _ => {
                text.clear();
                continue;
            }
        }
        if frame.fin {
            return Ok(Message::Text(String::from_utf8_lossy(&text).into_owned()));
        }
    }
}

#[cfg(test)]
mod tests;
