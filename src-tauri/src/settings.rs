//! Ayarlar: veri klasöründeki settings.json. Bozuk dosyada ya da yanlış türde varsayılanlar kullanılır.

use crate::catalog;
use crate::plan::{normalize_domain, parse_domains, Strategy, MAX_DOMAINS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "auto" | "tr" | "en" | "ru"
    pub language: String,
    /// Pencere kapatılınca tepside kal.
    pub close_to_tray: bool,
    /// Windows açılışında (tepside) başlat; motor açık bırakıldıysa o da başlar.
    pub launch_at_login: bool,
    /// Kullanıcının istediği durum: motor açık mı?
    pub enabled: bool,
    /// İlk açılış adımları (dil, sorumluluk reddinin kabulü, kapsam) tamamlandı mı? Tamamlanmadan motor açılmaz.
    pub onboarded: bool,
    /// "selected": yalnız seçilen uygulamalar ve siteler | "all": bütün siteler (hariç tutulanlar dışında)
    pub scope: String,
    /// Açık olan hazır uygulamalar (catalog::SERVICES kimlikleri).
    pub services: Vec<String>,
    /// Kullanıcının eklediği alan adları (eklenme sırasıyla; arayüzde Discord ve Roblox'un altında listelenir).
    pub custom_domains: Vec<String>,
    /// Eklenen sitelerden anahtarı kapalı olanlar: listede kalırlar, motora gitmezler.
    pub disabled_domains: Vec<String>,
    /// "Bütün siteler" kipinde dokunulmayacak alan adları (ve alt alan adları).
    pub excluded_domains: Vec<String>,
    /// catalog::METHODS kimliği ya da "custom" (aşağıdaki `custom` stratejisi kullanılır).
    pub method: String,
    /// "Gelişmiş" ekranında düzenlenen strateji. Yalnız method "custom" iken motora gider.
    pub custom: Strategy,
    /// "Gelişmiş" bölümü arayüzde açık mı?
    pub advanced: bool,
    pub dns_enabled: bool,
    /// Bütün DNS sorgularını yönlendir (kapalıyken yalnız seçili sitelerin sorguları yönlendirilir).
    pub dns_all: bool,
    /// catalog::DNS kimliği ya da "custom".
    pub dns_provider: String,
    pub dns_custom_addr: String,
    pub dns_custom_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            close_to_tray: true,
            launch_at_login: false,
            enabled: false,
            onboarded: false,
            scope: "selected".into(),
            services: catalog::SERVICES.iter().map(|s| s.id.to_string()).collect(),
            custom_domains: Vec::new(),
            disabled_domains: Vec::new(),
            excluded_domains: Vec::new(),
            method: catalog::DEFAULT_METHOD.into(),
            custom: Strategy::default(),
            advanced: false,
            dns_enabled: true,
            dns_all: false,
            dns_provider: catalog::DEFAULT_DNS.into(),
            dns_custom_addr: String::new(),
            dns_custom_port: 53,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut settings: Settings =
            std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        settings.sanitize();
        settings
    }

    /// Elle düzenlenmiş ya da eski sürümden kalmış dosyadaki geçersiz değerleri varsayılana çeker.
    fn sanitize(&mut self) {
        let defaults = Settings::default();
        if !matches!(self.language.as_str(), "auto" | "tr" | "en" | "ru") {
            self.language = defaults.language;
        }
        if !matches!(self.scope.as_str(), "selected" | "all") {
            self.scope = defaults.scope;
        }
        self.services.retain(|id| catalog::service(id).is_some());
        self.services.dedup();
        self.custom_domains = clean_domains(std::mem::take(&mut self.custom_domains));
        self.excluded_domains = clean_domains(std::mem::take(&mut self.excluded_domains));
        let custom = &self.custom_domains;
        self.disabled_domains = clean_domains(std::mem::take(&mut self.disabled_domains));
        self.disabled_domains.retain(|d| custom.contains(d));
        if self.method != catalog::CUSTOM_METHOD && catalog::method(&self.method).is_none() {
            self.method = defaults.method;
        }
        // Alanlarından biri geçersizse özel strateji baştan kurulur. "Hiçbir şey yapmıyor" durumu korunur:
        // kullanıcı düzenlemenin ortasında olabilir, motoru açarken ayrıca denetlenir.
        if self.custom.validate().is_err_and(|error| !error.is("empty-strategy")) {
            self.custom = Strategy::default();
        }
        if self.dns_provider != catalog::CUSTOM_DNS && catalog::dns(&self.dns_provider).is_none() {
            self.dns_provider = defaults.dns_provider;
        }
        if self.dns_custom_addr.parse::<Ipv4Addr>().is_err() {
            self.dns_custom_addr.clear();
        }
        if self.dns_custom_port == 0 {
            self.dns_custom_port = 53;
        }
    }

    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        if let Ok(text) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }

    /// Arayüzden gelen değişiklik. Bilinmeyen anahtar, yanlış tür ya da geçersiz değer: false.
    pub fn set(&mut self, key: &str, value: &Value) -> bool {
        match (key, value) {
            ("language", Value::String(s)) if matches!(s.as_str(), "auto" | "tr" | "en" | "ru") => self.language = s.clone(),
            ("closeToTray", Value::Bool(b)) => self.close_to_tray = *b,
            ("onboarded", Value::Bool(b)) => self.onboarded = *b,
            ("launchAtLogin", Value::Bool(b)) => self.launch_at_login = *b,
            ("scope", Value::String(s)) if matches!(s.as_str(), "selected" | "all") => self.scope = s.clone(),
            ("method", Value::String(s)) if s == catalog::CUSTOM_METHOD || catalog::method(s).is_some() => {
                self.method = s.clone()
            }
            ("advanced", Value::Bool(b)) => self.advanced = *b,
            ("dnsEnabled", Value::Bool(b)) => self.dns_enabled = *b,
            ("dnsAll", Value::Bool(b)) => self.dns_all = *b,
            ("dnsProvider", Value::String(s)) if s == catalog::CUSTOM_DNS || catalog::dns(s).is_some() => {
                self.dns_provider = s.clone()
            }
            ("dnsCustomAddr", Value::String(s)) if s.is_empty() || s.parse::<Ipv4Addr>().is_ok() => {
                self.dns_custom_addr = s.clone()
            }
            ("dnsCustomPort", Value::Number(n)) => match n.as_u64() {
                Some(port @ 1..=65535) => self.dns_custom_port = port as u16,
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    /// "Gelişmiş" ekranından tek alanlık strateji değişikliği. Hazır bir yöntem seçiliyken yapılırsa o yöntemin
    /// stratejisi "özel"e kopyalanır ve değişiklik onun üstüne uygulanır: kullanıcı sıfırdan kurmak zorunda kalmaz.
    pub fn set_custom(&mut self, field: &str, value: &Value) -> bool {
        let mut strategy = match catalog::method(&self.method) {
            Some(method) => method.strategy(),
            None => self.custom.clone(),
        };
        if !strategy.set(field, value) {
            return false;
        }
        self.custom = strategy;
        self.method = catalog::CUSTOM_METHOD.into();
        true
    }

    /// Hazır bir uygulamayı (Discord, Roblox) açar ya da kapatır.
    pub fn set_service(&mut self, id: &str, on: bool) -> bool {
        if catalog::service(id).is_none() {
            return false;
        }
        self.services.retain(|s| s != id);
        if on {
            self.services.push(id.to_string());
            // Katalog sırasını koru: dosya ve parmak izi kararlı kalsın.
            self.services.sort_by_key(|s| catalog::SERVICES.iter().position(|c| c.id == s));
        }
        true
    }

    /// Kullanıcının yazdığı ya da yapıştırdığı adresleri ekler (anahtarları açık). (eklenenler, reddedilenler) döner.
    pub fn add_domains(&mut self, text: &str) -> (Vec<String>, Vec<String>) {
        add_to(&mut self.custom_domains, text)
    }

    pub fn remove_domain(&mut self, domain: &str) -> bool {
        self.disabled_domains.retain(|d| d != domain);
        remove_from(&mut self.custom_domains, domain)
    }

    /// Eklenen bir sitenin anahtarı: kapalıyken site listede kalır ama motora gitmez.
    pub fn set_domain(&mut self, domain: &str, on: bool) -> bool {
        if !self.custom_domains.iter().any(|d| d == domain) {
            return false;
        }
        self.disabled_domains.retain(|d| d != domain);
        if !on {
            self.disabled_domains.push(domain.to_string());
        }
        true
    }

    /// Motora gidecek eklenen siteler: anahtarı açık olanlar.
    pub fn enabled_domains(&self) -> impl Iterator<Item = &String> {
        self.custom_domains.iter().filter(|d| !self.disabled_domains.contains(d))
    }

    /// "Bütün siteler" kipinde hariç tutulacak adresleri ekler. (eklenenler, reddedilenler) döner.
    pub fn add_excluded(&mut self, text: &str) -> (Vec<String>, Vec<String>) {
        add_to(&mut self.excluded_domains, text)
    }

    pub fn remove_excluded(&mut self, domain: &str) -> bool {
        remove_from(&mut self.excluded_domains, domain)
    }

    /// Arayüze gönderilen kopya.
    pub fn public(&self) -> Value {
        json!(self)
    }
}

/// Geçerli, tekrarsız ve en çok MAX_DOMAINS alan adı; sıra korunur.
fn clean_domains(list: Vec<String>) -> Vec<String> {
    let mut domains = Vec::new();
    for domain in list {
        if let Some(domain) = normalize_domain(&domain) {
            if !domains.contains(&domain) && domains.len() < MAX_DOMAINS {
                domains.push(domain);
            }
        }
    }
    domains
}

fn add_to(list: &mut Vec<String>, text: &str) -> (Vec<String>, Vec<String>) {
    let (parsed, rejected) = parse_domains(text);
    let mut added = Vec::new();
    for domain in parsed {
        if list.len() >= MAX_DOMAINS {
            break;
        }
        if !list.contains(&domain) {
            list.push(domain.clone());
            added.push(domain);
        }
    }
    (added, rejected)
}

fn remove_from(list: &mut Vec<String>, domain: &str) -> bool {
    let before = list.len();
    list.retain(|d| d != domain);
    before != list.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_changes() {
        let mut s = Settings::default();
        assert!(s.set("language", &json!("en")));
        assert!(!s.set("language", &json!("de")));
        assert!(s.set("language", &json!("ru")) && s.set("language", &json!("en")));
        assert!(s.set("scope", &json!("all")));
        assert!(!s.set("scope", &json!("everything")));
        assert!(s.set("method", &json!("m3")));
        assert!(!s.set("method", &json!("-5 --evil")));
        assert!(s.set("dnsProvider", &json!("custom")));
        assert!(!s.set("dnsProvider", &json!("nope")));
        assert!(s.set("dnsCustomAddr", &json!("1.1.1.1")));
        assert!(!s.set("dnsCustomAddr", &json!("1.1.1.1 --flag")));
        assert!(s.set("dnsCustomPort", &json!(5353)));
        assert!(!s.set("dnsCustomPort", &json!(0)));
        assert!(!s.set("dnsCustomPort", &json!(70000)));
        assert!(!s.set("enabled", &json!(true)), "motor durumu yalnız aç/kapa ile değişir");
        assert!(!s.onboarded, "ilk açılış adımları varsayılan olarak tamamlanmamıştır");
        assert!(s.set("onboarded", &json!(true)) && s.onboarded);
        assert!(!s.set("onboarded", &json!("yes")));
        assert!(!s.set("customDomains", &json!(["x.com"])));
        assert!(!s.set("custom", &json!({ "split": 9 })), "strateji alan alan değişir");
        assert!(s.set("dnsAll", &json!(true)));
        assert!(s.set("advanced", &json!(true)));
        assert!(s.set("method", &json!("custom")));
        assert_eq!((s.language.as_str(), s.scope.as_str(), s.method.as_str()), ("en", "all", "custom"));
    }

    #[test]
    fn services_and_domains() {
        let mut s = Settings::default();
        assert_eq!(s.services, ["discord", "roblox"]);
        assert!(s.set_service("discord", false));
        assert_eq!(s.services, ["roblox"]);
        assert!(s.set_service("discord", true));
        assert_eq!(s.services, ["discord", "roblox"], "katalog sırası korunur");
        assert!(!s.set_service("netflix", true));

        let (added, rejected) = s.add_domains("https://www.example.com/x, example.com not-a-domain\nfoo.org");
        assert_eq!(added, ["example.com", "foo.org"]);
        assert_eq!(rejected, ["not-a-domain"]);
        assert!(s.remove_domain("example.com"));
        assert!(!s.remove_domain("example.com"));
        assert_eq!(s.custom_domains, ["foo.org"]);
    }

    #[test]
    fn editing_a_preset_turns_it_into_custom() {
        let mut s = Settings { method: "m4".into(), ..Settings::default() };
        let m4 = catalog::method("m4").unwrap().strategy();
        assert!(s.set_custom("split", &json!(5)));
        assert_eq!(s.method, "custom");
        assert_eq!(s.custom, Strategy { split: 5, ..m4 }, "yöntem 4'ün üstüne tek değişiklik");
        // Sonraki değişiklik özel stratejinin üstüne gelir.
        assert!(s.set_custom("blockQuic", &json!(false)));
        assert_eq!((s.custom.split, s.custom.block_quic, s.custom.fake_bad_seq), (5, false, true));
        // Geçersiz değişiklik hiçbir şeyi değiştirmez.
        let before = s.clone();
        assert!(!s.set_custom("split", &json!(99999)));
        assert_eq!(s, before);
    }

    #[test]
    fn survives_broken_files() {
        let dir = std::env::temp_dir().join(format!("robzdpi-settings-{}", std::process::id()));
        let file = dir.join("settings.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(Settings::load(&file), Settings::default());
        std::fs::write(
            &file,
            r#"{ "language": "de", "scope": "x", "services": ["discord", "nope"], "customDomains": ["HTTPS://A.com/", "bad", "a.com"],
                 "method": "zz", "dnsProvider": "zz", "dnsCustomAddr": "999.1.1.1", "dnsCustomPort": 0, "closeToTray": false,
                 "custom": { "split": 7, "fakeTtlMode": "bogus" } }"#,
        )
        .unwrap();
        let s = Settings::load(&file);
        assert_eq!(s.language, "auto");
        assert_eq!(s.scope, "selected");
        assert_eq!(s.services, ["discord"]);
        assert_eq!(s.custom_domains, ["a.com"]);
        assert_eq!(s.method, catalog::DEFAULT_METHOD);
        assert_eq!(s.custom, Strategy::default(), "geçersiz alanlı özel strateji baştan kurulur");
        assert_eq!(s.dns_provider, catalog::DEFAULT_DNS);
        assert_eq!(s.dns_custom_addr, "");
        assert_eq!(s.dns_custom_port, 53);
        assert!(!s.close_to_tray);
        s.save(&file);
        assert_eq!(Settings::load(&file), s);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn added_sites_can_be_switched_off_and_excluded_sites_are_kept_apart() {
        let mut s = Settings::default();
        assert_eq!(s.add_domains("example.com violabot.com").0.len(), 2);
        assert!(s.set_domain("example.com", false));
        assert!(!s.set_domain("unknown.org", false), "listede olmayan site");
        assert_eq!(s.enabled_domains().collect::<Vec<_>>(), ["violabot.com"]);
        assert!(s.set_domain("example.com", true));
        assert_eq!(s.enabled_domains().count(), 2);

        s.set_domain("example.com", false);
        assert!(s.remove_domain("example.com"));
        assert!(s.disabled_domains.is_empty(), "kaldırılan site kapalılar listesinde kalmaz");

        let (added, rejected) = s.add_excluded("https://www.Bank.example.com/giris not-a-domain");
        assert_eq!((added, rejected), (vec!["bank.example.com".to_string()], vec!["not-a-domain".to_string()]));
        assert!(s.custom_domains.iter().all(|d| d != "bank.example.com"), "hariç tutulanlar ayrı listede");
        assert!(s.remove_excluded("bank.example.com"));
        assert!(!s.remove_excluded("bank.example.com"));
    }

    #[test]
    fn disabled_sites_outside_the_list_are_dropped_on_load() {
        let dir = std::env::temp_dir().join(format!("robzdpi-settings-sites-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"customDomains":["a.com","B.com"],"disabledDomains":["b.com","ghost.com"],"excludedDomains":["x.org","x.org","bad value"]}"#,
        )
        .unwrap();
        let s = Settings::load(&path);
        assert_eq!(s.custom_domains, ["a.com", "b.com"]);
        assert_eq!(s.disabled_domains, ["b.com"]);
        assert_eq!(s.excluded_domains, ["x.org"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
