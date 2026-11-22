//! Yerleşik listeler: hazır uygulamalar (Discord, Roblox) ve alan adları, motor yöntemleri, DNS sağlayıcıları.
//! Kullanıcı bunları arayüzden seçer; dosya ya da bayrak düzenlemesi gerekmez.

use crate::plan::Strategy;
use serde_json::{json, Value};

pub struct Service {
    pub id: &'static str,
    pub name: &'static str,
    /// Alan adları alt alan adlarını da kapsar (motor "discord.com" için "cdn.discord.com"u da eşleştirir).
    pub domains: &'static [&'static str],
    /// "Bağlantıyı test et" bu adrese istek atar.
    pub probe: &'static str,
}

pub const SERVICES: &[Service] = &[
    Service {
        id: "discord",
        name: "Discord",
        domains: &[
            "discord.com",
            "discord.gg",
            "discordapp.com",
            "discordapp.net",
            "discord.media",
            "discordcdn.com",
            "discord.dev",
            "discord.new",
            "discord.gift",
            "discord.gifts",
            "discord.co",
            "discord.app",
            "discord.design",
            "discord.store",
            "discord.tools",
            "discordstatus.com",
            "discordmerch.com",
            "discordpartygames.com",
            "discordactivities.com",
            "discord-activities.com",
            "discordsays.com",
            "dis.gd",
        ],
        probe: "https://discord.com/api/v9/gateway",
    },
    Service {
        id: "roblox",
        name: "Roblox",
        domains: &[
            "roblox.com",
            "rbxcdn.com",
            "rbx.com",
            "rbxtrk.com",
            "rbxinfra.com",
            "rbxinfra.net",
            "robloxlabs.com",
            "robloxdev.com",
        ],
        probe: "https://www.roblox.com/",
    },
];

pub struct Method {
    pub id: &'static str,
    /// Aynı davranışı veren GoodbyeDPI bayrakları (GoodbyeDPI-Turkey'in komut dosyalarındaki dört birleşim).
    /// Yalnız bilgi içindir: GoodbyeDPI kullanmış olanlar hangi yöntemin neye karşılık geldiğini görsün.
    pub goodbyedpi: &'static str,
}

pub const METHODS: &[Method] = &[
    Method { id: "m1", goodbyedpi: "-5 --set-ttl 5" },
    Method { id: "m2", goodbyedpi: "-5" },
    Method { id: "m3", goodbyedpi: "--set-ttl 3" },
    Method { id: "m4", goodbyedpi: "-9" },
];

/// Kullanıcının "Gelişmiş" ekranında kendi kurduğu strateji.
pub const CUSTOM_METHOD: &str = "custom";

impl Method {
    /// Yöntemin motor stratejisi.
    pub fn strategy(&self) -> Strategy {
        let base = Strategy::default();
        match self.id {
            // Böl (2 bayt, ters sırayla) + 5 sıçramada ölen sahte paket.
            "m1" => base,
            // Aynısı; sahte paketin ömrü sunucunun uzaklığına göre hesaplanır.
            "m2" => Strategy { fake_ttl_mode: "auto".into(), ..base },
            // Bölme yok; yalnız 3 sıçramada ölen sahte paket.
            "m3" => Strategy { split: 0, http_split: 0, reverse: false, fake_ttl: 3, ..base },
            // Böl + sağlama toplamı bozuk ve sıra numarası eski iki sahte paket; QUIC kapalı.
            _ => Strategy {
                fake_ttl_mode: "off".into(),
                fake_bad_checksum: true,
                fake_bad_seq: true,
                block_quic: true,
                ..base
            },
        }
    }
}

pub struct Dns {
    pub id: &'static str,
    pub name: &'static str,
    pub v4: &'static str,
    pub port: u16,
    pub v6: &'static str,
    pub port6: u16,
}

/// Standart 53 dışında bir bağlantı noktasında da yanıt veren açık DNS sunucuları. Servis sağlayıcılar 53'e giden
/// sorguları kendi sunucularına çevirebildiği için motor sorguları bu bağlantı noktalarına yönlendirir.
pub const DNS: &[Dns] = &[
    Dns { id: "quad9", name: "Quad9", v4: "9.9.9.9", port: 9953, v6: "2620:fe::fe", port6: 9953 },
    Dns { id: "yandex", name: "Yandex", v4: "77.88.8.8", port: 1253, v6: "2a02:6b8::feed:0ff", port6: 1253 },
    Dns { id: "adguard", name: "AdGuard", v4: "94.140.14.140", port: 5353, v6: "2a10:50c0::1:ff", port6: 5353 },
    Dns { id: "opendns", name: "OpenDNS", v4: "208.67.222.222", port: 5353, v6: "2620:119:35::35", port6: 5353 },
];

pub const DEFAULT_METHOD: &str = "m1";
pub const DEFAULT_DNS: &str = "quad9";
pub const CUSTOM_DNS: &str = "custom";

pub fn service(id: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.id == id)
}

pub fn method(id: &str) -> Option<&'static Method> {
    METHODS.iter().find(|m| m.id == id)
}

pub fn dns(id: &str) -> Option<&'static Dns> {
    DNS.iter().find(|d| d.id == id)
}

/// Arayüze gönderilen kopya.
pub fn public() -> Value {
    json!({
        "services": SERVICES.iter().map(|s| json!({ "id": s.id, "name": s.name, "domains": s.domains })).collect::<Vec<_>>(),
        "methods": METHODS.iter().map(|m| json!({ "id": m.id, "flags": m.goodbyedpi, "strategy": m.strategy() })).collect::<Vec<_>>(),
        "dns": DNS.iter().map(|d| json!({ "id": d.id, "name": d.name, "address": d.v4, "port": d.port })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::normalize_domain;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn built_in_lists_are_well_formed() {
        for s in SERVICES {
            assert!(!s.domains.is_empty());
            for d in s.domains {
                assert_eq!(normalize_domain(d).as_deref(), Some(*d), "{d}");
            }
            assert!(s.probe.starts_with("https://"));
        }
        for d in DNS {
            assert!(d.v4.parse::<Ipv4Addr>().is_ok(), "{}", d.v4);
            assert!(d.v6.parse::<Ipv6Addr>().is_ok(), "{}", d.v6);
            assert!(d.port != 0 && d.port6 != 0);
        }
        assert!(method(DEFAULT_METHOD).is_some());
        assert!(dns(DEFAULT_DNS).is_some());
        assert!(method(CUSTOM_METHOD).is_none());
    }

    #[test]
    fn every_method_is_a_valid_and_distinct_strategy() {
        let strategies: Vec<_> = METHODS.iter().map(Method::strategy).collect();
        for (method, strategy) in METHODS.iter().zip(&strategies) {
            assert!(strategy.validate().is_ok(), "{}", method.id);
        }
        for (i, a) in strategies.iter().enumerate() {
            assert!(strategies.iter().skip(i + 1).all(|b| a != b), "iki yöntem aynı");
        }
        assert_eq!(strategies[0], Strategy::default());
        assert!(!strategies[2].splits() && strategies[2].fakes(), "yöntem 3 yalnız sahte paket gönderir");
        assert!(strategies[3].block_quic && strategies[3].fake_ttl_mode == "off");
    }
}
