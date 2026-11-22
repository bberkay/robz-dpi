//! Motorun kararları: bir paket geldiğinde ne yapılacak?
//!
//! Burası saf mantıktır: sürücü, soket ya da Windows çağrısı yoktur. Girdi bir paket ve yönü, çıktı bir karardır
//! (geçir, düşür ya da yerine şunları gönder). Bu sayede her davranış gerçek ağa dokunmadan test edilir.
//!
//! Teknikler GoodbyeDPI'dan (ValdikSS, Apache-2.0) alınmıştır: ClientHello'yu iki TCP parçasına bölmek, önüne
//! sunucuya ulaşmayacak sahte bir istek göndermek (kısa ömür, bozuk sağlama toplamı ya da eski sıra numarası),
//! DNS sorgularını başka bir sunucuya çevirmek. Farklar:
//!   * Yalnız ClientHello paketleri motora gelir (süzgeç sürücüde, bkz. filter.rs); GoodbyeDPI 1200 bayttan küçük
//!     bütün giden paketleri alır.
//!   * DNS yönlendirmesi alan adına göre seçicidir: hedef olmayan sorgulara dokunulmaz.
//!   * Alan adı pakette görünmüyorsa (büyük ClientHello) karar, hedeflerin DNS'ten öğrenilen adreslerine göre verilir.
//!   * Otomatik sahte paket ömründe GoodbyeDPI'daki taşma hatası yoktur (3 sıçrama uzaklıkta ömür 10 çıkıyordu).

use super::dnsmsg;
use super::packet::{self, Parsed, FLAG_ACK, FLAG_SYN, TCP, UDP};
use super::tls::{self, Hello, Rng};
use crate::plan::{EngineRequest, Strategy};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::{Duration, Instant};

const SWEEP_EVERY: Duration = Duration::from_secs(10);
/// Yanıtı gelmeyen DNS sorgusunun ve kullanılmayan bağlantı kaydının ömrü (GoodbyeDPI ile aynı).
const FLOW_LIFETIME: Duration = Duration::from_secs(30);
/// Öğrenilen bir hedef adresinin ömrü. Windows DNS önbelleği dolunca sorgu yeniden gider ve kayıt yenilenir.
const KNOWN_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_ENTRIES: usize = 8192;
const RECENT_HOSTS: usize = 8;
/// Ethernet çerçevesine sığan en büyük IP paketi; bundan büyüğü ağ kartının böleceği birleşik pakettir.
const MTU: usize = 1500;
/// Her yolda (PPPoE, tünel) tek çerçeveye sığan TCP yükü.
const SAFE_PAYLOAD: usize = 1200;

/// Hedef alan adları. "discord.com" kaydı "cdn.discord.com" ile de eşleşir.
pub struct HostSet(HashSet<String>);

impl HostSet {
    pub fn new(domains: &[String]) -> Self {
        Self(domains.iter().cloned().collect())
    }

    pub fn matches(&self, host: &str) -> bool {
        let mut rest = host;
        loop {
            if self.0.contains(rest) {
                return true;
            }
            match rest.split_once('.') {
                Some((_, parent)) => rest = parent,
                None => return false,
            }
        }
    }
}

struct Resolver {
    v4: (IpAddr, u16),
    v6: Option<(IpAddr, u16)>,
    all: bool,
}

struct DnsFlow {
    server: IpAddr,
    port: u16,
    at: Instant,
}

/// Arayüzde gösterilen sayaçlar.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Strateji uygulanan TLS bağlantıları
    pub tls: u64,
    /// Strateji uygulanan şifresiz HTTP istekleri
    pub http: u64,
    /// Yönlendirilen DNS sorguları
    pub dns: u64,
    /// Düşürülen QUIC bağlantı denemeleri
    pub quic: u64,
    /// Adresi öğrenilmiş hedef sayısı
    pub known: usize,
    /// Son işlenen alan adları (en yenisi başta)
    pub recent: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Paketi olduğu gibi geçir.
    Pass,
    /// Paketi at.
    Drop,
    /// Paketin yerine bunları, bu sırayla gönder.
    Send(Vec<Vec<u8>>),
}

pub struct Engine {
    strategy: Strategy,
    /// None: bütün siteler
    hosts: Option<HostSet>,
    dns: Option<Resolver>,
    /// (istemci adresi, istemci bağlantı noktası, sorgu kimliği) -> sorgunun asıl gittiği sunucu
    dns_flows: HashMap<(IpAddr, u16, u16), DnsFlow>,
    /// (sunucu, sunucu bağlantı noktası, istemci, istemci bağlantı noktası) -> sunucudan gelen ilk paketin TTL'i
    ttls: HashMap<(IpAddr, u16, IpAddr, u16), (u8, Instant)>,
    /// Hedef alan adlarına ait olduğu DNS yanıtlarından öğrenilen adresler -> son kullanma
    known: HashMap<IpAddr, Instant>,
    rng: Rng,
    last_sweep: Instant,
    pub stats: Stats,
}

