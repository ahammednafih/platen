//! Print head cleaning and the nozzle check, through the device's web UI (IPP has neither).
//!
//! Cleaning posts the picked button to a confirmation page, then that page's Yes. The nozzle check
//! posts straight from its page; a busy device sends the form back instead of starting.
use crate::ipp::Ink;
use crate::webui::{hidden, inputs, page, post};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

const CLEAN_PAGE: &str = "/general/head_cleaning.html";
const CLEAN_CONFIRM: &str = "/general/head_cleaning_confirm.html";
const CLEAN_START: &str = "/general/head_cleaning_confirm.html?pc=12";
const CLEAN_FORM: &str = "action=\"/general/head_cleaning.html\"";
const CONFIRM_FORM: &str = "action=\"/general/head_cleaning_confirm.html\"";
const CHECK_PAGE: &str = "/general/checkprintquality.html";
const CHECK_FORM: &str = "action=\"/general/checkprintquality.html\"";
/// The page warns that cleaning with little ink left can damage the machine; at or below this
/// level Platen asks first.
pub const LOW_INK: i32 = 15;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Inks {
    Black,
    Colour,
    All,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Strength {
    Normal,
    Strong,
    Strongest,
    /// All inks only; uses the most ink.
    Special,
}

/// The device's button number for a clean.
fn button(inks: Inks, strength: Strength) -> Result<u8> {
    let base = match inks {
        Inks::Black => 0,
        Inks::Colour => 3,
        Inks::All => 6,
    };
    Ok(match strength {
        Strength::Normal => base + 1,
        Strength::Strong => base + 2,
        Strength::Strongest => base + 3,
        Strength::Special if inks == Inks::All => 10,
        Strength::Special => bail!("special cleaning is for all inks only"),
    })
}

/// Inks a clean draws from that are at or below [`LOW_INK`].
pub fn low_inks(levels: &[Ink], inks: Inks) -> Vec<Ink> {
    let used = |name: &str| match inks {
        Inks::Black => name.eq_ignore_ascii_case("black"),
        Inks::Colour => !name.eq_ignore_ascii_case("black"),
        Inks::All => true,
    };
    levels.iter().filter(|i| used(&i.name) && (0..=LOW_INK).contains(&i.level)).cloned().collect()
}

/// Start a head clean. It runs on the device for a minute or more; IPP keeps reporting idle.
pub fn clean(host: &str, password: &str, inks: Inks, strength: Strength) -> Result<()> {
    let btn = button(inks, strength)?.to_string();
    let (agent, cookie, html) = page(host, password, CLEAN_PAGE)?;
    let base = format!("https://{host}");
    let pick = hidden(&inputs(button_form(&html, &btn)?, CLEAN_FORM)?);
    let confirm = post(&agent, &cookie, &format!("{base}{CLEAN_CONFIRM}"), &format!("{base}{CLEAN_PAGE}"), pick)?;
    if !confirm.contains(CLEAN_START.rsplit('/').next().unwrap_or_default()) {
        bail!("the printer didn't offer to start cleaning; it may be busy");
    }
    let yes = with_button(hidden(&inputs(&confirm, CONFIRM_FORM)?), &btn);
    post(&agent, &cookie, &format!("{base}{CLEAN_START}"), &format!("{base}{CLEAN_CONFIRM}"), yes).map(drop)
}

/// Print the nozzle check pattern on A4/Letter plain paper from the main tray.
pub fn nozzle_check(host: &str, password: &str) -> Result<()> {
    let (agent, cookie, html) = page(host, password, CHECK_PAGE)?;
    let url = format!("https://{host}{CHECK_PAGE}");
    let reply = post(&agent, &cookie, &url, &url, hidden(&inputs(&html, CHECK_FORM)?))?;
    if !printing(&reply) {
        bail!("the printer is busy; try again once it's finished");
    }
    Ok(())
}

/// Each button is its own form with its own CSRF token.
fn button_form<'a>(html: &'a str, btn: &str) -> Result<&'a str> {
    let wanted = format!("name=\"btn_def\" value=\"{btn}\"");
    let starts = html.match_indices(CLEAN_FORM).map(|(i, _)| i);
    let form = starts.map(|i| &html[i..]).find(|f| f[..f.find("</form>").unwrap_or(f.len())].contains(&wanted));
    form.ok_or_else(|| anyhow::anyhow!("the printer doesn't offer this cleaning"))
}

