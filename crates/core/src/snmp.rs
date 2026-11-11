//! Just enough SNMP v1 to set one string value: BER-encode a SetRequest, send it over UDP and
//! check the response's error status. Saves users from needing net-snmp tools.
use anyhow::{Context, Result, bail};
use std::net::UdpSocket;
use std::time::Duration;

const SET_REQUEST: u8 = 0xa3;
const GET_RESPONSE: u8 = 0xa2;

/// Set `oid` (dotted) to `value` with a write community; retried once, as the device can drop the first.
pub fn set_string(host: &str, community: &str, oid: &str, value: &str) -> Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_read_timeout(Some(Duration::from_secs(3)))?;
    sock.connect((host, 161)).context("can't reach the device")?;
    let id = std::process::id() as i64 & 0x7fff_ffff;
    let msg = set_request(id, community, oid, value)?;
    let mut buf = [0u8; 1500];
    for _ in 0..2 {
        sock.send(&msg)?;
        if let Ok(n) = sock.recv(&mut buf) {
            return check(&buf[..n]);
        }
    }
    bail!("device didn't answer the SNMP write (is write access on?)")
}

fn set_request(id: i64, community: &str, oid: &str, value: &str) -> Result<Vec<u8>> {
    let bind = tlv(0x30, &[encode_oid(oid)?, tlv(0x04, value.as_bytes())].concat());
    let pdu = tlv(SET_REQUEST, &[int(id), int(0), int(0), tlv(0x30, &bind)].concat());
    Ok(tlv(0x30, &[int(0), tlv(0x04, community.as_bytes()), pdu].concat()))
}

/// Error status of a GetResponse; 0 is success.
fn check(msg: &[u8]) -> Result<()> {
    let (_, outer, _) = read(msg).context("bad SNMP reply")?;
    let (_, _, rest) = read(outer).context("bad SNMP reply")?;
    let (_, _, rest) = read(rest).context("bad SNMP reply")?;
    let (tag, pdu, _) = read(rest).context("bad SNMP reply")?;
    let (_, _, pdu) = read(pdu).context("bad SNMP reply")?;
    let (_, status, _) = read(pdu).context("bad SNMP reply")?;
    let status = status.iter().fold(0i64, |a, b| (a << 8) | *b as i64);
    if tag != GET_RESPONSE || status != 0 {
        bail!("device rejected the SNMP write (status {status})");
    }
    Ok(())
}

/// One TLV: (tag, contents, rest).
fn read(b: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, b) = b.split_first()?;
    let (&first, mut b) = b.split_first()?;
    let len = if first < 0x80 {
        first as usize
    } else {
        let n = (first & 0x7f) as usize;
        let len = b.get(..n)?.iter().fold(0usize, |a, x| (a << 8) | *x as usize);
        b = &b[n..];
        len
    };
    Some((tag, b.get(..len)?, &b[len..]))
}

fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    match body.len() {
        n if n < 0x80 => out.push(n as u8),
        n if n < 0x100 => out.extend([0x81, n as u8]),
        n => out.extend([0x82, (n >> 8) as u8, n as u8]),
    }
    out.extend_from_slice(body);
    out
}

fn int(v: i64) -> Vec<u8> {
    let mut bytes = v.to_be_bytes().to_vec();
    while bytes.len() > 1 && ((bytes[0] == 0 && bytes[1] < 0x80) || (bytes[0] == 0xff && bytes[1] >= 0x80)) {
        bytes.remove(0);
    }
    tlv(0x02, &bytes)
}

fn encode_oid(oid: &str) -> Result<Vec<u8>> {
    let ids: Vec<u64> = oid.trim_start_matches('.').split('.').map(str::parse).collect::<Result<_, _>>().context("bad OID")?;
    let [a, b, rest @ ..] = ids.as_slice() else { bail!("bad OID") };
    let mut body = vec![(a * 40 + b) as u8];
    for &id in rest {
        let mut chunk = vec![(id & 0x7f) as u8];
        let mut v = id >> 7;
        while v > 0 {
            chunk.insert(0, (v & 0x7f) as u8 | 0x80);
            v >>= 7;
        }
        body.extend(chunk);
    }
    Ok(tlv(0x06, &body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_oids_with_multi_byte_ids() {
        assert_eq!(encode_oid("1.3.6.1.2.1.1.1.0").unwrap(), [0x06, 0x08, 0x2b, 6, 1, 2, 1, 1, 1, 0]);
        assert_eq!(encode_oid("1.3.6.1.4.1.2435").unwrap(), [0x06, 0x07, 0x2b, 6, 1, 4, 1, 0x93, 0x03]);
    }

    #[test]
    fn builds_a_set_request() {
        let msg = set_request(1, "internal", "1.3.6.1.2.1.1.5.0", "x").unwrap();
        let expect = [0x30, 0x29, 0x02, 0x01, 0x00, 0x04, 0x08, b'i', b'n', b't', b'e', b'r', b'n', b'a', b'l', 0xa3, 0x1a, 0x02, 0x01, 0x01, 0x02, 0x01, 0x00, 0x02, 0x01, 0x00, 0x30, 0x0f, 0x30, 0x0d, 0x06, 0x08, 0x2b, 6, 1, 2, 1, 1, 5, 0, 0x04, 0x01, b'x'];
        assert_eq!(msg, expect);
    }

    #[test]
    fn reads_the_error_status_of_a_reply() {
        let mut reply = set_request(1, "internal", "1.3.6.1.2.1.1.5.0", "x").unwrap();
        reply[15] = GET_RESPONSE;
        assert!(check(&reply).is_ok());
        reply[22] = 0x02;
        assert!(check(&reply).is_err());
    }
}
