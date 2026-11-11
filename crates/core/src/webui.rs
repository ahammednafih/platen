//! Device details from the printer's web UI: counters, refills, error history, Wi-Fi status.
//!
//! Login is HTTPS-only (self-signed cert): POST the password field to the status page; success sets
//! `AuthCookie`, a wrong password redirects to `passerror`. Pages are `h3` groups of `dt`/`dd` pairs and
//! tables; groups that hold form controls are settings, not status, and are skipped.
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::time::Duration;

const KEYCHAIN_SERVICE: &str = "platen-device-web";
/// Service name before the rename; read as a fallback so a saved password survives.
const OLD_KEYCHAIN_SERVICE: &str = "scan-device-web";
const PAGES: [&str; 2] = ["/general/information.html?kind=item", "/net/wireless/wireless.html"];
/// Already shown elsewhere, or vendor branding.
const SKIP_ROWS: [&str; 1] = ["Model Name"];

#[derive(Serialize, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    /// A breakdown of the row above (e.g. Colour / B&W under a total).
    pub sub: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Group {
    pub title: String,
    pub rows: Vec<Row>,
}

#[derive(Debug)]
pub struct WrongPassword;

impl std::fmt::Display for WrongPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("wrong device password")
    }
}

impl std::error::Error for WrongPassword {}

/// Log in and read every status group from the device pages.
pub fn details(host: &str, password: &str) -> Result<Vec<Group>> {
    let agent = agent();
    let base = format!("https://{host}");
    let cookie = login(&agent, &base, password)?;
    PAGES.iter().map(|p| fetch(&agent, &base, p, &cookie).map(|html| parse(&html))).collect::<Result<Vec<_>>>().map(|v| v.into_iter().flatten().collect())
}

pub(crate) fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().disable_verification(true).build();
    let config = ureq::Agent::config_builder().tls_config(tls).max_redirects(0).http_status_as_error(false);
    config.timeout_global(Some(Duration::from_secs(15))).build().into()
}

pub(crate) fn login(agent: &ureq::Agent, base: &str, password: &str) -> Result<String> {
    let page = fetch(agent, base, "/home/status.html", "")?;
    let field = password_field(&page).context("no login form on the device page")?;
    let form = [(field.as_str(), password), ("loginurl", "/home/status.html")];
    let res = agent.post(format!("{base}/home/status.html")).send_form(form).context("device didn't answer")?;
    auth_cookie(res.headers().get_all("set-cookie").iter().filter_map(|v| v.to_str().ok())).ok_or_else(|| WrongPassword.into())
}

pub(crate) fn fetch(agent: &ureq::Agent, base: &str, path: &str, cookie: &str) -> Result<String> {
    let mut res = agent.get(format!("{base}{path}")).header("Cookie", cookie).call().context("device didn't answer")?;
    if res.status().is_redirection() {
        bail!(WrongPassword);
    }
    Ok(res.body_mut().read_to_string()?)
}

/// The password input's name changes between firmware versions, so read it from the form.
fn password_field(html: &str) -> Option<String> {
    let input = &html[html.find("type=\"password\"")?..];
    let name = &input[input.find("name=\"")? + 6..];
    Some(name[..name.find('"')?].to_string())
}

fn auth_cookie<'a>(mut headers: impl Iterator<Item = &'a str>) -> Option<String> {
    headers.find(|h| h.starts_with("AuthCookie=")).map(|h| h.split(';').next().unwrap_or(h).to_string())
}

/// Status groups of a device page, in page order.
pub fn parse(html: &str) -> Vec<Group> {
    html.split("class=\"contentsGroup").skip(1).filter(|g| !has_controls(g)).filter_map(group).collect()
}

fn has_controls(chunk: &str) -> bool {
    ["<input", "<select", "<textarea"].iter().any(|t| chunk.contains(t))
}

fn group(chunk: &str) -> Option<Group> {
    let head = tags(chunk, "dt").into_iter().find(|dt| dt.contains("class=\"head\""));
    let title = between(chunk, "<h3", "</h3>").map(str::to_string).or(head).map(|t| text(&t)).unwrap_or_default();
    let mut rows = pairs(chunk);
    rows.extend(table_rows(chunk));
    rows.retain(|r| !SKIP_ROWS.contains(&r.label.as_str()));
    if rows.iter().all(|r| r.sub) {
        rows.iter_mut().for_each(|r| r.sub = false);
    }
    let title = if title == "Current Status" { "Wi-Fi".into() } else { title };
    (!rows.is_empty()).then_some(Group { title, rows })
}

fn pairs(chunk: &str) -> Vec<Row> {
    let dts = tags(chunk, "dt").into_iter().filter(|dt| !dt.contains("class=\"head\""));
    dts.zip(tags(chunk, "dd")).map(|(dt, dd)| row(&dt, &dd)).collect()
}