impl Engine {
    /// `request` doğrulanmış olmalıdır (`EngineRequest::validate`).
    pub fn new(request: &EngineRequest, seed: u64, now: Instant) -> Self {
        let dns = request.dns.as_ref().and_then(|dns| {
            Some(Resolver {
                v4: (dns.v4.parse().ok()?, dns.port),
                v6: dns.v6.as_ref().and_then(|v6| Some((v6.parse().ok()?, dns.port6))),
                all: dns.all,
            })
        });
        Self {
            strategy: request.strategy.clone(),
            hosts: request.domains.as_deref().map(HostSet::new),
            dns,
            dns_flows: HashMap::new(),
            ttls: HashMap::new(),
            known: HashMap::new(),
            rng: Rng::new(seed),
            last_sweep: now,
            stats: Stats::default(),
        }
    }

    pub fn handle(&mut self, packet: &[u8], outbound: bool, now: Instant) -> Verdict {
        if now.duration_since(self.last_sweep) >= SWEEP_EVERY {
            self.sweep(now);
        }
        let Some(p) = packet::parse(packet) else { return Verdict::Pass };
        match (p.proto, outbound) {
            (TCP, true) if p.payload_len() > 0 => self.outbound_tcp(packet, &p),
            (TCP, false) => {
                self.inbound_tcp(&p, now);
                Verdict::Pass
            }
            (UDP, true) => self.outbound_udp(packet, &p, now),
            (UDP, false) => self.inbound_udp(packet, &p, now),
            _ => Verdict::Pass,
        }
    }

    fn targets(&self, host: &str) -> bool {
        self.hosts.as_ref().is_none_or(|hosts| hosts.matches(host))
    }

    fn note(&mut self, host: String) {
        self.stats.recent.retain(|h| *h != host);
        self.stats.recent.insert(0, host);
        self.stats.recent.truncate(RECENT_HOSTS);
    }

    // ---- TCP --------------------------------------------------------------------------------------------------------

    /// Sunucunun SYN+ACK paketi: otomatik kipte sahte paketin ömrünü hesaplamak için TTL'i saklanır.
    fn inbound_tcp(&mut self, p: &Parsed, now: Instant) {
        let syn_ack = p.flags & (FLAG_SYN | FLAG_ACK) == FLAG_SYN | FLAG_ACK;
        if syn_ack && self.strategy.fake_ttl_mode == "auto" && self.ttls.len() < MAX_ENTRIES {
            self.ttls.insert((p.src, p.src_port, p.dst, p.dst_port), (p.ttl, now));
        }
    }

    fn outbound_tcp(&mut self, packet: &[u8], p: &Parsed) -> Verdict {
        let payload = &packet[p.payload..p.end];

        if p.dst_port == 80 {
            let Some(host) = tls::http_host(payload) else { return Verdict::Pass };
            if !self.targets(&host) {
                return Verdict::Pass;
            }
            self.stats.http += 1;
            self.note(host);
            return self.treat(packet, p, None, true);
        }

        match tls::inspect(payload) {
            Hello::Other => Verdict::Pass,
            Hello::Sni { host, offset } => {
                if !self.targets(&host) {
                    return Verdict::Pass;
                }
                self.stats.tls += 1;
                self.note(host);
                self.treat(packet, p, Some(offset), false)
            }
            // Alan adı görünmüyor. "Bütün siteler" kipinde zaten her bağlantıya uygulanır; seçili sitelerde
            // karar stratejideki kurala ve hedefin adresini tanıyıp tanımadığımıza bağlıdır.
            Hello::NoSni | Hello::Truncated => {
                let apply = self.hosts.is_none()
                    || match self.strategy.no_sni.as_str() {
                        "always" => true,
                        "known" => self.known.contains_key(&p.dst),
                        _ => false,
                    };
                if !apply {
                    return Verdict::Pass;
                }
                self.stats.tls += 1;
                self.treat(packet, p, None, false)
            }
        }
    }

