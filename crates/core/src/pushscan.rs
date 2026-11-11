//! The device's own Scan button ("Scan to PC").
//!
//! The PC registers itself over SNMP and the device then sends a UDP datagram to it when someone
//! scans from the panel. Newer firmware ships SNMP v1/v2c read-only, which silently drops the
//! registration, so write access is switched on through the web UI's SNMP form first, then the
//! device asks to restart and the confirmation is posted too.
//!
//! Panels without a screen scan to the PC picked on the web UI's Scan to PC page, so that is set to
//! this PC once it's registered (the device only lists registered PCs).
use crate::webui;
use anyhow::{Context, Result, bail};
use serde::Serialize;

const SNMP_PAGE: &str = "/net/net/snmp.html";
const SNMP_FORM: &str = "id=\"snmp\"";
/// Mode changes come back as a "restart now?" page; its Yes button posts the page's hidden fields here.
const CONFIRM_FORM: &str = "id=\"pageContents\"";
const CONFIRM_PATH: &str = "/net/net/snmp.html?pc=2";
const S2PC_PAGE: &str = "/scan/scantopc.html";
const S2PC_FORM: &str = "action=\"/scan/scantopc.html\"";
/// Profile select: "Default" (6) doesn't reach the PC on some firmware, Image (2) does.
const PROFILE_DEFAULT: &str = "6";
const PROFILE_IMAGE: &str = "2";
/// Hidden flag the page's script sets when the mode changes; the device restarts its network.
const RESTART_FIELD: &str = "B1af";

#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
pub enum SnmpMode {
    ReadWrite,
    ReadOnly,
    Off,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct SnmpAccess {
    pub mode: SnmpMode,
    /// Community the device accepts SNMP writes from.
    pub set_community: String,
}

/// An `<input>` in the SNMP form, with the text right after it (radio labels).
#[derive(Debug)]
struct Input {
    kind: String,
    name: String,
    value: String,
    checked: bool,
    label: String,
}

pub fn snmp_access(host: &str, password: &str) -> Result<SnmpAccess> {
    let (_, _, html) = page(host, password, SNMP_PAGE)?;
    access(&inputs(&html, SNMP_FORM)?)
}

/// Switch SNMP v1/v2c to read-write. The device restarts, so it drops off for about a minute.
pub fn enable_snmp_write(host: &str, password: &str) -> Result<()> {
    let (agent, cookie, html) = page(host, password, SNMP_PAGE)?;
    // Always submit: a saved change only takes effect after the restart, which may have been declined.
    let fields = inputs(&html, SNMP_FORM)?;
    let page = format!("https://{host}{SNMP_PAGE}");
    let confirm = post(&agent, &cookie, &page, &page, form_with_write(&fields)?)?;
    if !confirm.contains("restart") {
        bail!("device didn't accept the change");
    }
    let fields = inputs(&confirm, CONFIRM_FORM)?;
    post(&agent, &cookie, &format!("https://{host}{CONFIRM_PATH}"), &page, hidden(&fields)).map(drop)
}

/// The device 403s the restart confirmation without a Referer from its own pages.
fn post(agent: &ureq::Agent, cookie: &str, url: &str, referer: &str, form: Vec<(String, String)>) -> Result<String> {
    let req = agent.post(url).header("Cookie", cookie).header("Referer", referer);
    let mut res = req.send_form(form).context("device didn't answer")?;
    if !(res.status().is_success() || res.status().is_redirection()) {
        bail!("device refused the change ({})", res.status());
    }
    Ok(res.body_mut().read_to_string().unwrap_or_default())
}

fn hidden(fields: &[Input]) -> Vec<(String, String)> {
    fields.iter().filter(|f| f.kind == "hidden" && !f.name.is_empty()).map(|f| (f.name.clone(), f.value.clone())).collect()
}

/// The PC the device's Scan button sends to, if it's a network PC (not USB).
pub fn destination(host: &str, password: &str) -> Result<Option<String>> {
    let (_, _, html) = page(host, password, S2PC_PAGE)?;
    Ok(picked_pc(&pc_selects(&html)?))
}

/// Point the Scan button at `name`, which must already be registered.
pub fn set_destination(host: &str, password: &str, name: &str) -> Result<()> {
    let (agent, cookie, html) = page(host, password, S2PC_PAGE)?;
    let form = destination_form(&html, name)?;
    let url = format!("https://{host}{S2PC_PAGE}");
    post(&agent, &cookie, &url, &url, form).map(drop)
}

fn destination_form(html: &str, name: &str) -> Result<Vec<(String, String)>> {
    let mut form = hidden(&inputs(html, S2PC_FORM)?);
    let all = selects(html, S2PC_FORM)?;
    if let Some(profile) = all.iter().find(|s| !s.options.iter().any(|o| o.pc)) {
        let v = profile.options.iter().find(|o| o.selected).map_or(PROFILE_IMAGE, |o| o.value.as_str());
        form.push((profile.name.clone(), if v == PROFILE_DEFAULT { PROFILE_IMAGE } else { v }.to_string()));
    }
    let pcs: Vec<&Select> = all.iter().filter(|s| s.options.iter().any(|o| o.pc)).collect();
    if !pcs.iter().any(|s| s.options.iter().any(|o| o.label == name)) {
        bail!("the device doesn't list “{name}” yet; it shows up once the Scan button is registered");
    }
    form.extend(pcs.iter().map(|s| (s.name.clone(), name.to_string())));
    Ok(form)
}

/// Of the two PC lists only the second carries the selection; take whichever has one.
fn picked_pc(pcs: &[Select]) -> Option<String> {
    pcs.iter().find_map(|s| s.options.iter().find(|o| o.selected)).filter(|o| o.value != "0").map(|o| o.label.clone())
}

fn pc_selects(html: &str) -> Result<Vec<Select>> {
    Ok(selects(html, S2PC_FORM)?.into_iter().filter(|s| s.options.iter().any(|o| o.pc)).collect())
}

#[derive(Debug)]
struct Select {
    name: String,
    options: Vec<Opt>,
}

#[derive(Debug)]
struct Opt {
    value: String,
    label: String,
    selected: bool,
    /// Lists a scan-to-PC destination (USB or a registered PC).
    pc: bool,
}

fn selects(html: &str, marker: &str) -> Result<Vec<Select>> {
    let form = form_of(html, marker)?;
    Ok(form.split("<select").skip(1).map(|chunk| {
        let chunk = &chunk[..chunk.find("</select>").unwrap_or(chunk.len())];
        let tag = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
        Select { name: attr(tag, "name"), options: chunk.split("<option").skip(1).map(option).collect() }
    }).collect())
}

fn option(chunk: &str) -> Opt {
    let end = chunk.find('>').unwrap_or(chunk.len());
    let tag = &chunk[..end];
    let label = unescape(webui::text(&chunk[(end + 1).min(chunk.len())..]).trim());
    Opt { value: unescape(&attr(tag, "value")), label, selected: tag.contains(" selected"), pc: tag.contains("type=\"PCScan\"") }
}

/// Numeric entities (`&#77;`) the device uses for PC names.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("&#") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        match tail.find(';').and_then(|j| tail[..j].parse::<u32>().ok().and_then(char::from_u32).map(|c| (c, j))) {
            Some((c, j)) => {
                out.push(c);
                rest = &tail[j + 1..];
            }
            None => {
                out.push_str("&#");
                rest = tail;
            }
        }
    }
    out + rest
}

