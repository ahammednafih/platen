//! Listen for the device's Scan button. While running: keep this PC registered (the device forgets
//! it after `DURATION` seconds) and turn each press datagram into one `Press`.
//!
//! Datagram: `02 00 <len> 30` then `TYPE=BR;BUTTON=SCAN;USER="name";FUNC=IMAGE;…;SEQ=n;`, sent twice
//! per press with the same SEQ. While the device waits it refuses eSCL; scan over `native` instead.
use crate::snmp;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const PORT: u16 = 54925;
const OID: &str = "1.3.6.1.4.1.2435.2.3.9.2.11.1.1.0";
const DURATION: u64 = 360;
const FUNCS: [&str; 4] = ["IMAGE", "OCR", "EMAIL", "FILE"];

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Press {
    /// Profile picked on the device: IMAGE, OCR, EMAIL or FILE.
    pub func: String,
    pub seq: u32,
}

/// The device the Scan button was turned on for, so the listener can start on connect.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Saved {
    pub host: String,
    pub community: String,
}

fn saved_path() -> std::path::PathBuf {
    dirs::config_dir().unwrap_or_default().join("scan").join("button.json")
}

pub fn saved() -> Option<Saved> {
    serde_json::from_slice(&std::fs::read(saved_path()).ok()?).ok()
}

pub fn save(s: &Saved) -> Result<()> {
    let path = saved_path();
    std::fs::create_dir_all(path.parent().context("no config folder")?)?;
    Ok(std::fs::write(path, serde_json::to_vec(s)?)?)
}

pub fn forget() {
    let _ = std::fs::remove_file(saved_path());
}

/// Name the device lists this PC under: the short host name, ASCII, at most 15 characters.
pub fn pc_name() -> String {
    let out = std::process::Command::new("hostname").arg("-s").output().ok();
    let raw = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let name: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(15).collect();
    if name.is_empty() { "Mac".into() } else { name }
}

/// Register for every profile so panels with a screen can pick any of them.
pub fn register(host: &str, community: &str, name: &str) -> Result<()> {
    let ip = local_ip(host)?;
    for (i, func) in FUNCS.iter().enumerate() {
        let value = format!("TYPE=BR;BUTTON=SCAN;USER=\"{name}\";FUNC={func};HOST={ip}:{PORT};APPNUM={};DURATION={DURATION};BRID=;", i + 1);
        snmp::set_string(host, community, OID, &value)?;
    }
    Ok(())
}

/// Blocks until `stop` is set, calling `on_press` once per press.
pub fn listen(host: &str, community: &str, name: &str, stop: Arc<AtomicBool>, on_press: impl Fn(Press)) -> Result<()> {
    let sock = bind()?;
    sock.set_read_timeout(Some(Duration::from_secs(1)))?;
    register(host, community, name)?;
    let (mut registered, mut last_seq) = (Instant::now(), None);
    let mut buf = [0u8; 2048];
    while !stop.load(Ordering::Relaxed) {
        if registered.elapsed() > Duration::from_secs(DURATION / 2) {
            // A failed refresh (device asleep or restarting) is retried on the next round.
            if register(host, community, name).is_ok() {
                registered = Instant::now();
            }
        }
        let Ok((n, _)) = sock.recv_from(&mut buf) else { continue };
        match parse(&buf[..n], name) {
            Some(p) if last_seq != Some(p.seq) => {
                last_seq = Some(p.seq);
                on_press(p);
            }
            _ => {}
        }
    }
    Ok(())
}

/// A listener being replaced lets go of the port within a second, so retry for a bit.
fn bind() -> Result<UdpSocket> {
    for _ in 0..5 {
        if let Ok(s) = UdpSocket::bind(("0.0.0.0", PORT)) {
            return Ok(s);
        }
        std::thread::sleep(Duration::from_millis(600));
    }
    UdpSocket::bind(("0.0.0.0", PORT)).with_context(|| format!("port {PORT} is taken (is the vendor's scan app running?)"))
}

/// The address the device sees this PC at.
fn local_ip(host: &str) -> Result<IpAddr> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.connect((host, 161)).context("can't reach the device")?;
    Ok(sock.local_addr()?.ip())
}

fn parse(datagram: &[u8], name: &str) -> Option<Press> {
    let text = String::from_utf8_lossy(datagram);
    let body = &text[text.find("TYPE=")?..];
    let field = |key: &str| body.split(';').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('=')).map(|v| v.trim_matches('"').to_string());
    if field("BUTTON")? != "SCAN" || field("USER")? != name {
        return None;
    }
    Some(Press { func: field("FUNC")?, seq: field("SEQ")?.parse().ok()? })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRESS: &[u8] = b"\x02\x00v0TYPE=BR;BUTTON=SCAN;USER=\"Mac\";FUNC=IMAGE;HOST=192.0.2.20:54925;APPNUM=1;P1=0;P2=0;P3=0;P4=0;REGID=5620;SEQ=4;";

    #[test]
    fn parses_a_press_for_this_pc() {
        assert_eq!(parse(PRESS, "Mac"), Some(Press { func: "IMAGE".into(), seq: 4 }));
    }

    #[test]
    fn ignores_presses_for_another_pc() {
        assert_eq!(parse(PRESS, "Other"), None);
        assert_eq!(parse(b"junk", "Mac"), None);
    }
}