    /// Stratejiyi uygular: önce sahte paketler, sonra gerçek paket (bölünmüş ya da bütün).
    fn treat(&mut self, packet: &[u8], p: &Parsed, sni_offset: Option<usize>, http: bool) -> Verdict {
        let mut out = Vec::new();

        if self.strategy.fakes() {
            let fake = if http {
                tls::fake_http(&self.strategy.fake_host)
            } else {
                tls::fake_hello(&self.strategy.fake_host, &mut self.rng)
            };
            let ttl = match self.strategy.fake_ttl_mode.as_str() {
                "fixed" => Some(self.strategy.fake_ttl),
                // Bağlantının kaydı yoksa (SYN+ACK görülmediyse) ömür tahmin edilmez: sahte paket sunucuya
                // ulaşıp bağlantıyı bozabilir.
                "auto" => self.ttls.remove(&(p.dst, p.dst_port, p.src, p.src_port)).and_then(|(ttl, _)| auto_ttl(ttl)),
                _ => None,
            };
            for _ in 0..self.strategy.fake_repeats {
                if let Some(ttl) = ttl {
                    let mut packet = packet::with_payload(packet, p, &fake);
                    packet::set_ttl(&mut packet, p, ttl);
                    packet::fix_checksums(&mut packet);
                    out.push(packet);
                }
                if self.strategy.fake_bad_checksum {
                    let mut packet = packet::with_payload(packet, p, &fake);
                    packet::fix_checksums(&mut packet);
                    packet::corrupt_tcp_checksum(&mut packet, p);
                    out.push(packet);
                }
                if self.strategy.fake_bad_seq {
                    // GoodbyeDPI'daki değerler: Linux'un bağlantı izleyicisinin kabul etmediği en küçük ACK kayması.
                    let mut packet = packet::with_payload(packet, p, &fake);
                    packet::set_seq_ack(&mut packet, p, p.seq.wrapping_sub(10_000), p.ack.wrapping_sub(66_000));
                    packet::fix_checksums(&mut packet);
                    out.push(packet);
                }
            }
        }

        let at = if http {
            usize::from(self.strategy.http_split)
        } else {
            match sni_offset {
                Some(offset) if self.strategy.split_at_sni => offset,
                _ => usize::from(self.strategy.split),
            }
        };
        let real = match packet::split_tcp(packet, p, at) {
            Some((head, tail)) if self.strategy.reverse => vec![tail, head],
            Some((head, tail)) => vec![head, tail],
            None => vec![packet[..p.end].to_vec()],
        };
        // Sürücü, ağ kartına bölme devredildiğinde tek pakette bütün ClientHello'yu verebilir. Geri bırakılan
        // parçalar yol üstündeki en küçük çerçeveye sığsın diye büyük paket güvenli boya bölünür.
        let oversized = p.end > MTU;
        for packet in real {
            if oversized {
                out.extend(packet::segment_tcp(packet, SAFE_PAYLOAD));
            } else {
                out.push(packet);
            }
        }
        Verdict::Send(out)
    }

    // ---- UDP: DNS ve QUIC -------------------------------------------------------------------------------------------

    fn outbound_udp(&mut self, packet: &[u8], p: &Parsed, now: Instant) -> Verdict {
        let payload = &packet[p.payload..p.end];

        if p.dst_port == 53 {
            let Some(resolver) = &self.dns else { return Verdict::Pass };
            let (Some(name), Some(id)) = (dnsmsg::query_name(payload), dnsmsg::id(payload)) else { return Verdict::Pass };
            if !(resolver.all || self.targets(&name)) {
                return Verdict::Pass;
            }
            let (address, port) = match (p.v6, resolver.v6) {
                (false, _) => resolver.v4,
                (true, Some(v6)) => v6,
                (true, None) => return Verdict::Pass,
            };
            if (p.dst, p.dst_port) == (address, port) || self.dns_flows.len() >= MAX_ENTRIES {
                return Verdict::Pass;
            }
            let Some(moved) = packet::readdress(packet, p, true, address, port) else { return Verdict::Pass };
            self.dns_flows.insert((p.src, p.src_port, id), DnsFlow { server: p.dst, port: p.dst_port, at: now });
            self.stats.dns += 1;
            return Verdict::Send(vec![moved]);
        }

        if p.dst_port == 443 && self.strategy.block_quic && is_quic_initial(payload) {
            // QUIC'te alan adı şifrelidir: seçili sitelerde yalnız adresi bilinen hedeflere giden deneme düşürülür.
            if self.hosts.is_none() || self.known.contains_key(&p.dst) {
                self.stats.quic += 1;
                return Verdict::Drop;
            }
        }
        Verdict::Pass
    }

