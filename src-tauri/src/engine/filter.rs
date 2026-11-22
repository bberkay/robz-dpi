//! WinDivert süzgeci: hangi paketlerin sürücüden motora geleceği.
//!
//! Seçim çekirdekte, sürücünün içinde yapılır; motora yalnız işi olan paketler gelir. Bir bağlantıdan motora
//! gelen tek paket ClientHello'dur: sonraki bütün veri (indirme, yükleme, yayın) motora hiç uğramaz. GoodbyeDPI'ın
//! süzgeci 80 ve 443'e giden, yükü 1200 bayttan küçük her paketi kullanıcı kipine taşır.

use crate::plan::EngineRequest;

/// Şifresiz HTTP isteğinin ilk dört baytı (büyük uçlu): "GET ", "POST", "HEAD", "PUT ", "DELE", "OPTI", "CONN".
const HTTP_METHODS: [u32; 7] = [0x4745_5420, 0x504f_5354, 0x4845_4144, 0x5055_5420, 0x4445_4c45, 0x4f50_5449, 0x434f_4e4e];

pub fn build(request: &EngineRequest) -> String {
    let strategy = &request.strategy;
    let mut parts = Vec::new();

    // TLS ClientHello: kayıt türü 0x16, sürüm 0x03xx, el sıkışma türü 0x01.
    parts.push(
        "(outbound and tcp and tcp.DstPort == 443 and tcp.PayloadLength > 5 \
         and tcp.Payload[0] == 0x16 and tcp.Payload[1] == 0x03 and tcp.Payload[5] == 0x01)"
            .to_string(),
    );

    if strategy.http_split > 0 || strategy.fakes() {
        let methods: Vec<String> = HTTP_METHODS.iter().map(|m| format!("tcp.Payload32[0] == 0x{m:08X}")).collect();
        parts.push(format!(
            "(outbound and tcp and tcp.DstPort == 80 and tcp.PayloadLength > 16 and ({}))",
            methods.join(" or ")
        ));
    }

    // Otomatik sahte paket ömrü: sunucunun ilk yanıtındaki (SYN+ACK) TTL gerekir.
    if strategy.fake_ttl_mode == "auto" {
        parts.push("(inbound and tcp and tcp.Syn and tcp.Ack and (tcp.SrcPort == 443 or tcp.SrcPort == 80))".to_string());
    }

    // Hedef adreslerini DNS yanıtlarından öğrenmek gerekiyor mu?
    let learns = request.domains.is_some() && (strategy.no_sni == "known" || strategy.block_quic);
    let mut response_ports = Vec::new();
    if let Some(dns) = &request.dns {
        parts.push("(outbound and udp and udp.DstPort == 53)".to_string());
        response_ports.push(dns.port);
        if dns.v6.is_some() {
            response_ports.push(dns.port6);
        }
    } else if learns {
        // DNS'e dokunmuyoruz ama yanıtları (değiştirmeden) okuyoruz.
        response_ports.push(53);
    }
    response_ports.sort_unstable();
    response_ports.dedup();
    if !response_ports.is_empty() {
        let ports: Vec<String> = response_ports.iter().map(|port| format!("udp.SrcPort == {port}")).collect();
        parts.push(format!("(inbound and udp and ({}))", ports.join(" or ")));
    }

    // QUIC sürüm 1'in ilk paketi (GoodbyeDPI'ın -q süzgeci).
    if strategy.block_quic {
        parts.push(
            "(outbound and udp and udp.DstPort == 443 and udp.PayloadLength >= 1200 \
             and udp.Payload[0] >= 0xC0 and udp.Payload32[1b] == 0x01)"
                .to_string(),
        );
    }

    format!("!impostor and !loopback and ({})", parts.join(" or "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use crate::plan::DnsTarget;

    fn request(method: &str, dns: bool, selected: bool) -> EngineRequest {
        EngineRequest {
            strategy: catalog::method(method).unwrap().strategy(),
            dns: dns.then(|| DnsTarget { v4: "9.9.9.9".into(), port: 9953, v6: Some("2620:fe::fe".into()), port6: 9953, all: !selected }),
            domains: selected.then(|| vec!["discord.com".to_string()]),
        }
    }

    #[test]
    fn only_asks_for_what_the_strategy_needs() {
        let m1 = build(&request("m1", true, true));
        assert!(m1.starts_with("!impostor and !loopback and ("));
        assert!(m1.contains("tcp.Payload[5] == 0x01"));
        assert!(m1.contains("udp.DstPort == 53") && m1.contains("udp.SrcPort == 9953"));
        assert!(!m1.contains("tcp.Syn"), "sabit ömürde SYN+ACK gerekmez");
        assert!(!m1.contains("udp.DstPort == 443"), "QUIC engeli yöntem 1'de yok");
        assert_eq!(m1.matches("udp.SrcPort").count(), 1, "aynı bağlantı noktası iki kez yazılmaz");

        assert!(build(&request("m2", true, true)).contains("tcp.Syn and tcp.Ack"));
        assert!(build(&request("m4", true, true)).contains("udp.Payload32[1b] == 0x01"));

        // DNS kapalı: sorgulara dokunulmaz; adres öğrenmek gerekiyorsa yalnız yanıtlar okunur.
        let no_dns = build(&request("m1", false, true));
        assert!(!no_dns.contains("udp.DstPort == 53") && no_dns.contains("udp.SrcPort == 53"));
        // Bütün siteler kipinde öğrenmeye gerek yok.
        assert!(!build(&request("m1", false, false)).contains("udp"));
    }

    /// Süzgeçleri gerçek WinDivert.dll ile derler ve örnek paketlere uygular. Sürücü yüklenmez, ağa dokunulmaz:
    /// yalnız kitaplığın kullanıcı kipindeki derleme ve değerlendirme işlevleri çağrılır.
    #[cfg(windows)]
    #[test]
    fn windivert_accepts_the_filters_and_they_pick_the_right_packets() {
        use crate::engine::divert::{Address, Divert};
        use crate::engine::dnsmsg::testing as dns;
        use crate::engine::packet::testing::{tcp, udp};
        use crate::engine::tls::{self, Rng};
        use std::net::{IpAddr, Ipv4Addr};

        let dll = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("engine").join("WinDivert.dll");
        if !dll.is_file() {
            eprintln!("WinDivert.dll yok, sınama atlandı (npm run engine)");
            return;
        }
        let divert = Divert::load(&dll).expect("WinDivert.dll yüklenemedi");

        for method in ["m1", "m2", "m3", "m4"] {
            for (with_dns, selected) in [(true, true), (true, false), (false, true), (false, false)] {
                let filter = build(&request(method, with_dns, selected));
                assert_eq!(divert.compile(&filter), Ok(()), "{method} dns={with_dns} seçili={selected}: {filter}");
            }
        }
        assert!(divert.compile("outbound and tcp.Nope == 1").is_err(), "bozuk süzgeç reddedilmeli");

        let client = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
        let server = IpAddr::V4(Ipv4Addr::new(162, 159, 128, 233));
        let hello = tls::fake_hello("discord.com", &mut Rng::new(3));
        let out = Address::new(true);
        let inbound = Address::new(false);

        let m1 = build(&request("m1", true, true));
        let m4 = build(&request("m4", true, true));
        let m2 = build(&request("m2", true, true));

        // ClientHello'nun ilk paketi alınır; aynı bağlantının sonraki verisi alınmaz.
        assert!(divert.eval(&m1, &tcp(client, server, 50000, 443, 0x18, 1, 1, &hello), &out));
        assert!(divert.eval(&m1, &tcp(client, server, 50000, 443, 0x18, 1, 1, &hello[..300]), &out), "bölünmüş ClientHello'nun ilk parçası");
        assert!(!divert.eval(&m1, &tcp(client, server, 50000, 443, 0x18, 1, 1, &[0x17, 3, 3, 0, 40, 9, 9, 9, 9, 9]), &out), "uygulama verisi");
        assert!(!divert.eval(&m1, &tcp(client, server, 50000, 443, 0x10, 1, 1, b""), &out), "boş ACK");
        assert!(!divert.eval(&m1, &tcp(client, server, 50000, 8443, 0x18, 1, 1, &hello), &out), "başka bağlantı noktası");
        assert!(!divert.eval(&m1, &tcp(server, client, 443, 50000, 0x18, 1, 1, &hello), &inbound), "gelen veri");

        // Şifresiz HTTP isteği alınır, yanıtı ya da sıradan veri alınmaz.
        assert!(divert.eval(&m1, &tcp(client, server, 50000, 80, 0x18, 1, 1, b"GET / HTTP/1.1\r\nHost: a.com\r\n\r\n"), &out));
        assert!(!divert.eval(&m1, &tcp(client, server, 50000, 80, 0x18, 1, 1, b"just some bytes going by here"), &out));

        // DNS: giden sorgular ve seçilen sunucunun yanıtları.
        let router = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let quad9 = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));
        assert!(divert.eval(&m1, &udp(client, router, 61000, 53, &dns::query(1, "discord.com")), &out));
        assert!(divert.eval(&m1, &udp(quad9, client, 9953, 61000, &dns::response(1, "discord.com", &[server])), &inbound));
        assert!(!divert.eval(&m1, &udp(router, client, 53, 61000, &dns::response(1, "a.com", &[server])), &inbound));
        assert!(!divert.eval(&m1, &udp(client, server, 61000, 50001, b"discord voice or roblox game data"), &out), "oyun/ses trafiği");

        // SYN+ACK yalnız otomatik ömürde, QUIC yalnız engel açıkken alınır.
        let syn_ack = tcp(server, client, 443, 50000, 0x12, 1, 2, b"");
        assert!(divert.eval(&m2, &syn_ack, &inbound) && !divert.eval(&m1, &syn_ack, &inbound));
        let mut quic = vec![0xc3, 0, 0, 0, 1];
        quic.resize(1250, 0xaa);
        let quic = udp(client, server, 55000, 443, &quic);
        assert!(divert.eval(&m4, &quic, &out) && !divert.eval(&m1, &quic, &out));
    }
}