fn form_of<'a>(html: &'a str, marker: &str) -> Result<&'a str> {
    let start = html.find(marker).context("no settings form on the device page")?;
    let form = &html[start..];
    Ok(&form[..form.find("</form>").unwrap_or(form.len())])
}

fn page(host: &str, password: &str, path: &str) -> Result<(ureq::Agent, String, String)> {
    let agent = webui::agent();
    let base = format!("https://{host}");
    let cookie = webui::login(&agent, &base, password)?;
    let html = webui::fetch(&agent, &base, path, &cookie)?;
    Ok((agent, cookie, html))
}

fn access(fields: &[Input]) -> Result<SnmpAccess> {
    let mode = fields.iter().find(|f| f.kind == "radio" && f.checked).map(|f| mode_of(&f.label)).context("no SNMP mode on the device page")?;
    let set_community = fields.iter().filter(|f| f.kind == "text").nth(1).map(|f| f.value.clone()).unwrap_or_else(|| "internal".into());
    Ok(SnmpAccess { mode, set_community })
}

fn mode_of(label: &str) -> SnmpMode {
    let l = label.to_lowercase();
    if l.contains("read-write") {
        SnmpMode::ReadWrite
    } else if l.contains("read-only") {
        SnmpMode::ReadOnly
    } else {
        SnmpMode::Off
    }
}

/// The form as the browser would send it, with the read-write mode picked and the restart flag set.
fn form_with_write(fields: &[Input]) -> Result<Vec<(String, String)>> {
    let rw = fields.iter().find(|f| f.kind == "radio" && mode_of(&f.label) == SnmpMode::ReadWrite).context("no read-write option on the device page")?;
    let mut out: Vec<(String, String)> = fields
        .iter()
        .filter(|f| !f.name.is_empty() && matches!(f.kind.as_str(), "hidden" | "text" | "password") && f.name != RESTART_FIELD)
        .map(|f| (f.name.clone(), f.value.clone()))
        .collect();
    out.push((rw.name.clone(), rw.value.clone()));
    out.push((RESTART_FIELD.into(), "1".into()));
    Ok(out)
}

fn inputs(html: &str, marker: &str) -> Result<Vec<Input>> {
    let form = form_of(html, marker)?;
    Ok(form.match_indices("<input").map(|(i, _)| input(&form[i..])).collect())
}

