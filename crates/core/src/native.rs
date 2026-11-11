//! The device's own network scan protocol (TCP 54921), used where eSCL falls short:
//! eSCL tops out at 600 dpi and silently clamps higher requests, this one does 1200.
//!
//! Wire format (reverse engineered):
//! - server greets `+OK 200\r\n`
//! - `ESC I\nR=x,y\nM=mode\n\x80` → `00 <u16 len> "x,y,?,mm_w,px_w,mm_h,px_h," 00` (granted values)
//! - `ESC X\n…\x80` starts a scan; the reply is blocks of `<type> <9 bytes> <u16 LE len> <payload>`,
//!   type 0x64 = JPEG data, then a 10-byte 0x82 end-of-page and a lone 0x80 end-of-job.
use crate::escl::{ColorMode, ScanOpts};
use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const PORT: u16 = 54921;

/// A full page at 1200 dpi is ~40 MB; anything far past that is a broken or hostile device.
const MAX_SCAN_BYTES: usize = 400_000_000;

/// What the device grants for a requested resolution.
#[derive(Debug, PartialEq)]
pub struct Grant {
    pub dpi: (u32, u32),
    pub max_px: (u32, u32),
}

/// Connect and wait for the greeting; `-NG 401` means another client still holds the scanner.
fn open(host: &str, read_timeout: Duration) -> Result<TcpStream> {
    let ip = host.split(':').next().unwrap_or(host);
    let addr = format!("{ip}:{PORT}").parse().context("bad scanner address")?;
    let mut banner = [0u8; 9];
    for _ in 0..10 {
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3))?;
        s.set_read_timeout(Some(read_timeout))?;
        s.read_exact(&mut banner)?;
        if banner.starts_with(b"+OK") {
            return Ok(s);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!("scanner refused the connection: {:?}", String::from_utf8_lossy(&banner).trim())
}

/// Ask what the device would scan for `dpi` in colour (no scan happens).
pub fn inquire(host: &str, dpi: u32) -> Result<Grant> {
    ask(&mut open(host, Duration::from_secs(3))?, dpi)
}

fn ask(s: &mut TcpStream, dpi: u32) -> Result<Grant> {
    s.write_all(&command(&format!("I\nR={dpi},{dpi}\nM=CGRAY\n")))?;
    let mut head = [0u8; 3];
    s.read_exact(&mut head)?;
    let mut body = vec![0u8; u16::from_le_bytes([head[1], head[2]]) as usize];
    s.read_exact(&mut body)?;
    parse_grant(&String::from_utf8_lossy(&body))
}

fn parse_grant(text: &str) -> Result<Grant> {
    let v: Vec<u32> = text.trim_end_matches('\0').split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match v[..] {
        [x, y, _, _, w, _, h, ..] => Ok(Grant { dpi: (x, y), max_px: (w, h) }),
        _ => bail!("unexpected inquiry reply {text:?}"),
    }
}

/// One platen scan; always colour on the wire (gray is capped at 600×1200), converted after.
pub fn scan(host: &str, o: ScanOpts) -> Result<Vec<u8>> {
    let mut s = open(host, Duration::from_secs(600))?;
    let d = o.dpi;
    let g = ask(&mut s, d)?;
    if g.dpi != (d, d) {
        bail!("scanner offered {}×{} dpi instead of {d}", g.dpi.0, g.dpi.1);
    }
    let w = (o.width * d / 300).min(g.max_px.0);
    let h = (o.height * d / 300).min(g.max_px.1);
    let x = format!("X\nR={d},{d}\nM=CGRAY\nC=JPEG\nJ=MID\nB=50\nN=50\nA=0,0,{w},{h}\nS=NORMAL_SCAN\nP=0\nG=0\nL=0\n");
    s.write_all(&command(&x))?;
    let jpeg = read_blocks(&mut s)?;
    if o.mode == ColorMode::Color {
        return Ok(jpeg);
    }
    let gray = image::load_from_memory(&jpeg)?.to_luma8();
    crate::session::encode_gray(&gray, 92)
}

/// `ESC <body> 0x80`
fn command(body: &str) -> Vec<u8> {
    let mut c = vec![0x1b];
    c.extend(body.as_bytes());
    c.push(0x80);
    c
}

fn read_blocks(r: &mut impl Read) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let mut t = [0u8; 1];
        r.read_exact(&mut t).context("scanner closed the connection mid-scan")?;
        match t[0] {
            0x80 => break,
            0x82 => r.read_exact(&mut [0u8; 9])?,
            0x64 => {
                let mut h = [0u8; 11];
                r.read_exact(&mut h)?;
                let n = u16::from_le_bytes([h[9], h[10]]) as usize;
                let at = out.len();
                if at + n > MAX_SCAN_BYTES {
                    bail!("scanner sent more than {} MB for one page", MAX_SCAN_BYTES / 1_000_000);
                }
                out.resize(at + n, 0);
                r.read_exact(&mut out[at..])?;
            }
            other => bail!("scanner sent an unknown block 0x{other:02x} (lid open or paper missing?)"),
        }
    }
    if !out.starts_with(&[0xff, 0xd8]) {
        bail!("scanner sent no image");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_granted_resolution_and_area() {
        let g = parse_grant("1200,1200,2,213,10105,295,13937,\0").unwrap();
        assert_eq!(g, Grant { dpi: (1200, 1200), max_px: (10105, 13937) });
    }

    #[test]
    fn joins_jpeg_blocks_until_end_of_job() {
        let block = |p: &[u8]| {
            let mut b = vec![0x64, 7, 0, 1, 0, 0x84, 0, 0, 0, 0];
            b.extend((p.len() as u16).to_le_bytes());
            b.extend(p);
            b
        };
        let mut wire = block(&[0xff, 0xd8, 1]);
        wire.extend(block(&[2, 0xff, 0xd9]));
        wire.extend([0x82, 7, 0, 1, 0, 0x84, 0, 0, 0, 0, 0x80]);
        assert_eq!(read_blocks(&mut wire.as_slice()).unwrap(), [0xff, 0xd8, 1, 2, 0xff, 0xd9]);
    }
}
