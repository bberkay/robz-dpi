//! Ayarlardan motor isteğine: hangi strateji, hangi alan adları, hangi DNS.
//!
//! `EngineRequest` üç yerde kullanılır: arayüz sürecinde ayarlardan üretilir, yönetici yardımcısında dosyadan
//! okunup yeniden doğrulanır, motor (Windows servisi) onu engine.json'dan okuyup bir kez daha doğrular.
//! Serbest metin ya da komut satırı bayrağı yoktur: her alan sayı, açık/kapalı ya da sınırlı bir seçenektir.

use crate::catalog;
use crate::error::{DpiError, DpiResult};
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

pub const MAX_DOMAINS: usize = 2000;
const MAX_HOST_LEN: usize = 253;
/// Bir paketi bölebileceğimiz en uzak nokta (tek bir Ethernet çerçevesine sığan yük).
pub const MAX_SPLIT: u16 = 1200;

/// Motorun bir bağlantıya ne yapacağı. Hazır yöntemler (catalog::METHODS) bu yapının dört ayrı değeridir;
/// "Gelişmiş" ekranı aynı yapıyı alan alan düzenler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Strategy {
    /// TLS ClientHello'yu bu kadar bayttan sonra iki pakete böl (0: bölme).
    pub split: u16,
    /// Alan adı pakette görünüyorsa tam alan adının önünden böl (`split` yerine).
    pub split_at_sni: bool,
    /// İkinci parçayı önce gönder.
    pub reverse: bool,
    /// Şifresiz HTTP isteğini bu kadar bayttan sonra böl (0: bölme).
    pub http_split: u16,
    /// Sunucuya ulaşmadan ölen sahte paket: "off" | "fixed" (fake_ttl kadar sıçrama) | "auto" (uzaklığa göre)
    pub fake_ttl_mode: String,
    pub fake_ttl: u8,
    /// Sağlama toplamı bozuk sahte paket (sunucu atar).
    pub fake_bad_checksum: bool,
    /// Sıra numarası geçmişte kalan sahte paket (sunucu yok sayar).
    pub fake_bad_seq: bool,
    /// Her sahte paketin kaç kez gönderileceği.
    pub fake_repeats: u8,
    /// Sahte paketin içinde görünen, engelli olmayan alan adı.
    pub fake_host: String,
    /// Hedeflere giden QUIC (HTTP/3) bağlantısını düşür; tarayıcı TCP'ye döner.
    pub block_quic: bool,
    /// ClientHello'da alan adı görünmüyorsa (büyük ClientHello iki pakete bölünmüşse):
    /// "skip" (dokunma) | "known" (hedef olduğu DNS'ten bilinen adreslerde uygula) | "always"
    pub no_sni: String,
}

impl Default for Strategy {
    /// Yöntem 1: böl (2 bayt, ters sırayla) + 5 sıçramada ölen sahte paket.
    fn default() -> Self {
        Self {
            split: 2,
            split_at_sni: false,
            reverse: true,
            http_split: 2,
            fake_ttl_mode: "fixed".into(),
            fake_ttl: 5,
            fake_bad_checksum: false,
            fake_bad_seq: false,
            fake_repeats: 1,
            fake_host: "www.w3.org".into(),
            block_quic: false,
            no_sni: "known".into(),
        }
    }
}

impl Strategy {
    pub fn validate(&self) -> DpiResult<()> {
        let bad = |field: &str| Err(DpiError::with("bad-strategy", field));
        if self.split > MAX_SPLIT {
            return bad("split");
        }
        if self.http_split > MAX_SPLIT {
            return bad("httpSplit");
        }
        if !matches!(self.fake_ttl_mode.as_str(), "off" | "fixed" | "auto") {
            return bad("fakeTtlMode");
        }
        if self.fake_ttl_mode == "fixed" && !(1..=64).contains(&self.fake_ttl) {
            return bad("fakeTtl");
        }
        if !(1..=10).contains(&self.fake_repeats) {
            return bad("fakeRepeats");
        }
        if host_of(&self.fake_host).as_deref() != Some(self.fake_host.as_str()) {
            return bad("fakeHost");
        }
        if !matches!(self.no_sni.as_str(), "skip" | "known" | "always") {
            return bad("noSni");
        }
        if !self.splits() && !self.fakes() {
            // Hiçbir şey yapmayan strateji: motor boşuna çalışır.
            return Err(DpiError::new("empty-strategy"));
        }
        Ok(())
    }

