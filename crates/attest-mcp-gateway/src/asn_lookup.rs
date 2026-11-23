//! Offline IP to ASN lookup.
//!
//! The database is a file on disk: a small JSON network table, or a MaxMind
//! MMDB (`.mmdb`) read with the `maxminddb` crate. A lookup never opens a socket.

use std::net::IpAddr;
use std::path::Path;

use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Network {
    network: u32,
    mask: u32,
    prefix: u8,
    cidr: String,
    asn: u32,
    organization: String,
}

#[derive(Debug, Clone)]
pub struct AsnTable {
    networks: Vec<Network>,
}

pub fn respond(ip: &str, db_path: Option<&Path>, demo_mode: bool) -> Result<Value, String> {
    let Some(path) = db_path else {
        if demo_mode {
            return Ok(json!({
                "ip": ip,
                "found": false,
                "asn": null,
                "organization": null,
                "source": "asn_lookup_stub"
            }));
        }
        return Err("asn database is not configured".into());
    };
    lookup_file(path, ip)
}

pub fn lookup_file(path: &Path, ip: &str) -> Result<Value, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("mmdb") {
        return lookup_mmdb_file(path, ip);
    }
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("asn database unreadable: {e}"))?;
    let table = parse_table(&text)?;
    lookup_table(&table, ip)
}

pub fn parse_table(text: &str) -> Result<AsnTable, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("asn database is not json: {e}"))?;
    let networks = value
        .get("networks")
        .and_then(|v| v.as_array())
        .or_else(|| value.as_array())
        .ok_or_else(|| "asn database needs a networks array".to_string())?;
    let mut parsed = Vec::with_capacity(networks.len());
    for net in networks {
        let cidr = net
            .get("cidr")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "asn network is missing cidr".to_string())?;
        let asn = net
            .get("asn")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("asn network {cidr} is missing asn"))?;
        let organization = net
            .get("organization")
            .or_else(|| net.get("org"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let (network, mask, prefix) = parse_cidr(cidr)?;
        parsed.push(Network {
            network,
            mask,
            prefix,
            cidr: cidr.to_string(),
            asn: u32::try_from(asn).map_err(|_| format!("asn for {cidr} does not fit in u32"))?,
            organization,
        });
    }
    Ok(AsnTable { networks: parsed })
}

pub fn lookup_table(table: &AsnTable, ip: &str) -> Result<Value, String> {
    let addr = parse_ipv4(ip.trim())?;
    let mut best: Option<&Network> = None;
    for net in &table.networks {
        if addr & net.mask == net.network && best.is_none_or(|b| net.prefix > b.prefix) {
            best = Some(net);
        }
    }
    Ok(match best {
        Some(net) => json!({
            "ip": ip.trim(),
            "found": true,
            "asn": net.asn,
            "organization": net.organization,
            "prefix": net.cidr,
            "source": "offline_asn_db"
        }),
        None => json!({
            "ip": ip.trim(),
            "found": false,
            "asn": null,
            "organization": null,
            "source": "offline_asn_db"
        }),
    })
}

fn lookup_mmdb_file(path: &Path, ip: &str) -> Result<Value, String> {
    let addr: IpAddr = ip
        .trim()
        .parse()
        .map_err(|_| format!("ip_asn_lookup requires an IP address, got {ip}"))?;
    let reader = maxminddb::Reader::open_readfile(path)
        .map_err(|e| format!("asn mmdb is not readable: {e}"))?;
    let looked = reader
        .lookup(addr)
        .map_err(|e| format!("asn mmdb lookup failed: {e}"))?;
    let record = looked
        .decode::<maxminddb::geoip2::Asn>()
        .map_err(|e| format!("asn mmdb record: {e}"))?;
    let Some(record) = record else {
        return Ok(json!({
            "ip": ip.trim(),
            "found": false,
            "asn": null,
            "organization": null,
            "source": "offline_asn_db"
        }));
    };
    Ok(json!({
        "ip": ip.trim(),
        "found": record.autonomous_system_number.is_some(),
        "asn": record.autonomous_system_number,
        "organization": record.autonomous_system_organization,
        "source": "offline_asn_db"
    }))
}

