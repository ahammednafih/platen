//! Minimal IPP/2.0 client: printer status, ink, capabilities, and printing one JPEG per job.
//!
//! The target device accepts image/jpeg, image/urf and image/pwg-raster (no PDF), so
//! `printjob` lays pages out on sheets and sends each sheet as a JPEG.
use anyhow::{Result, bail};
use std::collections::HashMap;
use std::time::Duration;

const OP_PRINT_JOB: u16 = 0x0002;
const OP_GET_PRINTER_ATTRIBUTES: u16 = 0x000B;
const OP_CANCEL_JOB: u16 = 0x0008;
const OP_GET_JOBS: u16 = 0x000A;

// Value tags.
const T_INTEGER: u8 = 0x21;
const T_BOOLEAN: u8 = 0x22;
const T_ENUM: u8 = 0x23;
const T_RANGE: u8 = 0x33;
const T_KEYWORD: u8 = 0x44;
const T_URI: u8 = 0x45;
const T_CHARSET: u8 = 0x47;
const T_LANGUAGE: u8 = 0x48;
const T_MIME: u8 = 0x49;
const T_NAME: u8 = 0x42;
const T_BEGIN_COLLECTION: u8 = 0x34;
const T_END_COLLECTION: u8 = 0x37;
const T_MEMBER_NAME: u8 = 0x4A;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i32),
    Bool(bool),
    Range(i32, i32),
    Text(String),
    Raw(Vec<u8>),
}

impl Value {
    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}

pub type Attrs = HashMap<String, Vec<Value>>;