    fn inbound_udp(&mut self, packet: &[u8], p: &Parsed, now: Instant) -> Verdict {
        let from_resolver = self.dns.as_ref().is_some_and(|r| {
            (p.src, p.src_port) == r.v4 || r.v6.is_some_and(|v6| (p.src, p.src_port) == v6)
        });
        if !from_resolver && p.src_port != 53 {
            return Verdict::Pass;
        }
        let payload = &packet[p.payload..p.end];

        // Hedef bir alan adının yanıtıysa adreslerini öğren.
        if let (Some(hosts), Some((name, addresses))) = (&self.hosts, dnsmsg::answers(payload)) {
            if hosts.matches(&name) {
                for address in addresses {
                    if self.known.len() < MAX_ENTRIES || self.known.contains_key(&address) {
                        self.known.insert(address, now + KNOWN_LIFETIME);
                    }
                }
                self.stats.known = self.known.len();
            }
        }

        // Yönlendirdiğimiz sorgunun yanıtı: uygulamanın sorduğu sunucudan gelmiş gibi geri çevir.
        if from_resolver {
            let flow = dnsmsg::id(payload).and_then(|id| self.dns_flows.remove(&(p.dst, p.dst_port, id)));
            if let Some(flow) = flow {
                if let Some(restored) = packet::readdress(packet, p, false, flow.server, flow.port) {
                    return Verdict::Send(vec![restored]);
                }
            }
        }
        Verdict::Pass
    }

    fn sweep(&mut self, now: Instant) {
        self.last_sweep = now;
        self.dns_flows.retain(|_, flow| now.duration_since(flow.at) < FLOW_LIFETIME);
        self.ttls.retain(|_, (_, at)| now.duration_since(*at) < FLOW_LIFETIME);
        self.known.retain(|_, expires| *expires > now);
        self.stats.known = self.known.len();
    }
}

/// QUIC sürüm 1'in ilk (Initial) paketi: uzun başlık, sürüm 1, en az 1200 bayt.
fn is_quic_initial(payload: &[u8]) -> bool {
    payload.len() >= 1200 && payload[0] >= 0xc0 && payload[1..5] == [0, 0, 0, 1]
}

/// Sunucudan gelen paketin TTL'inden sahte paketin ömrü: sunucuya ulaşmadan, aradaki cihazı geçtikten sonra ölsün.
/// GoodbyeDPI'ın `--auto-ttl 1-4-10` (en az 3 sıçrama) hesabı.
pub fn auto_ttl(ttl: u8) -> Option<u8> {
    const NEAR: u8 = 1;
    const FAR: u8 = 4;
    const MIN_HOPS: u8 = 3;
    const MAX: u8 = 10;
    // Sunucular çoğunlukla 128 ya da 64 ile başlar.
    let hops = match ttl {
        99..=127 => 128 - ttl,
        35..=63 => 64 - ttl,
        _ => return None,
    };
    if hops <= NEAR || hops < MIN_HOPS {
        return None;
    }
    let mut fake = hops.saturating_sub(FAR);
    if fake < FAR && hops <= 9 {
        fake = hops - NEAR - (f32::from(FAR - NEAR) * (f32::from(hops) / 10.0)) as u8;
    }
    Some(fake.min(MAX)).filter(|ttl| *ttl > 0)
}