fn parse_cidr(cidr: &str) -> Result<(u32, u32, u8), String> {
    let (addr, bits) = cidr
        .split_once('/')
        .ok_or_else(|| format!("asn cidr {cidr} needs a prefix length"))?;
    let prefix: u8 = bits
        .parse()
        .map_err(|_| format!("asn cidr {cidr} has a bad prefix"))?;
    if prefix > 32 {
        return Err(format!("asn cidr {cidr} prefix is above 32"));
    }
    let ip = parse_ipv4(addr)?;
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    Ok((ip & mask, mask, prefix))
}

fn parse_ipv4(ip: &str) -> Result<u32, String> {
    if ip.contains(':') {
        return Err("ip_asn_lookup supports IPv4 addresses".into());
    }
    let mut parts = ip.split('.');
    let mut out = 0u32;
    for _ in 0..4 {
        let part = parts
            .next()
            .ok_or_else(|| format!("ip_asn_lookup requires an IPv4 address, got {ip}"))?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("ip_asn_lookup requires an IPv4 address, got {ip}"));
        }
        let octet: u32 = part
            .parse()
            .map_err(|_| format!("ip_asn_lookup requires an IPv4 address, got {ip}"))?;
        if octet > 255 {
            return Err(format!("ip_asn_lookup requires an IPv4 address, got {ip}"));
        }
        out = (out << 8) | octet;
    }
    if parts.next().is_some() {
        return Err(format!("ip_asn_lookup requires an IPv4 address, got {ip}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        r#"{
            "networks": [
                {"cidr": "203.0.113.0/24", "asn": 64500, "organization": "Example Net"},
                {"cidr": "203.0.113.10/32", "asn": 64501, "organization": "Example Host"}
            ]
        }"#
    }

    #[test]
    fn offline_file_lookup_does_not_use_the_network() {
        let dir = std::env::temp_dir().join(format!("attest-asn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("asn.json");
        std::fs::write(&path, sample()).unwrap();
        let hit = lookup_file(&path, "203.0.113.10").unwrap();
        assert_eq!(hit["asn"], 64501);
        assert_eq!(hit["organization"], "Example Host");
        assert_eq!(hit["source"], "offline_asn_db");
        assert!(!crate::demo::is_stub_source(
            hit["source"].as_str().unwrap()
        ));
        let miss = lookup_file(&path, "198.51.100.8").unwrap();
        assert_eq!(miss["found"], false);
        assert_eq!(miss["source"], "offline_asn_db");
        let err = lookup_file(dir.join("missing.json").as_path(), "203.0.113.10").unwrap_err();
        assert!(err.contains("unreadable"), "{err}");
        assert!(!err.contains("http"));
    }

    #[test]
    fn longest_prefix_wins() {
        let table = parse_table(sample()).unwrap();
        let broad = lookup_table(&table, "203.0.113.11").unwrap();
        assert_eq!(broad["asn"], 64500);
        assert_eq!(broad["organization"], "Example Net");
        assert_eq!(broad["prefix"], "203.0.113.0/24");
    }

    #[test]
    fn mmdb_path_is_read_locally_and_rejects_a_bad_file() {
        let dir = std::env::temp_dir().join(format!("attest-asn-mmdb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("asn.mmdb");
        std::fs::write(&path, b"not a maxmind database").unwrap();
        let err = lookup_file(&path, "203.0.113.10").unwrap_err();
        assert!(err.contains("mmdb"), "{err}");
        let lower = err.to_ascii_lowercase();
        assert!(!lower.contains("http"));
        assert!(!lower.contains("dns"));
    }

    #[test]
    fn production_without_a_file_is_not_a_stub() {
        let err = respond("203.0.113.10", None, false).unwrap_err();
        assert!(!err.contains("_stub"), "{err}");
        let stub = respond("203.0.113.10", None, true).unwrap();
        assert!(crate::demo::is_stub_source(
            stub["source"].as_str().unwrap()
        ));
    }
}