fn row(dt: &str, dd: &str) -> Row {
    Row { label: text(dt), value: text(&drop_units(dd)), sub: dt.contains("class=\"subhead\"") }
}

/// Error history: rows of `<th>n</th><td>what</td><td>when</td>`; empty slots are left out.
fn table_rows(chunk: &str) -> Vec<Row> {
    tags(chunk, "tr").iter().map(|tr| tags(tr, "td")).filter(|td| td.len() >= 2 && !text(&td[0]).is_empty()).map(|td| Row { label: text(&td[0]), value: text(&td[1]), sub: false }).collect()
}

/// Each `<tag …>…</tag>` in order, including the opening tag (so attributes stay visible).
fn tags(html: &str, tag: &str) -> Vec<String> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    html.match_indices(&open).filter(|(i, _)| html[i + open.len()..].starts_with(['>', ' '])).filter_map(|(i, _)| between(&html[i..], "", &close).map(str::to_string)).collect()
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = s.find(start)?;
    let len = s[from..].find(end)?;
    Some(&s[from..from + len])
}

fn drop_units(html: &str) -> String {
    match between(html, "<span class=\"unit\"", "</span>") {
        Some(unit) => html.replace(&format!("{unit}</span>"), ""),
        None => html.to_string(),
    }
}

/// Visible text: tags stripped, entities decoded, whitespace collapsed.
pub(crate) fn text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode(s: &str) -> String {
    s.replace("&#32;", " ").replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}

/// Device password from the login Keychain, if saved.
pub fn saved_password(host: &str) -> Option<String> {
    let find = |service| keychain(&["find-generic-password", "-s", service, "-a", host, "-w"]).ok();
    let out = find(KEYCHAIN_SERVICE).or_else(|| find(OLD_KEYCHAIN_SERVICE))?;
    Some(out.trim_end_matches('\n').to_string()).filter(|p| !p.is_empty())
}

pub fn save_password(host: &str, password: &str) -> Result<()> {
    keychain(&["add-generic-password", "-U", "-s", KEYCHAIN_SERVICE, "-a", host, "-w", password]).map(drop)
}

fn keychain(args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("security").args(args).output().context("can't run security")?;
    if !out.status.success() {
        bail!("keychain: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &str = r#"<div class="contentsGroup"><h3>Node&#32;Information</h3><dl class="items"><dt>Model&#32;Name</dt><dd>X&nbsp;Y</dd><dt>Serial&#32;no.</dt><dd>E836</dd></dl></div><div class="contentsGroup"><dl class="items_info_1line"><dt>Page&#32;Counter</dt><dd>64</dd><dt class="subhead"><span>B&amp;W</span></dt><dd>45</dd></dl></div><div class="contentsGroup"><h3>Total&#32;Pages&#32;Scanned</h3><dl class="items"><dt>Flatbed</dt><dd>123<span class="unit">Page(s)</span></dd></dl></div><div class="contentsGroup"><h3>Error&#32;History</h3><table><tbody><tr><th>1</th><td>Paper Tray 1 not detected</td><td>10/11/2026&nbsp;21:16</td></tr><tr><th>2</th><td></td><td></td></tr></tbody></table></div><div class="contentsGroup status"><dl class="items"><dt class="head">Current&#32;Status</dt><dt class="subhead"><span>Channel</span></dt><dd><div>153</div></dd></dl></div><div class="contentsGroup"><h3>Settings</h3><dl><dt>SSID</dt><dd><input name="x" /></dd></dl></div>"#;

    #[test]
    fn parses_status_groups_and_skips_settings_forms() {
        let g = parse(INFO);
        let r = |label: &str, value: &str, sub| Row { label: label.into(), value: value.into(), sub };

        assert_eq!(g.len(), 5);
        assert_eq!(g[0], Group { title: "Node Information".into(), rows: vec![r("Serial no.", "E836", false)] });
        assert_eq!(g[1].rows, vec![r("Page Counter", "64", false), r("B&W", "45", true)]);
        assert_eq!(g[2].rows, vec![r("Flatbed", "123", false)]);
        assert_eq!(g[3].rows, vec![r("Paper Tray 1 not detected", "10/11/2026 21:16", false)]);
        assert_eq!(g[4], Group { title: "Wi-Fi".into(), rows: vec![r("Channel", "153", false)] });
    }

    #[test]
    fn reads_login_field_and_auth_cookie() {
        let form = r#"<input id="login" type="submit" /><input type="password" id="LogBox" name="B17a" />"#;
        let headers = ["Other=1; path=/", "AuthCookie=abc123; path=/; httponly"];

        assert_eq!(password_field(form).as_deref(), Some("B17a"));
        assert_eq!(auth_cookie(headers.into_iter()).as_deref(), Some("AuthCookie=abc123"));
        assert_eq!(auth_cookie(["Other=1"].into_iter()), None);
    }
}