#[cfg(test)]
mod tests {
    use super::super::dnsmsg::testing as dns;
    use super::super::packet::testing::{checksums_ok, tcp, udp};
    use super::*;
    use crate::catalog;
    use crate::plan::DnsTarget;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    const DISCORD: IpAddr = IpAddr::V4(Ipv4Addr::new(162, 159, 128, 233));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    const ROUTER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    const QUAD9: IpAddr = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));

    fn request(method: &str, domains: Option<&[&str]>) -> EngineRequest {
        EngineRequest {
            strategy: catalog::method(method).unwrap().strategy(),
            dns: Some(DnsTarget { v4: "9.9.9.9".into(), port: 9953, v6: Some("2620:fe::fe".into()), port6: 9953, all: domains.is_none() }),
            domains: domains.map(|list| list.iter().map(|d| d.to_string()).collect()),
        }
    }

    fn start(method: &str, domains: Option<&[&str]>) -> (Engine, Instant) {
        let now = Instant::now();
        (Engine::new(&request(method, domains), 1234, now), now)
    }

    fn hello(host: &str) -> Vec<u8> {
        tls::fake_hello(host, &mut Rng::new(5))
    }

    fn client_hello_to(dst: IpAddr, payload: &[u8]) -> Vec<u8> {
        tcp(CLIENT, dst, 50123, 443, 0x18, 1_000_000, 2_000_000, payload)
    }

    fn sent(verdict: Verdict) -> Vec<Vec<u8>> {
        match verdict {
            Verdict::Send(packets) => packets,
            other => panic!("gönderim bekleniyordu: {other:?}"),
        }
    }

    fn payload_of(packet: &[u8]) -> &[u8] {
        let p = packet::parse(packet).unwrap();
        &packet[p.payload..p.end]
    }

    #[test]
    fn host_set_matches_subdomains_only() {
        let hosts = HostSet::new(&["discord.com".to_string(), "rbxcdn.com".to_string()]);
        assert!(hosts.matches("discord.com"));
        assert!(hosts.matches("cdn.eu.discord.com"));
        assert!(hosts.matches("tr.rbxcdn.com"));
        assert!(!hosts.matches("notdiscord.com"));
        assert!(!hosts.matches("discord.com.evil.org"));
        assert!(!hosts.matches("com"));
    }

    #[test]
    fn method_1_fakes_then_splits_in_reverse() {
        let (mut engine, now) = start("m1", Some(&["discord.com"]));
        let real = hello("gateway.discord.com");
        let packet = client_hello_to(DISCORD, &real);
        let out = sent(engine.handle(&packet, true, now));
        assert_eq!(out.len(), 3, "sahte + iki parça");

        // 1) Sahte: zararsız alan adı, 5 sıçramada ölür, gerisi gerçek paketle aynı.
        let fake = packet::parse(&out[0]).unwrap();
        assert_eq!((fake.ttl, fake.seq, fake.ack, fake.dst, fake.dst_port), (5, 1_000_000, 2_000_000, DISCORD, 443));
        assert!(matches!(tls::inspect(payload_of(&out[0])), Hello::Sni { host, .. } if host == "www.w3.org"));
        assert!(checksums_ok(&out[0]));

        // 2) Kuyruk önce, 3) ilk iki bayt sonra; birleşince gerçek ClientHello.
        let (tail, head) = (packet::parse(&out[1]).unwrap(), packet::parse(&out[2]).unwrap());
        assert_eq!((head.seq, tail.seq, head.ttl, tail.ttl), (1_000_000, 1_000_002, 128, 128));
        assert_eq!(payload_of(&out[2]), &real[..2]);
        assert_eq!([payload_of(&out[2]), payload_of(&out[1])].concat(), real);
        assert!(checksums_ok(&out[1]) && checksums_ok(&out[2]));

        assert_eq!((engine.stats.tls, engine.stats.recent.as_slice()), (1, &["gateway.discord.com".to_string()][..]));
    }

    #[test]
    fn other_sites_are_left_alone() {
        let (mut engine, now) = start("m1", Some(&["discord.com"]));
        let packet = client_hello_to(OTHER, &hello("www.sahibinden.com"));
        assert_eq!(engine.handle(&packet, true, now), Verdict::Pass);
        // TLS olmayan veri, veri taşımayan paket ve gelen veri de geçer.
        assert_eq!(engine.handle(&client_hello_to(DISCORD, b"\x17\x03\x03\x00\x05hello"), true, now), Verdict::Pass);
        assert_eq!(engine.handle(&client_hello_to(DISCORD, b""), true, now), Verdict::Pass);
        assert_eq!(engine.handle(&client_hello_to(DISCORD, &hello("discord.com")), false, now), Verdict::Pass);
        assert_eq!(engine.stats, Stats::default());
    }

    #[test]
    fn method_3_only_sends_a_fake() {
        let (mut engine, now) = start("m3", Some(&["roblox.com"]));
        let packet = client_hello_to(OTHER, &hello("www.roblox.com"));
        let out = sent(engine.handle(&packet, true, now));
        assert_eq!(out.len(), 2);
        assert_eq!(packet::parse(&out[0]).unwrap().ttl, 3);
        assert_eq!(out[1], packet, "gerçek paket bölünmeden, değişmeden gider");
    }

    #[test]
    fn method_4_uses_bad_checksum_and_old_sequence() {
        let (mut engine, now) = start("m4", Some(&["discord.com"]));
        let out = sent(engine.handle(&client_hello_to(DISCORD, &hello("discord.com")), true, now));
        assert_eq!(out.len(), 4, "iki sahte + iki parça");

        let bad_sum = packet::parse(&out[0]).unwrap();
        assert!(!checksums_ok(&out[0]), "sunucu bu paketi atar");
        assert_eq!((bad_sum.ttl, bad_sum.seq), (128, 1_000_000));

        let old_seq = packet::parse(&out[1]).unwrap();
        assert!(checksums_ok(&out[1]));
        assert_eq!((old_seq.seq, old_seq.ack), (1_000_000 - 10_000, 2_000_000 - 66_000));
        assert_eq!(old_seq.ttl, 128, "ömür kısaltılmaz");
    }

    #[test]
    fn method_2_needs_the_servers_ttl() {
        let (mut engine, now) = start("m2", Some(&["discord.com"]));
        let packet = client_hello_to(DISCORD, &hello("discord.com"));
        // Sunucunun uzaklığı bilinmiyorken sahte paket gönderilmez; yalnız bölünür.
        assert_eq!(sent(engine.handle(&packet, true, now)).len(), 2);

        // Sunucudan SYN+ACK 52 TTL ile geldi: 12 sıçrama uzakta -> sahte paket 8 sıçrama yaşar.
        let mut syn_ack = tcp(DISCORD, CLIENT, 443, 50123, FLAG_SYN | FLAG_ACK, 9, 1_000_000, b"");
        syn_ack[8] = 52;
        assert_eq!(engine.handle(&syn_ack, false, now), Verdict::Pass);
        let out = sent(engine.handle(&packet, true, now));
        assert_eq!(out.len(), 3);
        assert_eq!(packet::parse(&out[0]).unwrap().ttl, 8);
        // Kayıt bir kez kullanılır.
        assert_eq!(sent(engine.handle(&packet, true, now)).len(), 2);
    }

    #[test]
    fn auto_ttl_table() {
        // (sunucudan gelen TTL, beklenen sahte paket ömrü)
        let cases = [
            (64, None),      // bilinmeyen başlangıç değeri
            (63, None),      // 1 sıçrama: çok yakın
            (62, None),      // 2 sıçrama: çok yakın
            (61, Some(2)),   // 3 sıçrama (GoodbyeDPI burada taşıp 10 verir)
            (60, Some(2)),   // 4
            (59, Some(3)),   // 5
            (58, Some(4)),   // 6
            (56, Some(4)),   // 8
            (54, Some(6)),   // 10
            (50, Some(10)),  // 14
            (40, Some(10)),  // 24: üst sınır
            (120, Some(4)),  // 128 ile başlayan sunucu, 8 sıçrama
            (200, None),
            (20, None),
        ];
        for (ttl, expected) in cases {
            assert_eq!(auto_ttl(ttl), expected, "TTL {ttl}");
        }
        // Sahte paket hiçbir uzaklıkta sunucuya ulaşacak kadar yaşamaz.
        for ttl in (35..=63).chain(99..=127) {
            let hops = if ttl > 64 { 128 - ttl } else { 64 - ttl };
            assert!(auto_ttl(ttl).is_none_or(|fake| fake < hops), "TTL {ttl}");
        }
    }

    #[test]
    fn split_at_sni_cuts_right_before_the_name() {
        let mut request = request("m1", Some(&["discord.com"]));
        request.strategy = Strategy { split_at_sni: true, reverse: false, fake_ttl_mode: "off".into(), ..Strategy::default() };
        let mut engine = Engine::new(&request, 1, Instant::now());
        let real = hello("discord.com");
        let out = sent(engine.handle(&client_hello_to(DISCORD, &real), true, Instant::now()));
        assert_eq!(out.len(), 2);
        assert!(payload_of(&out[1]).starts_with(b"discord.com"), "ikinci parça alan adıyla başlar");
        assert_eq!([payload_of(&out[0]), payload_of(&out[1])].concat(), real);
    }

    #[test]
    fn oversized_packets_are_cut_to_safe_pieces() {
        // Ağ kartına bölme devredilmişse sürücü 3000 baytlık ClientHello'yu tek paket olarak verir.
        let mut real = hello("discord.com");
        real.resize(3000, 0x5a);
        let (mut engine, now) = start("m1", Some(&["discord.com"]));
        let out = sent(engine.handle(&client_hello_to(DISCORD, &real), true, now));
        let pieces = &out[1..]; // ilki sahte paket
        assert!(pieces.len() >= 4, "{} parça", pieces.len());
        assert!(pieces.iter().all(|packet| payload_of(packet).len() <= SAFE_PAYLOAD && checksums_ok(packet)));

        // Sıra numarasına göre dizilince gerçek veri eksiksiz çıkar; en son gönderilen ilk iki bayttır.
        let mut ordered: Vec<_> = pieces.iter().map(|packet| (packet::parse(packet).unwrap().seq, payload_of(packet))).collect();
        assert_eq!(ordered.last().unwrap(), &(1_000_000, &real[..2]));
        ordered.sort_by_key(|(seq, _)| *seq);
        assert_eq!(ordered.iter().flat_map(|(_, bytes)| bytes.iter().copied()).collect::<Vec<u8>>(), real);
    }

    #[test]
    fn hidden_sni_follows_the_policy() {
        let real = hello("discord.com");
        let Hello::Sni { offset, .. } = tls::inspect(&real) else { panic!() };
        let first_segment = &real[..offset]; // alan adı ikinci pakette kalmış
        assert_eq!(tls::inspect(first_segment), Hello::Truncated);
        let packet = client_hello_to(DISCORD, first_segment);

        // "known": adres henüz öğrenilmedi -> dokunma.
        let (mut engine, now) = start("m1", Some(&["discord.com"]));
        assert_eq!(engine.handle(&packet, true, now), Verdict::Pass);

        // discord.com'un DNS yanıtı görülünce adres öğrenilir ve aynı paket artık işlenir.
        let answer = udp(ROUTER, CLIENT, 53, 61000, &dns::response(1, "discord.com", &[DISCORD]));
        assert_eq!(engine.handle(&answer, false, now), Verdict::Pass);
        assert_eq!(engine.stats.known, 1);
        assert_eq!(sent(engine.handle(&packet, true, now)).len(), 3);
        // Başka bir adrese giden gizli ClientHello'ya yine dokunulmaz.
        assert_eq!(engine.handle(&client_hello_to(OTHER, first_segment), true, now), Verdict::Pass);

        // Hedef olmayan bir alan adının yanıtı adres öğretmez.
        let unrelated = udp(ROUTER, CLIENT, 53, 61001, &dns::response(2, "example.org", &[OTHER]));
        engine.handle(&unrelated, false, now);
        assert_eq!(engine.stats.known, 1);

        // Öğrenilen adres süresi dolunca unutulur.
        let later = now + KNOWN_LIFETIME + SWEEP_EVERY;
        assert_eq!(engine.handle(&packet, true, later), Verdict::Pass);
        assert_eq!(engine.stats.known, 0);

        for (policy, applied) in [("skip", false), ("always", true)] {
            let mut request = request("m1", Some(&["discord.com"]));
            request.strategy.no_sni = policy.into();
            let mut engine = Engine::new(&request, 1, now);
            assert_eq!(matches!(engine.handle(&packet, true, now), Verdict::Send(_)), applied, "{policy}");
        }
    }

    #[test]
    fn all_sites_mode_treats_everything() {
        let (mut engine, now) = start("m1", None);
        assert_eq!(sent(engine.handle(&client_hello_to(OTHER, &hello("www.sahibinden.com")), true, now)).len(), 3);
        let real = hello("example.org");
        assert_eq!(sent(engine.handle(&client_hello_to(OTHER, &real[..80]), true, now)).len(), 3, "alan adı görünmese de");
    }

    #[test]
    fn plain_http_is_treated_by_host_header() {
        let (mut engine, now) = start("m1", Some(&["example.com"]));
        let request = b"GET /page HTTP/1.1\r\nHost: www.example.com\r\nAccept: */*\r\n\r\n";
        let packet = tcp(CLIENT, OTHER, 50200, 80, 0x18, 500, 600, request);
        let out = sent(engine.handle(&packet, true, now));
        assert_eq!(out.len(), 3);
        assert!(payload_of(&out[0]).starts_with(b"GET / HTTP/1.1\r\nHost: www.w3.org\r\n"));
        assert_eq!([payload_of(&out[2]), payload_of(&out[1])].concat(), request);
        assert_eq!(payload_of(&out[2]), b"GE");
        assert_eq!(engine.stats.http, 1);

        let other = tcp(CLIENT, OTHER, 50201, 80, 0x18, 500, 600, b"GET / HTTP/1.1\r\nHost: other.org\r\n\r\n");
        assert_eq!(engine.handle(&other, true, now), Verdict::Pass);
    }

    #[test]
    fn dns_is_redirected_only_for_targets() {
        let (mut engine, now) = start("m1", Some(&["discord.com"]));

        // Hedef olmayan sorgu olduğu gibi geçer.
        let other = udp(CLIENT, ROUTER, 61000, 53, &dns::query(10, "www.google.com"));
        assert_eq!(engine.handle(&other, true, now), Verdict::Pass);

        // Hedef sorgu Quad9'un 9953 bağlantı noktasına çevrilir.
        let query = udp(CLIENT, ROUTER, 61001, 53, &dns::query(11, "cdn.discord.com"));
        let out = sent(engine.handle(&query, true, now));
        let moved = packet::parse(&out[0]).unwrap();
        assert_eq!((moved.dst, moved.dst_port, moved.src, moved.src_port), (QUAD9, 9953, CLIENT, 61001));
        assert_eq!(payload_of(&out[0]), payload_of(&query), "sorgunun içeriği değişmez");
        assert!(checksums_ok(&out[0]));

        // Yanıt, uygulamanın sorduğu sunucudan (yönlendirici, 53) gelmiş gibi geri çevrilir ve adres öğrenilir.
        let answer = udp(QUAD9, CLIENT, 9953, 61001, &dns::response(11, "cdn.discord.com", &[DISCORD]));
        let out = sent(engine.handle(&answer, false, now));
        let restored = packet::parse(&out[0]).unwrap();
        assert_eq!((restored.src, restored.src_port, restored.dst, restored.dst_port), (ROUTER, 53, CLIENT, 61001));
        assert!(checksums_ok(&out[0]));
        assert_eq!((engine.stats.dns, engine.stats.known), (1, 1));

        // Aynı yanıt ikinci kez gelirse (ya da hiç sormadığımız bir yanıtsa) dokunulmadan geçer.
        assert_eq!(engine.handle(&answer, false, now), Verdict::Pass);
        // Yanıtı 30 saniye gelmeyen sorgunun kaydı silinir.
        sent(engine.handle(&query, true, now));
        assert_eq!(engine.handle(&answer, false, now + FLOW_LIFETIME + SWEEP_EVERY), Verdict::Pass);
    }

    #[test]
    fn dns_for_everything_and_ipv6() {
        let (mut engine, now) = start("m1", None);
        let query = udp(CLIENT, ROUTER, 61000, 53, &dns::query(1, "www.google.com"));
        assert!(matches!(engine.handle(&query, true, now), Verdict::Send(_)), "bütün siteler kipinde her sorgu");

        let client6 = IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 2));
        let router6 = IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1));
        let out = sent(engine.handle(&udp(client6, router6, 61002, 53, &dns::query(2, "a.com")), true, now));
        let moved = packet::parse(&out[0]).unwrap();
        assert_eq!((moved.dst, moved.dst_port), ("2620:fe::fe".parse().unwrap(), 9953));

        // DNS kapalıyken sorgulara hiç dokunulmaz.
        let mut off = request("m1", None);
        off.dns = None;
        let mut engine = Engine::new(&off, 1, now);
        assert_eq!(engine.handle(&query, true, now), Verdict::Pass);
    }

    #[test]
    fn quic_is_dropped_only_towards_known_targets() {
        let mut initial = vec![0xc3, 0, 0, 0, 1];
        initial.resize(1250, 0xaa);
        let to_discord = udp(CLIENT, DISCORD, 55000, 443, &initial);

        let (mut engine, now) = start("m4", Some(&["discord.com"]));
        assert_eq!(engine.handle(&to_discord, true, now), Verdict::Pass, "adres henüz bilinmiyor");
        engine.handle(&udp(ROUTER, CLIENT, 53, 61000, &dns::response(1, "discord.com", &[DISCORD])), false, now);
        assert_eq!(engine.handle(&to_discord, true, now), Verdict::Drop);
        assert_eq!(engine.handle(&udp(CLIENT, OTHER, 55000, 443, &initial), true, now), Verdict::Pass);
        // Kısa paket ya da başka sürüm QUIC ilk paketi sayılmaz.
        assert_eq!(engine.handle(&udp(CLIENT, DISCORD, 55000, 443, &initial[..600]), true, now), Verdict::Pass);

        // QUIC engeli kapalı yöntemde hiçbir şey düşmez.
        let (mut engine, now) = start("m1", None);
        assert_eq!(engine.handle(&to_discord, true, now), Verdict::Pass);
        // "Bütün siteler" + yöntem 4: her QUIC denemesi düşer.
        let (mut engine, now) = start("m4", None);
        assert_eq!(engine.handle(&udp(CLIENT, OTHER, 55000, 443, &initial), true, now), Verdict::Drop);
    }

    #[test]
    fn works_over_ipv6() {
        let client = IpAddr::V6(Ipv6Addr::new(0x2a02, 0, 0, 0, 0, 0, 0, 2));
        let server = IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 1));
        let (mut engine, now) = start("m1", Some(&["discord.com"]));
        let real = hello("discord.com");
        let out = sent(engine.handle(&tcp(client, server, 50000, 443, 0x18, 7, 8, &real), true, now));
        assert_eq!(out.len(), 3);
        assert_eq!(packet::parse(&out[0]).unwrap().ttl, 5, "IPv6'da Hop Limit");
        assert_eq!([payload_of(&out[2]), payload_of(&out[1])].concat(), real);
        assert!(out.iter().all(|packet| checksums_ok(packet)));
    }

    #[test]
    fn never_panics_on_noise() {
        let (mut engine, now) = start("m4", Some(&["discord.com"]));
        let mut rng = Rng::new(2024);
        let base = client_hello_to(DISCORD, &hello("discord.com"));
        for round in 0..2000 {
            let mut noise = base.clone();
            let length = (rng.next_u64() % (noise.len() as u64 + 1)) as usize;
            noise.truncate(length);
            for _ in 0..4 {
                if !noise.is_empty() {
                    let at = (rng.next_u64() % noise.len() as u64) as usize;
                    noise[at] = rng.next_u64() as u8;
                }
            }
            let _ = engine.handle(&noise, round % 2 == 0, now);
        }
    }
}