fn with_button(fields: Vec<(String, String)>, btn: &str) -> Vec<(String, String)> {
    let mut out: Vec<_> = fields.into_iter().filter(|(k, _)| k != "btn_def").collect();
    out.push(("btn_def".into(), btn.into()));
    out
}

/// Accepted, the print state shows a wait message in place of the Print form.
fn printing(reply: &str) -> bool {
    let state = match (reply.find("printstate_start"), reply.rfind("printstate_end")) {
        (Some(a), Some(b)) if a < b => &reply[a..b],
        _ => reply,
    };
    !state.contains(CHECK_FORM)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = r#"<form method="post" action="/general/head_cleaning.html"><input type="hidden" id="pageid" name="pageid" value="21" /><input type="hidden" id="CSRFToken1" name="CSRFToken" value="a
b" /><input type="hidden" name="btn_def" value="1" /><input type="submit" onclick="this.form.action='/general/head_cleaning_confirm.html'" value="Start" /></form><form method="post" action="/general/head_cleaning.html"><input type="hidden" id="pageid" name="pageid" value="21" /><input type="hidden" id="CSRFToken4" name="CSRFToken" value="d" /><input type="hidden" name="btn_def" value="4" /><input type="submit" value="Start" /></form>"#;
    const CONFIRM: &str = r#"<form method="post" action="/general/head_cleaning_confirm.html" ><input type="hidden" id="pageid" name="pageid" value="22" /><input type="hidden" id="CSRFToken3" name="CSRFToken" value="c" /><input type="hidden" name="btn_def" value="1" /><input type="button" onclick="this.form.action='/general/head_cleaning_confirm.html?pc=12'; this.form.submit();" value="Yes" /></form>"#;

    fn pairs(v: &[(String, String)]) -> Vec<(&str, &str)> {
        v.iter().map(|(k, x)| (k.as_str(), x.as_str())).collect()
    }

    #[test]
    fn numbers_the_buttons_like_the_page() {
        assert_eq!(button(Inks::Black, Strength::Normal).unwrap(), 1);
        assert_eq!(button(Inks::Colour, Strength::Strongest).unwrap(), 6);
        assert_eq!(button(Inks::All, Strength::Strong).unwrap(), 8);
        assert_eq!(button(Inks::All, Strength::Special).unwrap(), 10);
        assert!(button(Inks::Black, Strength::Special).is_err());
    }

    #[test]
    fn posts_the_picked_button_then_the_confirmation() {
        let pick = hidden(&inputs(button_form(CLEAN, "4").unwrap(), CLEAN_FORM).unwrap());
        assert_eq!(pairs(&pick), [("pageid", "21"), ("CSRFToken", "d"), ("btn_def", "4")]);
        assert!(button_form(CLEAN, "7").is_err());
        let yes = with_button(hidden(&inputs(CONFIRM, CONFIRM_FORM).unwrap()), "4");
        assert_eq!(pairs(&yes), [("pageid", "22"), ("CSRFToken", "c"), ("btn_def", "4")]);
        assert!(CONFIRM.contains("head_cleaning_confirm.html?pc=12"));
    }

    #[test]
    fn tells_a_started_check_from_a_busy_device() {
        let started = r#"<input type="hidden" name="printstate_start" /> Please&#32;wait. <input type="hidden" name="printstate_end" />"#;
        let busy = r#"<input type="hidden" name="printstate_start" /><form method="post" action="/general/checkprintquality.html"><input type="submit" value="Print" /></form><input type="hidden" name="printstate_end" />"#;
        assert!(printing(started));
        assert!(!printing(busy));
    }

    #[test]
    fn warns_only_about_the_inks_a_clean_uses() {
        let ink = |name: &str, level| Ink { name: name.into(), level };
        let levels = [ink("Black", 8), ink("Cyan", 50), ink("Yellow", 15), ink("Magenta", -3)];
        let names = |v: Vec<Ink>| v.into_iter().map(|i| i.name).collect::<Vec<_>>();
        assert_eq!(names(low_inks(&levels, Inks::Black)), ["Black"]);
        assert_eq!(names(low_inks(&levels, Inks::Colour)), ["Yellow"]);
        assert_eq!(names(low_inks(&levels, Inks::All)), ["Black", "Yellow"]);
    }
}