fn input(s: &str) -> Input {
    let end = s.find('>').unwrap_or(s.len());
    let (tag, rest) = (&s[..end], &s[end + 1.min(s.len() - end)..]);
    let label = webui::text(&rest[..rest.find("<input").unwrap_or(rest.len())]);
    Input { kind: attr(tag, "type"), name: attr(tag, "name"), value: attr(tag, "value").replace('\n', ""), checked: tag.contains(" checked"), label }
}

fn attr(tag: &str, name: &str) -> String {
    let key = format!(" {name}=\"");
    tag.find(&key).map(|i| &tag[i + key.len()..]).and_then(|v| v.find('"').map(|j| v[..j].to_string())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<form method="post" action="/home/status.html"><input type="hidden" name="CSRFToken" value="login" /></form><form method="post" action="/net/net/snmp.html" id="snmp" onsubmit="f()"><div><input type="hidden" id="pageid" name="pageid" value="424" /></div><input type="hidden" id="CSRFToken1" name="CSRFToken" value="ab
cd" /><input type="hidden" id="pc" name="pc" value="1" /><dl><dd><label class="radioSimple"><input type="radio" name="B177" value="1" onclick="g()" /> SNMP v1/v2c read-write access</label><label class="radioSimple"><input type="radio" name="B177" value="2" onclick="g()" checked="checked" />SNMP v1/v2c read-only access</label><label class="radioSimple"><input type="radio" name="B177" value="3" />Disabled</label></dd><dt>Get</dt><dd><input type="text" id="B11a" name="B11a" maxlength="32" value="public" /></dd><dt>Set</dt><dd><input type="text" id="B11b" name="B11b" maxlength="32" value="internal" /><input type="button" onclick="d()" value="Set to Default" /><input type="hidden" id="B1af" name="B1af" value="0" /></dd></dl><input type="submit" value="Submit" /></form>"#;

    #[test]
    fn confirms_the_restart_with_the_hidden_fields() {
        let page = r#"<div id="pageContents"><form action="" method="post"><input type="hidden" id="pageid" name="pageid" value="424" /><input type="hidden" name="CSRFToken" value="x
y" /><input type="hidden" value="2" name="pc" /><input type="button" value="Yes" /></form></div>"#;
        let form = hidden(&inputs(page, CONFIRM_FORM).unwrap());
        let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(pairs, [("pageid", "424"), ("CSRFToken", "xy"), ("pc", "2")]);
    }

    const S2PC: &str = r#"<form method="post" action="/scan/scantopc.html"><input type="hidden" id="pageid" name="pageid" value="108" /><input type="hidden" name="CSRFToken" value="t" /><input type="hidden" name="postif_registration_reject" value="1" /><select id="B6e" name="B6e" onchange="x();" ><option value="6" selected="selected">Default</option><option value="2">Image</option></select><select id="B6f" name="B6f"><option value="0"  type="PCScan">USB</option><option value="&#77;&#97;&#99;"  type="PCScan">&#77;&#97;&#99;</option><option value="-1"  type="WSScan">--</option></select><select id="B70" name="B70"><option value="0"  selected="selected" type="PCScan">USB</option></select><input type="submit" value="Submit" /></form>"#;

    #[test]
    fn points_the_scan_button_at_this_pc() {
        let form = destination_form(S2PC, "Mac").unwrap();
        let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(pairs, [("pageid", "108"), ("CSRFToken", "t"), ("postif_registration_reject", "1"), ("B6e", "2"), ("B6f", "Mac"), ("B70", "Mac")]);
    }

    #[test]
    fn refuses_a_pc_the_device_doesnt_list() {
        assert!(destination_form(S2PC, "Other").is_err());
    }

    #[test]
    fn reads_the_current_destination_from_the_list_that_has_one() {
        assert_eq!(picked_pc(&pc_selects(S2PC).unwrap()), None);
        let here = S2PC.replace(r#"<option value="0"  selected="selected" type="PCScan">USB</option></select>"#, r#"<option value="0" type="PCScan">USB</option><option value="Mac" selected="selected" type="PCScan">Mac</option></select>"#);
        assert_eq!(picked_pc(&pc_selects(&here).unwrap()), Some("Mac".into()));
    }

    #[test]
    fn reads_mode_and_set_community() {
        let a = access(&inputs(PAGE, SNMP_FORM).unwrap()).unwrap();
        assert_eq!(a, SnmpAccess { mode: SnmpMode::ReadOnly, set_community: "internal".into() });
    }

    #[test]
    fn posts_the_form_with_read_write_and_restart() {
        let form = form_with_write(&inputs(PAGE, SNMP_FORM).unwrap()).unwrap();
        let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(pairs, [("pageid", "424"), ("CSRFToken", "abcd"), ("pc", "1"), ("B11a", "public"), ("B11b", "internal"), ("B177", "1"), ("B1af", "1")]);
    }
}
