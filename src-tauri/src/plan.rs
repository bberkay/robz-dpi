//! Ayarlardan motor isteğine: hangi bayraklar, hangi alan adları, hangi DNS.
//!
//! `EngineRequest` hem arayüz sürecinde (ayarlardan üretilir) hem yönetici yardımcısında (dosyadan okunur ve
//! yeniden doğrulanır) kullanılır. Yardımcı, komut satırını bu yapıdan kendisi kurar: dışarıdan serbest bayrak
//! ya da yol kabul edilmez.

use crate::catalog;
use crate::error::{DpiError, DpiResult};
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

pub const MAX_DOMAINS: usize = 2000;
const MAX_HOST_LEN: usize = 253;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsTarget {
    pub v4: String,
    pub port: u16,
    pub v6: Option<String>,
    pub port6: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngineRequest {
    pub method: String,
    /// None: DNS'e dokunma.
    pub dns: Option<DnsTarget>,
    /// None: bütün sitelere uygula. Some: yalnız bu alan adlarına (ve alt alan adlarına).
    pub domains: Option<Vec<String>>,
}

impl EngineRequest {
    pub fn from_settings(settings: &Settings) -> DpiResult<Self> {
        let domains = if settings.scope == "all" {
            None
        } else {
            let mut set = BTreeSet::new();
            for id in &settings.services {
                if let Some(service) = catalog::service(id) {
                    set.extend(service.domains.iter().map(|d| d.to_string()));
                }
            }
            set.extend(settings.custom_domains.iter().cloned());
            if set.is_empty() {
                return Err(DpiError::new("nothing-selected"));
            }
            Some(set.into_iter().collect())
        };

        let dns = if !settings.dns_enabled {
            None
        } else if settings.dns_provider == catalog::CUSTOM_DNS {
            if settings.dns_custom_addr.parse::<Ipv4Addr>().is_err() || settings.dns_custom_port == 0 {
                return Err(DpiError::new("bad-dns"));
            }
            Some(DnsTarget { v4: settings.dns_custom_addr.clone(), port: settings.dns_custom_port, v6: None, port6: 0 })
        } else {
            let provider = catalog::dns(&settings.dns_provider).ok_or_else(|| DpiError::new("bad-dns"))?;
            Some(DnsTarget {
                v4: provider.v4.to_string(),
                port: provider.port,
                v6: Some(provider.v6.to_string()),
                port6: provider.port6,
            })
        };

        let request = Self { method: settings.method.clone(), dns, domains };
        request.validate()?;
        Ok(request)
    }

    /// Yönetici yardımcısının güvendiği tek denetim: bilinen yöntem, gerçek IP adresleri, geçerli alan adları.
    pub fn validate(&self) -> DpiResult<()> {
        if catalog::method(&self.method).is_none() {
            return Err(DpiError::with("bad-request", "method"));
        }
        if let Some(dns) = &self.dns {
            if dns.v4.parse::<Ipv4Addr>().is_err() || dns.port == 0 {
                return Err(DpiError::new("bad-dns"));
            }
            if let Some(v6) = &dns.v6 {
                if v6.parse::<Ipv6Addr>().is_err() || dns.port6 == 0 {
                    return Err(DpiError::new("bad-dns"));
                }
            }
        }
        if let Some(domains) = &self.domains {
            if domains.is_empty() {
                return Err(DpiError::new("nothing-selected"));
            }
            if domains.len() > MAX_DOMAINS {
                return Err(DpiError::new("too-many-domains"));
            }
            for domain in domains {
                if normalize_domain(domain).as_deref() != Some(domain.as_str()) {
                    return Err(DpiError::with("bad-request", "domain"));
                }
            }
        }
        Ok(())
    }

    /// goodbyedpi.exe bayrakları. `blacklist`: alan adı listesinin yazılacağı dosya (yalnız liste varsa kullanılır).
    pub fn args(&self, blacklist: &Path) -> Vec<String> {
        let mut args: Vec<String> =
            catalog::method(&self.method).map(|m| m.args.iter().map(|a| a.to_string()).collect()).unwrap_or_default();
        if self.domains.is_some() {
            args.push("--blacklist".into());
            args.push(blacklist.to_string_lossy().into_owned());
        }
        if let Some(dns) = &self.dns {
            args.extend(["--dns-addr".to_string(), dns.v4.clone(), "--dns-port".to_string(), dns.port.to_string()]);
            if let Some(v6) = &dns.v6 {
                args.extend(["--dnsv6-addr".to_string(), v6.clone(), "--dnsv6-port".to_string(), dns.port6.to_string()]);
            }
        }
        args
    }

    /// Alan adı dosyasının içeriği: her satırda bir alan adı.
    pub fn blacklist_text(&self) -> Option<String> {
        self.domains.as_ref().map(|domains| {
            let mut text = domains.join("\n");
            text.push('\n');
            text
        })
    }

    /// İstek ile motor dosyalarının parmak izi. Uygulanan ayar ile şimdiki ayar aynı mı diye karşılaştırılır.
    pub fn fingerprint(&self, engine_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(engine_id.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Kullanıcının yazdığını alan adına çevirir: "https://www.Example.com/yol?x=1" -> "example.com".
/// Alan adı değilse (IP adresi, tek kelime, geçersiz karakter) None.
pub fn normalize_domain(input: &str) -> Option<String> {
    let mut text = input.trim().to_lowercase();
    if let Some(rest) = text.split_once("://").map(|(_, rest)| rest.to_string()) {
        text = rest;
    }
    // yol, sorgu, parça
    if let Some(end) = text.find(['/', '?', '#']) {
        text.truncate(end);
    }
    // kullanıcı adı ve bağlantı noktası
    if let Some((_, host)) = text.rsplit_once('@') {
        text = host.to_string();
    }
    if let Some((host, port)) = text.rsplit_once(':') {
        if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            text = host.to_string();
        }
    }
    let mut text = text.trim_matches('.').to_string();
    for prefix in ["*.", "www."] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.to_string();
        }
    }
    if !text.is_ascii() {
        text = idna::domain_to_ascii(&text).ok()?;
    }
    if text.is_empty() || text.len() > MAX_HOST_LEN || !text.contains('.') {
        return None;
    }
    let labels: Vec<&str> = text.split('.').collect();
    let valid_label = |label: &&str| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    if !labels.iter().all(valid_label) {
        return None;
    }
    // Son parça tümüyle rakamsa bu bir IP adresidir.
    if labels.last().is_some_and(|tld| tld.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some(text)
}

/// Boşluk, virgül, noktalı virgül ya da satır sonuyla ayrılmış girdiyi (geçerli, geçersiz) diye ayırır.
pub fn parse_domains(text: &str) -> (Vec<String>, Vec<String>) {
    let mut ok = Vec::new();
    let mut rejected = Vec::new();
    for part in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter(|p| !p.is_empty()) {
        match normalize_domain(part) {
            Some(domain) if !ok.contains(&domain) => ok.push(domain),
            Some(_) => {}
            None => rejected.push(part.chars().take(80).collect()),
        }
    }
    (ok, rejected)
}

/// Windows komut satırı: program yolu ve bağımsız değişkenler, CommandLineToArgvW kurallarıyla tırnaklanır.
pub fn command_line(exe: &Path, args: &[String]) -> String {
    let mut line = quote_arg(&exe.to_string_lossy());
    for arg in args {
        line.push(' ');
        line.push_str(&quote_arg(arg));
    }
    line
}

pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn blacklist() -> PathBuf {
        PathBuf::from(r"C:\Program Files\RobzDPI\blacklist.txt")
    }

    #[test]
    fn normalizes_what_people_type() {
        let cases = [
            ("discord.com", Some("discord.com")),
            ("  https://www.Example.com/path?x=1#y ", Some("example.com")),
            ("http://user:pw@sub.example.co.uk:8443/", Some("sub.example.co.uk")),
            ("*.rbxcdn.com", Some("rbxcdn.com")),
            ("example.com.", Some("example.com")),
            ("çiçek.com", Some("xn--iek-1lab.com")),
            ("localhost", None),
            ("192.168.1.1", None),
            ("exa mple.com", None),
            ("-bad.com", None),
            ("a..b.com", None),
            ("", None),
            ("--blacklist", None),
            ("ex\"ample.com", None),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize_domain(input).as_deref(), expected, "{input:?}");
        }
    }

    #[test]
    fn splits_pasted_lists() {
        let (ok, rejected) = parse_domains("discord.com, https://roblox.com\nnope  discord.com;x.org");
        assert_eq!(ok, ["discord.com", "roblox.com", "x.org"]);
        assert_eq!(rejected, ["nope"]);
    }

    #[test]
    fn default_settings_target_only_discord_and_roblox() {
        let request = EngineRequest::from_settings(&Settings::default()).unwrap();
        let domains = request.domains.clone().unwrap();
        assert!(domains.contains(&"discord.com".to_string()));
        assert!(domains.contains(&"roblox.com".to_string()));
        assert!(domains.windows(2).all(|w| w[0] < w[1]), "sıralı ve yinelenmesiz olmalı");

        let args = request.args(&blacklist());
        assert_eq!(
            args,
            [
                "-5",
                "--set-ttl",
                "5",
                "--blacklist",
                r"C:\Program Files\RobzDPI\blacklist.txt",
                "--dns-addr",
                "9.9.9.9",
                "--dns-port",
                "9953",
                "--dnsv6-addr",
                "2620:fe::fe",
                "--dnsv6-port",
                "9953",
            ]
        );
        assert!(request.blacklist_text().unwrap().ends_with("\n"));
    }

    #[test]
    fn scope_modes() {
        let mut s = Settings { scope: "all".into(), ..Settings::default() };
        let all = EngineRequest::from_settings(&s).unwrap();
        assert_eq!(all.domains, None);
        assert!(!all.args(&blacklist()).contains(&"--blacklist".to_string()));
        assert_eq!(all.blacklist_text(), None);

        s.scope = "selected".into();
        s.services = vec!["discord".into()];
        let discord = EngineRequest::from_settings(&s).unwrap().domains.unwrap();
        assert!(discord.iter().all(|d| !d.contains("roblox") && !d.contains("rbx")));

        s.services.clear();
        s.custom_domains = vec!["example.com".into()];
        assert_eq!(EngineRequest::from_settings(&s).unwrap().domains.unwrap(), ["example.com"]);

        s.custom_domains.clear();
        assert!(EngineRequest::from_settings(&s).unwrap_err().is("nothing-selected"));
    }

    #[test]
    fn dns_choices() {
        let mut s = Settings { dns_enabled: false, ..Settings::default() };
        let none = EngineRequest::from_settings(&s).unwrap();
        assert!(!none.args(&blacklist()).iter().any(|a| a.starts_with("--dns")));

        s.dns_enabled = true;
        s.dns_provider = "custom".into();
        assert!(EngineRequest::from_settings(&s).unwrap_err().is("bad-dns"));
        s.dns_custom_addr = "8.8.8.8".into();
        s.dns_custom_port = 53;
        let custom = EngineRequest::from_settings(&s).unwrap().args(&blacklist());
        assert!(custom.ends_with(&["--dns-addr".into(), "8.8.8.8".into(), "--dns-port".into(), "53".into()]));
    }

    #[test]
    fn helper_rejects_tampered_requests() {
        let good = EngineRequest::from_settings(&Settings::default()).unwrap();

        let mut bad = good.clone();
        bad.method = "-5 --blacklist C:\\evil".into();
        assert!(bad.validate().is_err());

        let mut bad = good.clone();
        bad.domains = Some(vec!["discord.com\" --dns-addr 6.6.6.6".into()]);
        assert!(bad.validate().is_err());

        let mut bad = good.clone();
        bad.dns.as_mut().unwrap().v4 = "9.9.9.9 --foo".into();
        assert!(bad.validate().is_err());

        let mut bad = good.clone();
        bad.domains = Some(vec![]);
        assert!(bad.validate().is_err());

        assert!(good.validate().is_ok());
    }

    #[test]
    fn fingerprint_follows_settings_and_engine() {
        let mut s = Settings::default();
        let a = EngineRequest::from_settings(&s).unwrap();
        assert_eq!(a.fingerprint("e1"), a.clone().fingerprint("e1"));
        assert_ne!(a.fingerprint("e1"), a.fingerprint("e2"));
        s.method = "m4".into();
        assert_ne!(a.fingerprint("e1"), EngineRequest::from_settings(&s).unwrap().fingerprint("e1"));
        // Arayüz ayarları (dil, tepsi) motoru etkilemez.
        let mut ui = Settings::default();
        ui.language = "en".into();
        ui.close_to_tray = !ui.close_to_tray;
        assert_eq!(a.fingerprint("e1"), EngineRequest::from_settings(&ui).unwrap().fingerprint("e1"));
    }

    #[test]
    fn quotes_windows_command_lines() {
        let exe = PathBuf::from(r"C:\Program Files\RobzDPI\engine\goodbyedpi.exe");
        let line = command_line(&exe, &["-5".into(), "--blacklist".into(), r"C:\Program Files\RobzDPI\blacklist.txt".into()]);
        assert_eq!(
            line,
            r#""C:\Program Files\RobzDPI\engine\goodbyedpi.exe" -5 --blacklist "C:\Program Files\RobzDPI\blacklist.txt""#
        );
        assert_eq!(quote_arg(r"C:\dir with space\"), r#""C:\dir with space\\""#);
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote_arg(""), r#""""#);
    }
}
