//! Pure network-list parsing.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub security: String,
    pub signal: u8,
    pub connected: bool,
    pub known: bool,
}

/// iwd's detail format escapes control characters inside SSIDs, not separators.
pub fn parse_scanlist(output: &str) -> Vec<Network> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let ssid = unescape(fields.next()?);
            let security = fields.next()?;
            let signal = fields.next()?.parse::<u8>().ok()?;
            let connected = fields.next()?;
            let known = fields.next()?;
            if fields.next().is_some()
                || ssid.is_empty()
                || signal > 100
                || !matches!(connected, "0" | "1")
                || !matches!(known, "0" | "1")
            {
                return None;
            }
            Some(Network {
                ssid,
                security: security.into(),
                signal,
                connected: connected == "1",
                known: known == "1",
            })
        })
        .collect()
}
fn unescape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// WPA passphrase (8-63 bytes) or raw 64-digit hex key.
pub fn wpa_valid(key: &str) -> bool {
    (8..=63).contains(&key.len()) || (key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanlist_detail_keeps_ssid_and_ignores_malformed_records() {
        let output = "Café 5G\twpa2\t78\t1\t0\ninvalid\nGuest\topen\t52\t0\t1\n";
        let networks = parse_scanlist(output);
        assert_eq!(networks.len(), 2);
        assert_eq!(networks[0].ssid, "Café 5G");
        assert!(networks[0].connected);
        assert!(!networks[0].known);
        assert!(networks[1].known);
    }
    #[test]
    fn wpa_accepts_passphrases_and_hex_keys_only() {
        assert!(!wpa_valid("short"));
        assert!(wpa_valid("longenough"));
        assert!(wpa_valid(&"a".repeat(64)));
        assert!(!wpa_valid(&"z".repeat(64)));
    }
}
