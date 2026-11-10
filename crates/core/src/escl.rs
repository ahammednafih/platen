//! Minimal eSCL (AirScan) client: discover the scanner, run one platen scan.
use anyhow::{Context, Result, anyhow, bail};
use std::time::Duration;

/// Glass size in 1/300 inch (from ScannerCapabilities).
pub const GLASS_W: u32 = 2550;
pub const GLASS_H: u32 = 3507;
/// A4 = 210 x 297 mm, height clamped to the glass.
pub const A4_W: u32 = 2480;
pub const A4_H: u32 = 3507;

#[derive(Clone)]
pub struct Scanner {
    pub name: String,
    pub base: String, // e.g. http://192.0.2.10:80/eSCL
    agent: ureq::Agent,
}

/// Colour modes offered to the user. The device rejects 1-bit B&W as JPEG,
/// so `BlackWhite` scans in gray and is thresholded locally (see `capture`).
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum ColorMode {
    Color,
    Gray,
    BlackWhite,
}

impl ColorMode {
    pub fn next(self) -> ColorMode {
        match self {
            ColorMode::Color => ColorMode::Gray,
            ColorMode::Gray => ColorMode::BlackWhite,
            ColorMode::BlackWhite => ColorMode::Color,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ColorMode::Color => "Colour",
            ColorMode::Gray => "Gray",
            ColorMode::BlackWhite => "B&W",
        }
    }
}

/// What the device says it can do (from ScannerCapabilities).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Caps {
    pub resolutions: Vec<u32>,
    pub modes: Vec<ColorMode>,
}

impl Caps {
    pub fn parse(xml: &str) -> Caps {
        let mut resolutions: Vec<u32> = all(xml, "scan:XResolution").filter_map(|v| v.parse().ok()).collect();
        resolutions.sort_unstable();
        resolutions.dedup();
        let mut modes = Vec::new();
        for m in all(xml, "scan:ColorMode") {
            let m = match m {
                "RGB24" => ColorMode::Color,
                "Grayscale8" => ColorMode::Gray,
                "BlackAndWhite1" => ColorMode::BlackWhite,
                _ => continue,
            };
            if !modes.contains(&m) {
                modes.push(m);
            }
        }
        // Local B&W only needs gray.
        if modes.contains(&ColorMode::Gray) && !modes.contains(&ColorMode::BlackWhite) {
            modes.push(ColorMode::BlackWhite);
        }
        Caps { resolutions, modes }
    }
}

#[derive(Clone, Copy)]
pub struct ScanOpts {
    pub dpi: u32,
    pub mode: ColorMode,
    pub width: u32,
    pub height: u32,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .http_status_as_error(false)
        .build()
        .into()
}

impl Scanner {
    pub fn at(host: &str) -> Scanner {
        let base = if host.starts_with("http") {
            host.trim_end_matches('/').to_string()
        } else {
            format!("http://{host}/eSCL")
        };
        Scanner { name: host.to_string(), base, agent: agent() }
    }