    pub fn splits(&self) -> bool {
        self.split > 0 || self.split_at_sni
    }

    pub fn fakes(&self) -> bool {
        self.fake_ttl_mode != "off" || self.fake_bad_checksum || self.fake_bad_seq
    }

    /// "Gelişmiş" ekranından gelen tek alanlık değişiklik. Bilinmeyen alan ya da yanlış tür: false.
    /// Aralık denetimi `validate`'tedir; burada yalnız tür ve kaba sınır bakılır.
    pub fn set(&mut self, field: &str, value: &Value) -> bool {
        let number = |max: u64| value.as_u64().filter(|n| *n <= max);
        match field {
            "split" => match number(MAX_SPLIT as u64) {
                Some(n) => self.split = n as u16,
                None => return false,
            },
            "httpSplit" => match number(MAX_SPLIT as u64) {
                Some(n) => self.http_split = n as u16,
                None => return false,
            },
            "fakeTtl" => match number(64).filter(|n| *n >= 1) {
                Some(n) => self.fake_ttl = n as u8,
                None => return false,
            },
            "fakeRepeats" => match number(10).filter(|n| *n >= 1) {
                Some(n) => self.fake_repeats = n as u8,
                None => return false,
            },
            "splitAtSni" | "reverse" | "fakeBadChecksum" | "fakeBadSeq" | "blockQuic" => {
                let Some(on) = value.as_bool() else { return false };
                match field {
                    "splitAtSni" => self.split_at_sni = on,
                    "reverse" => self.reverse = on,
                    "fakeBadChecksum" => self.fake_bad_checksum = on,
                    "fakeBadSeq" => self.fake_bad_seq = on,
                    _ => self.block_quic = on,
                }
            }
            "fakeTtlMode" => match value.as_str() {
                Some(mode @ ("off" | "fixed" | "auto")) => self.fake_ttl_mode = mode.into(),
                _ => return false,
            },
            "noSni" => match value.as_str() {
                Some(policy @ ("skip" | "known" | "always")) => self.no_sni = policy.into(),
                _ => return false,
            },
            "fakeHost" => match value.as_str().and_then(host_of) {
                Some(host) => self.fake_host = host,
                None => return false,
            },
            _ => return false,
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsTarget {
    pub v4: String,
    pub port: u16,
    pub v6: Option<String>,
    pub port6: u16,
    /// true: bütün DNS sorguları yönlendirilir. false: yalnız hedef alan adlarının sorguları.
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngineRequest {
    pub strategy: Strategy,
    /// None: DNS'e dokunma.
    pub dns: Option<DnsTarget>,
    /// None: bütün sitelere uygula. Some: yalnız bu alan adlarına (ve alt alan adlarına).
    pub domains: Option<Vec<String>>,
    /// Bütün siteler kipinde dokunulmayacak alan adları (ve alt alan adları). Boşsa yazılmaz: eski sürümlerin
    /// uyguladığı ayarın parmak izi değişmez.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<String>,
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
            set.extend(settings.enabled_domains().cloned());
            if set.is_empty() {
                return Err(DpiError::new("nothing-selected"));
            }
            Some(set.into_iter().collect())
        };

        // "Bütün siteler" kipinde hedef listesi yoktur: DNS'in tamamı yönlendirilir.
        let all = domains.is_none() || settings.dns_all;
        let dns = if !settings.dns_enabled {
            None
        } else if settings.dns_provider == catalog::CUSTOM_DNS {
            if settings.dns_custom_addr.parse::<Ipv4Addr>().is_err() || settings.dns_custom_port == 0 {
                return Err(DpiError::new("bad-dns"));
            }
            Some(DnsTarget { v4: settings.dns_custom_addr.clone(), port: settings.dns_custom_port, v6: None, port6: 0, all })
        } else {
            let provider = catalog::dns(&settings.dns_provider).ok_or_else(|| DpiError::new("bad-dns"))?;
            Some(DnsTarget {
                v4: provider.v4.to_string(),
                port: provider.port,
                v6: Some(provider.v6.to_string()),
                port6: provider.port6,
                all,
            })
        };

        let strategy = match catalog::method(&settings.method) {
            Some(method) => method.strategy(),
            None => settings.custom.clone(),
        };

        // Hariç tutulan siteler yalnız "bütün siteler" kipinde anlamlıdır.
        let excluded = if domains.is_none() {
            settings.excluded_domains.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect()
        } else {
            Vec::new()
        };

        let request = Self { strategy, dns, domains, excluded };
        request.validate()?;
        Ok(request)
    }

    /// Yönetici yardımcısının ve motorun güvendiği tek denetim.
    pub fn validate(&self) -> DpiResult<()> {
        self.strategy.validate()?;
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
        if self.excluded.len() > MAX_DOMAINS {
            return Err(DpiError::new("too-many-domains"));
        }
        for domain in &self.excluded {
            if normalize_domain(domain).as_deref() != Some(domain.as_str()) {
                return Err(DpiError::with("bad-request", "excluded"));
            }
        }
        Ok(())
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

/// Kullanıcının yazdığını hedef alan adına çevirir: "https://www.Example.com/yol?x=1" -> "example.com".
/// Baştaki "www." ve "*." atılır: liste alt alan adlarını zaten kapsar. Alan adı değilse None.
pub fn normalize_domain(input: &str) -> Option<String> {
    let input = input.trim();
    let host = host_of(input.strip_prefix("*.").unwrap_or(input))?;
    host_of(host.strip_prefix("www.").unwrap_or(&host))
}

/// Yazılan metindeki sunucu adı, olduğu gibi: "https://www.Example.com:8443/yol" -> "www.example.com".
/// Alan adı değilse (IP adresi, tek kelime, geçersiz karakter) None.
pub fn host_of(input: &str) -> Option<String> {
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
#[cfg_attr(not(windows), allow(dead_code))]
pub fn command_line(exe: &Path, args: &[&str]) -> String {
    let mut line = quote_arg(&exe.to_string_lossy());
    for arg in args {
        line.push(' ');
        line.push_str(&quote_arg(arg));
    }
    line
}

#[cfg_attr(not(windows), allow(dead_code))]
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
    use serde_json::json;
    use std::path::PathBuf;

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
        assert_eq!(request.strategy, Strategy::default());
        let dns = request.dns.unwrap();
        assert_eq!((dns.v4.as_str(), dns.port, dns.all), ("9.9.9.9", 9953, false));
    }

    #[test]
    fn scope_modes() {
        let mut s = Settings { scope: "all".into(), ..Settings::default() };
        let all = EngineRequest::from_settings(&s).unwrap();
        assert_eq!(all.domains, None);
        assert!(all.dns.unwrap().all, "liste yokken DNS'in tamamı yönlendirilir");

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
        assert_eq!(EngineRequest::from_settings(&s).unwrap().dns, None);

        s.dns_enabled = true;
        s.dns_all = true;
        assert!(EngineRequest::from_settings(&s).unwrap().dns.unwrap().all);

        s.dns_provider = "custom".into();
        assert!(EngineRequest::from_settings(&s).unwrap_err().is("bad-dns"));
        s.dns_custom_addr = "8.8.8.8".into();
        s.dns_custom_port = 53;
        let custom = EngineRequest::from_settings(&s).unwrap().dns.unwrap();
        assert_eq!((custom.v4.as_str(), custom.port, custom.v6), ("8.8.8.8", 53, None));
    }

    #[test]
    fn methods_and_custom_strategy() {
        let mut s = Settings::default();
        for method in catalog::METHODS {
            s.method = method.id.into();
            let request = EngineRequest::from_settings(&s).unwrap();
            assert_eq!(request.strategy, method.strategy(), "{}", method.id);
        }

        s.method = "custom".into();
        s.custom = Strategy { split: 7, reverse: false, ..Strategy::default() };
        assert_eq!(EngineRequest::from_settings(&s).unwrap().strategy.split, 7);

        s.custom = Strategy { split: 0, http_split: 0, fake_ttl_mode: "off".into(), ..Strategy::default() };
        assert!(EngineRequest::from_settings(&s).unwrap_err().is("empty-strategy"));
    }

    #[test]
    fn strategy_fields_are_validated() {
        let mut s = Strategy::default();
        assert!(s.set("split", &json!(40)));
        assert!(!s.set("split", &json!(5000)));
        assert!(!s.set("split", &json!("2")));
        assert!(s.set("fakeTtlMode", &json!("auto")));
        assert!(!s.set("fakeTtlMode", &json!("sometimes")));
        assert!(s.set("fakeTtl", &json!(3)));
        assert!(!s.set("fakeTtl", &json!(0)));
        assert!(!s.set("fakeTtl", &json!(200)));
        assert!(s.set("fakeRepeats", &json!(3)));
        assert!(!s.set("fakeRepeats", &json!(50)));
        assert!(s.set("blockQuic", &json!(true)));
        assert!(!s.set("blockQuic", &json!(1)));
        assert!(s.set("noSni", &json!("always")));
        assert!(!s.set("noSni", &json!("maybe")));
        assert!(s.set("fakeHost", &json!("https://www.Example.org/")));
        assert!(!s.set("fakeHost", &json!("not a host")));
        assert!(!s.set("nope", &json!(true)));
        assert_eq!((s.split, s.fake_ttl, s.fake_host.as_str(), s.no_sni.as_str()), (40, 3, "www.example.org", "always"));
        assert!(!s.set("fakeHost", &json!("*.example.org")), "sahte paketteki ad gerçek bir sunucu adı olmalı");
        assert!(s.validate().is_ok());

        let bad = Strategy { fake_host: "a b".into(), ..Strategy::default() };
        assert!(bad.validate().is_err());
        let bad = Strategy { no_sni: "x".into(), ..Strategy::default() };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn helper_rejects_tampered_requests() {
        let good = EngineRequest::from_settings(&Settings::default()).unwrap();

        let mut bad = good.clone();
        bad.strategy.fake_ttl_mode = "fixed; rm -rf".into();
        assert!(bad.validate().is_err());

        let mut bad = good.clone();
        bad.strategy.split = 60000;
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
        // Dosyadan okunan istek aynı yapıya geri döner.
        let text = serde_json::to_string(&good).unwrap();
        assert_eq!(serde_json::from_str::<EngineRequest>(&text).unwrap(), good);
    }

    #[test]
    fn fingerprint_follows_settings_and_engine() {
        let mut s = Settings::default();
        let a = EngineRequest::from_settings(&s).unwrap();
        assert_eq!(a.fingerprint("e1"), a.clone().fingerprint("e1"));
        assert_ne!(a.fingerprint("e1"), a.fingerprint("e2"));
        s.method = "m4".into();
        assert_ne!(a.fingerprint("e1"), EngineRequest::from_settings(&s).unwrap().fingerprint("e1"));
        // Arayüz ayarları (dil, tepsi, gelişmiş ekranı) motoru etkilemez.
        let ui = Settings { language: "en".into(), close_to_tray: false, advanced: true, ..Settings::default() };
        assert_eq!(a.fingerprint("e1"), EngineRequest::from_settings(&ui).unwrap().fingerprint("e1"));
        // Seçili olmayan özel strateji de etkilemez.
        let unused = Settings { custom: Strategy { split: 9, ..Strategy::default() }, ..Settings::default() };
        assert_eq!(a.fingerprint("e1"), EngineRequest::from_settings(&unused).unwrap().fingerprint("e1"));
    }

    #[test]
    fn quotes_windows_command_lines() {
        let exe = PathBuf::from(r"C:\Program Files\RobzDPI\RobzDPI-engine.exe");
        assert_eq!(command_line(&exe, &["--engine"]), r#""C:\Program Files\RobzDPI\RobzDPI-engine.exe" --engine"#);
        assert_eq!(quote_arg(r"C:\dir with space\"), r#""C:\dir with space\\""#);
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote_arg(""), r#""""#);
    }
}