#[derive(Clone)]
pub struct Printer {
    uri: String,
    url: String,
    agent: ureq::Agent,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Ink {
    pub name: String,
    pub level: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Status {
    pub model: String,
    pub state: String,
    pub reasons: Vec<String>,
    pub ink: Vec<Ink>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Quality {
    Draft,
    Normal,
    High,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrintOpts {
    pub copies: u32,
    pub color: bool,
    /// IPP media keyword, e.g. `iso_a4_210x297mm`.
    pub media: String,
    /// `fit` (shrink long pages onto the sheet), `fill`, `none`, `auto`.
    pub scaling: String,
    pub quality: Quality,
    /// IPP `media-type`, e.g. `stationery` or `photographic-glossy`; the printer's default when unset.
    #[serde(default)]
    pub media_type: Option<String>,
    /// Print to the edges of the paper, where the printer allows it.
    #[serde(default)]
    pub borderless: bool,
}

impl Default for PrintOpts {
    fn default() -> Self {
        PrintOpts { copies: 1, color: true, media: "iso_a4_210x297mm".into(), scaling: "fit".into(), quality: Quality::Normal, media_type: None, borderless: false }
    }
}

const TIMEOUT: Duration = Duration::from_secs(120);

/// A busy printer reads a sheet only as fast as it prints, which can take minutes for a photo.
const PRINT_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// What the printer offers, for the print dialog.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PrintCaps {
    pub media: Vec<String>,
    pub media_default: String,
    pub color: bool,
    pub qualities: Vec<Quality>,
    pub max_copies: i32,
    /// True when the printer reports two-sided printing (otherwise use odd/even sheets).
    pub duplex: bool,
    /// Paper types the printer takes, e.g. `stationery`, `photographic-glossy`.
    pub media_types: Vec<String>,
    /// True when the printer can print without margins.
    pub borderless: bool,
}

impl Printer {
    /// Printer on the same device as the scanner (`host` = IP or name).
    pub fn for_host(host: &str) -> Printer {
        let host = host.split(':').next().unwrap_or(host);
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .into();
        Printer { uri: format!("ipp://{host}:631/ipp/print"), url: format!("http://{host}:631/ipp/print"), agent }
    }

    pub fn attributes(&self, names: &[&str]) -> Result<Attrs> {
        let mut req = Request::new(OP_GET_PRINTER_ATTRIBUTES, &self.uri);
        req.keywords("requested-attributes", names);
        let (_, groups) = self.send(req.end(), &[])?;
        Ok(groups.into_iter().flatten().collect())
    }

    pub fn status(&self) -> Result<Status> {
        let a = self.attributes(&["printer-make-and-model", "printer-state", "printer-state-reasons", "marker-names", "marker-levels"])?;
        let strs = |k: &str| -> Vec<String> { a.get(k).map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() };
        let levels: Vec<i32> = a.get("marker-levels").map(|v| v.iter().filter_map(Value::as_int).collect()).unwrap_or_default();
        let ink = strs("marker-names").into_iter().zip(levels).map(|(name, level)| Ink { name: ink_name(&name), level }).collect();
        let state = match a.get("printer-state").and_then(|v| v.first()).and_then(Value::as_int) {
            Some(3) => "idle",
            Some(4) => "printing",
            Some(5) => "stopped",
            _ => "unknown",
        };
        Ok(Status {
            model: strs("printer-make-and-model").into_iter().next().unwrap_or_default(),
            state: state.into(),
            reasons: strs("printer-state-reasons").into_iter().filter(|r| r != "none").collect(),
            ink,
        })
    }

    pub fn caps(&self) -> Result<PrintCaps> {
        let a = self.attributes(&[
            "media-supported", "media-default", "print-color-mode-supported", "print-quality-supported", "copies-supported", "sides-supported",
            "media-type-supported", "media-bottom-margin-supported",
        ])?;
        let strs = |k: &str| -> Vec<String> { a.get(k).map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default() };
        let qualities = a.get("print-quality-supported").map(|v| v.iter().filter_map(Value::as_int).filter_map(quality).collect()).unwrap_or_default();
        let max_copies = match a.get("copies-supported").and_then(|v| v.first()) {
            Some(Value::Range(_, hi)) => *hi,
            _ => 1,
        };
        Ok(PrintCaps {
            media: strs("media-supported"),
            media_default: strs("media-default").into_iter().next().unwrap_or_else(|| "iso_a4_210x297mm".into()),
            color: strs("print-color-mode-supported").iter().any(|m| m == "color"),
            qualities,
            max_copies,
            duplex: strs("sides-supported").iter().any(|s| s.starts_with("two-sided")),
            media_types: strs("media-type-supported"),
            borderless: a.get("media-bottom-margin-supported").is_some_and(|v| v.iter().any(|m| m.as_int() == Some(0))),
        })
    }

    /// Print one JPEG page. Returns the job id.
    pub fn print_jpeg(&self, jpeg: &[u8], o: &PrintOpts, job_name: &str) -> Result<i32> {
        let mut req = Request::new(OP_PRINT_JOB, &self.uri);
        req.attr(T_NAME, "job-name", job_name.as_bytes());
        req.attr(T_MIME, "document-format", b"image/jpeg");
        req.group(0x02);
        req.attr(T_INTEGER, "copies", &(o.copies.max(1) as i32).to_be_bytes());
        req.attr(T_KEYWORD, "print-color-mode", if o.color { b"color" as &[u8] } else { b"monochrome" });
        match crate::sheet::Paper::from_media(&o.media).filter(|_| o.media_type.is_some() || o.borderless) {
            Some(paper) => req.media_col(paper, o.media_type.as_deref(), o.borderless),
            None => req.attr(T_KEYWORD, "media", o.media.as_bytes()),
        }
        req.attr(T_KEYWORD, "print-scaling", if o.borderless { b"fill" as &[u8] } else { o.scaling.as_bytes() });
        let q: i32 = match o.quality {
            Quality::Draft => 3,
            Quality::Normal => 4,
            Quality::High => 5,
        };
        req.attr(T_ENUM, "print-quality", &q.to_be_bytes());
        let (_, groups) = self.send_timeout(req.end(), jpeg, PRINT_TIMEOUT)?;
        let job = groups.into_iter().flatten().find(|(k, _)| k == "job-id").and_then(|(_, v)| v.first().and_then(Value::as_int));
        Ok(job.unwrap_or(0))
    }

    /// Ids of jobs the printer hasn't finished (queued or printing).
    pub fn active_jobs(&self) -> Result<Vec<i32>> {
        let mut req = Request::new(OP_GET_JOBS, &self.uri);
        req.attr(T_KEYWORD, "which-jobs", b"not-completed");
        req.keywords("requested-attributes", &["job-id"]);
        let (_, groups) = self.send(req.end(), &[])?;
        Ok(groups.iter().filter_map(|g| g.iter().find(|(k, _)| k == "job-id")).filter_map(|(_, v)| v.first().and_then(Value::as_int)).collect())
    }

    pub fn cancel_job(&self, id: i32) -> Result<()> {
        let mut req = Request::new(OP_CANCEL_JOB, &self.uri);
        req.attr(T_INTEGER, "job-id", &id.to_be_bytes());
        self.send(req.end(), &[]).map(drop)
    }

    /// Cancel everything still queued or printing; returns how many jobs were cancelled.
    pub fn cancel_active(&self) -> Result<usize> {
        let jobs = self.active_jobs()?;
        jobs.iter().try_for_each(|id| self.cancel_job(*id))?;
        Ok(jobs.len())
    }

    fn send(&self, body: Vec<u8>, doc: &[u8]) -> Result<(u16, Vec<Vec<(String, Vec<Value>)>>)> {
        self.send_timeout(body, doc, TIMEOUT)
    }

    fn send_timeout(&self, body: Vec<u8>, doc: &[u8], timeout: Duration) -> Result<(u16, Vec<Vec<(String, Vec<Value>)>>)> {
        let mut data = body;
        data.extend_from_slice(doc);
        let req = self.agent.post(&self.url).config().timeout_global(Some(timeout)).build();
        let mut r = req.header("Content-Type", "application/ipp").send(&data[..])?;
        if r.status() != 200 {
            bail!("printer HTTP {}", r.status());
        }
        let bytes = r.body_mut().with_config().limit(16 << 20).read_to_vec()?;
        let (status, groups) = parse(&bytes)?;
        if status >= 0x0400 {
            bail!("printer refused request (IPP status 0x{status:04x})");
        }
        Ok((status, groups))
    }
}

fn quality(v: i32) -> Option<Quality> {
    match v {
        3 => Some(Quality::Draft),
        4 => Some(Quality::Normal),
        5 => Some(Quality::High),
        _ => None,
    }
}

fn ink_name(n: &str) -> String {
    match n {
        "BK" | "K" => "Black",
        "C" => "Cyan",
        "M" => "Magenta",
        "Y" => "Yellow",
        other => other,
    }
    .to_string()
}

struct Request(Vec<u8>);

impl Request {
    fn new(op: u16, printer_uri: &str) -> Request {
        let mut b = vec![2, 0];
        b.extend_from_slice(&op.to_be_bytes());
        b.extend_from_slice(&1u32.to_be_bytes());
        let mut r = Request(b);
        r.group(0x01);
        r.attr(T_CHARSET, "attributes-charset", b"utf-8");
        r.attr(T_LANGUAGE, "attributes-natural-language", b"en");
        r.attr(T_URI, "printer-uri", printer_uri.as_bytes());
        r.attr(T_NAME, "requesting-user-name", b"scan");
        r
    }

    fn group(&mut self, tag: u8) {
        self.0.push(tag);
    }

    fn attr(&mut self, tag: u8, name: &str, value: &[u8]) {
        self.0.push(tag);
        self.0.extend_from_slice(&(name.len() as u16).to_be_bytes());
        self.0.extend_from_slice(name.as_bytes());
        self.0.extend_from_slice(&(value.len() as u16).to_be_bytes());
        self.0.extend_from_slice(value);
    }

    /// `media-col` instead of `media`: the paper size plus a paper type and/or zero margins.
    fn media_col(&mut self, paper: crate::sheet::Paper, media_type: Option<&str>, borderless: bool) {
        let hundredths = |mm: f32| ((mm * 100.0).round() as i32).to_be_bytes();
        self.attr(T_BEGIN_COLLECTION, "media-col", b"");
        self.member("media-size");
        self.attr(T_BEGIN_COLLECTION, "", b"");
        self.member("x-dimension");
        self.attr(T_INTEGER, "", &hundredths(paper.w_mm));
        self.member("y-dimension");
        self.attr(T_INTEGER, "", &hundredths(paper.h_mm));
        self.attr(T_END_COLLECTION, "", b"");
        if let Some(t) = media_type {
            self.member("media-type");
            self.attr(T_KEYWORD, "", t.as_bytes());
        }
        if borderless {
            for m in ["media-bottom-margin", "media-left-margin", "media-right-margin", "media-top-margin"] {
                self.member(m);
                self.attr(T_INTEGER, "", &0i32.to_be_bytes());
            }
        }
        self.attr(T_END_COLLECTION, "", b"");
    }

    fn member(&mut self, name: &str) {
        self.attr(T_MEMBER_NAME, "", name.as_bytes());
    }

    fn keywords(&mut self, name: &str, values: &[&str]) {
        for (i, v) in values.iter().enumerate() {
            self.attr(T_KEYWORD, if i == 0 { name } else { "" }, v.as_bytes());
        }
    }

    fn end(mut self) -> Vec<u8> {
        self.0.push(0x03);
        self.0
    }
}

type Groups = Vec<Vec<(String, Vec<Value>)>>;

fn parse(b: &[u8]) -> Result<(u16, Groups)> {
    if b.len() < 9 {
        bail!("short IPP response");
    }
    let status = u16::from_be_bytes([b[2], b[3]]);
    let mut i = 8;
    let mut groups: Groups = Vec::new();
    let take = |i: &mut usize, n: usize| -> Result<&[u8]> {
        let s = b.get(*i..*i + n).ok_or_else(|| anyhow::anyhow!("truncated IPP response"))?;
        *i += n;
        Ok(s)
    };
    while i < b.len() {
        let tag = b[i];
        i += 1;
        if tag == 0x03 {
            break;
        }
        if tag < 0x10 {
            groups.push(Vec::new());
            continue;
        }
        let nl = u16::from_be_bytes(take(&mut i, 2)?.try_into()?) as usize;
        let name = String::from_utf8_lossy(take(&mut i, nl)?).into_owned();
        let vl = u16::from_be_bytes(take(&mut i, 2)?.try_into()?) as usize;
        let v = take(&mut i, vl)?;
        let value = match tag {
            T_INTEGER | T_ENUM if vl == 4 => Value::Int(i32::from_be_bytes(v.try_into()?)),
            T_BOOLEAN if vl == 1 => Value::Bool(v[0] != 0),
            T_RANGE if vl == 8 => Value::Range(i32::from_be_bytes(v[..4].try_into()?), i32::from_be_bytes(v[4..].try_into()?)),
            0x30..=0x32 | 0x34..=0x3F => Value::Raw(v.to_vec()),
            _ => Value::Text(String::from_utf8_lossy(v).into_owned()),
        };
        let Some(g) = groups.last_mut() else { continue };
        if name.is_empty() {
            if let Some((_, vals)) = g.last_mut() {
                vals.push(value);
            }
        } else {
            g.push((name, vec![value]));
        }
    }
    Ok((status, groups))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(b: &[u8], needle: &[u8]) -> Option<usize> {
        b.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn media_col_carries_size_type_and_zero_margins() {
        let mut r = Request(Vec::new());
        r.media_col(crate::sheet::Paper { w_mm: 101.6, h_mm: 152.4 }, Some("photographic-glossy"), true);
        let b = r.0;
        assert_eq!(b[0], T_BEGIN_COLLECTION);
        assert!(find(&b, b"media-col").is_some());
        let x = find(&b, b"x-dimension").unwrap();
        assert_eq!(&b[x + 11 + 5..x + 11 + 5 + 4], &10160i32.to_be_bytes());
        assert!(find(&b, b"photographic-glossy").is_some());
        assert!(find(&b, b"media-top-margin").is_some());
        assert_eq!(b.iter().filter(|&&t| t == T_END_COLLECTION).count(), 2);
    }

    #[test]
    fn plain_jobs_still_send_media_keyword() {
        let o = PrintOpts::default();
        let paper = crate::sheet::Paper::from_media(&o.media).filter(|_| o.media_type.is_some() || o.borderless);
        assert!(paper.is_none());
    }
}