    /// Browse mDNS for an AirScan scanner.
    pub fn discover(timeout: Duration) -> Result<Scanner> {
        use mdns_sd::{ServiceDaemon, ServiceEvent};
        let mdns = ServiceDaemon::new()?;
        let rx = mdns.browse("_uscan._tcp.local.")?;
        let deadline = std::time::Instant::now() + timeout;
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            let Ok(ev) = rx.recv_timeout(left) else { break };
            if let ServiceEvent::ServiceResolved(info) = ev {
                let ip = info
                    .get_addresses_v4()
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("scanner has no IPv4 address"))?;
                let rs = info.get_property_val_str("rs").unwrap_or("eSCL").trim_matches('/');
                let name = info.get_property_val_str("ty").unwrap_or("scanner").to_string();
                let _ = mdns.shutdown();
                return Ok(Scanner {
                    name,
                    base: format!("http://{ip}:{}/{rs}", info.get_port()),
                    agent: agent(),
                });
            }
        }
        let _ = mdns.shutdown();
        bail!("no AirScan scanner found on the network")
    }

    pub fn host(&self) -> &str {
        self.base.trim_start_matches("http://").split('/').next().unwrap_or("")
    }

    pub fn state(&self) -> Result<String> {
        let mut r = self.agent.get(format!("{}/ScannerStatus", self.base)).call()?;
        let body = r.body_mut().read_to_string()?;
        Ok(tag(&body, "pwg:State").unwrap_or("Unknown").to_string())
    }

    pub fn capabilities(&self) -> Result<Caps> {
        let mut r = self.agent.get(format!("{}/ScannerCapabilities", self.base)).call()?;
        Ok(Caps::parse(&r.body_mut().read_to_string()?))
    }

    /// Run one platen scan and return the JPEG bytes.
    pub fn scan(&self, o: ScanOpts) -> Result<Vec<u8>> {
        let job = self.create_job(o)?;
        let result = self.fetch(&job);
        // Always close the job; otherwise the scanner stays "Processing".
        let _ = self.agent.get(format!("{job}/NextDocument")).call();
        let _ = self.agent.delete(&job).call();
        result
    }

    fn create_job(&self, o: ScanOpts) -> Result<String> {
        let body = settings_xml(o);
        for attempt in 0..20 {
            let r = self
                .agent
                .post(format!("{}/ScanJobs", self.base))
                .header("Content-Type", "text/xml")
                .send(body.as_str())?;
            match r.status().as_u16() {
                201 => {
                    let loc = r
                        .headers()
                        .get("location")
                        .and_then(|v| v.to_str().ok())
                        .context("scanner returned no job location")?;
                    return Ok(self.absolute(loc));
                }
                503 if attempt < 19 => std::thread::sleep(Duration::from_millis(500)),
                s => bail!("scanner refused job (HTTP {s})"),
            }
        }
        bail!("scanner busy")
    }

    fn fetch(&self, job: &str) -> Result<Vec<u8>> {
        for _ in 0..600 {
            let mut r = self.agent.get(format!("{job}/NextDocument")).call()?;
            match r.status().as_u16() {
                200 => {
                    return Ok(r.body_mut().with_config().limit(512 << 20).read_to_vec()?);
                }
                503 => std::thread::sleep(Duration::from_millis(500)),
                s => bail!("scan failed (HTTP {s})"),
            }
        }
        bail!("scan timed out")
    }

    fn absolute(&self, loc: &str) -> String {
        if loc.starts_with("http") {
            return loc.trim_end_matches('/').to_string();
        }
        let origin = self.base.split("/eSCL").next().unwrap_or(&self.base);
        let origin = origin.trim_end_matches('/');
        format!("{origin}/{}", loc.trim_matches('/'))
    }
}

fn settings_xml(o: ScanOpts) -> String {
    let mode = if o.mode == ColorMode::Color { "RGB24" } else { "Grayscale8" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><scan:ScanSettings xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03" xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm"><pwg:Version>2.93</pwg:Version><scan:Intent>Document</scan:Intent><pwg:ScanRegions><pwg:ScanRegion><pwg:Height>{h}</pwg:Height><pwg:Width>{w}</pwg:Width><pwg:XOffset>0</pwg:XOffset><pwg:YOffset>0</pwg:YOffset><pwg:ContentRegionUnits>escl:ThreeHundredthsOfInches</pwg:ContentRegionUnits></pwg:ScanRegion></pwg:ScanRegions><pwg:InputSource>Platen</pwg:InputSource><scan:ColorMode>{mode}</scan:ColorMode><scan:XResolution>{d}</scan:XResolution><scan:YResolution>{d}</scan:YResolution><pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat><scan:DocumentFormatExt>image/jpeg</scan:DocumentFormatExt></scan:ScanSettings>"#,
        h = o.height,
        w = o.width,
        d = o.dpi,
    )
}

/// Text of every `<name>` element, in order.
fn all<'a>(xml: &'a str, name: &str) -> impl Iterator<Item = &'a str> {
    let open = format!("<{name}>");
    let len = open.len();
    let starts: Vec<usize> = xml.match_indices(open.as_str()).map(|(i, _)| i + len).collect();
    starts.into_iter().filter_map(move |start| xml[start..].find('<').map(|end| &xml[start..start + end]))
}

fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find('<')? + start;
    Some(&xml[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_lists_resolutions_and_adds_local_black_and_white() {
        let xml = "<scan:ColorModes><scan:ColorMode>BlackAndWhite1</scan:ColorMode><scan:ColorMode>Grayscale8</scan:ColorMode>\
            <scan:ColorMode>RGB24</scan:ColorMode></scan:ColorModes><scan:DiscreteResolution><scan:XResolution>300</scan:XResolution>\
            </scan:DiscreteResolution><scan:DiscreteResolution><scan:XResolution>100</scan:XResolution></scan:DiscreteResolution>\
            <scan:ColorMode>RGB24</scan:ColorMode><scan:XResolution>300</scan:XResolution>";
        let c = Caps::parse(xml);
        assert_eq!(c.resolutions, [100, 300]);
        assert_eq!(c.modes, [ColorMode::BlackWhite, ColorMode::Gray, ColorMode::Color]);
        assert_eq!(Caps::parse("<scan:ColorMode>Grayscale8</scan:ColorMode>").modes, [ColorMode::Gray, ColorMode::BlackWhite]);
    }
}
