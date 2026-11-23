//! Linux: paketleri çekirdekten alıp geri bırakan katman (Windows'taki WinDivert'in karşılığı).
//!
//! İki parçadır:
//!   * nftables kuralları: hangi paketlerin motora geleceğini çekirdekte seçer ve onları bir kuyruğa (nfqueue) koyar.
//!     Bir bağlantıdan motora yalnız ilk birkaç veri paketi gelir; indirme, yükleme ve yayın motora uğramaz.
//!   * Kuyruk ve ham yuva: motor kuyruktaki paketi okur, kararını bildirir; ürettiği paketleri (sahte istek, bölünmüş
//!     parçalar) işaretli bir ham yuvadan gönderir. İşaret (fwmark), bu paketlerin yeniden kuyruğa girmesini önler.
//!
//! Ek sürücü gerekmez: nfnetlink_queue ve nf_tables Linux çekirdeğinin parçasıdır.

use crate::plan::EngineRequest;
use nfq::{Message, Queue};
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Motorun kuyruk numarası.
pub const QUEUE: u16 = 5377;
/// Motorun kendi gönderdiği paketlerin işareti: kurallar bu işareti taşıyan paketi kuyruğa koymaz.
pub const MARK: u32 = 0x2000_0000;
/// nftables tablosu: `nft list table inet robzdpi`.
pub const TABLE: &str = "robzdpi";

// Çekirdeğin kanca numaraları (linux/netfilter.h).
const NF_INET_LOCAL_OUT: u8 = 3;

/// nft'nin bulunabileceği yerler (servis ortamında PATH dar olabilir).
const NFT_PATHS: [&str; 4] = ["/usr/sbin/nft", "/sbin/nft", "/usr/bin/nft", "/usr/local/sbin/nft"];

pub fn nft_path() -> Option<&'static str> {
    NFT_PATHS.into_iter().find(|path| std::path::Path::new(path).is_file())
}

/// Motora hangi paketlerin geleceğini seçen nftables kuralları (Windows'taki süzgecin karşılığı, filter.rs).
///
/// Giden yön (postrouting): 443'e (ve gerekiyorsa 80'e) giden bağlantıların ilk veri paketleri, DNS sorguları ve
/// istenirse QUIC'in ilk paketi. Gelen yön (input): sunucunun ilk yanıtı (otomatik ömür için) ve DNS yanıtları.
/// "bypass": motor çalışmıyorsa paketler beklemeden geçer; bağlantı kesilmez.
pub fn rules(request: &EngineRequest) -> String {
    let strategy = &request.strategy;
    let queue = format!("queue num {QUEUE} bypass");
    // Bağlantının ilk paketleri: 1. paket SYN, 2. paket ACK, sonrakiler veridir. Yalnız veri taşıyanlar
    // (72 bayttan büyükler) ve bağlantının başındakiler kuyruğa girer.
    let early = "ct original packets 2-12 meta length > 72 tcp flags & (syn | fin | rst) == 0";

    let mut out = vec![
        "oifname \"lo\" return".to_string(),
        format!("meta mark & 0x{MARK:08x} != 0 return"),
        format!("tcp dport 443 {early} {queue}"),
    ];
    if strategy.http_split > 0 || strategy.fakes() {
        out.push(format!("tcp dport 80 {early} {queue}"));
    }

    let mut inbound = vec!["iifname \"lo\" return".to_string()];
    // Otomatik sahte paket ömrü: sunucunun ilk yanıtındaki (SYN+ACK) TTL gerekir.
    if strategy.fake_ttl_mode == "auto" {
        inbound.push(format!("tcp sport {{ 80, 443 }} tcp flags & (syn | ack) == syn | ack {queue}"));
    }

    // Hedef adreslerini DNS yanıtlarından öğrenmek gerekiyor mu?
    // Hariç tutulan sitelerin adresleri de DNS yanıtlarından öğrenilir.
    let learns = (request.domains.is_some() && (strategy.no_sni == "known" || strategy.block_quic)) || !request.excluded.is_empty();
    let mut response_ports = Vec::new();
    if let Some(dns) = &request.dns {
        out.push(format!("udp dport 53 {queue}"));
        response_ports.push(dns.port);
        if dns.v6.is_some() {
            response_ports.push(dns.port6);
        }
    } else if learns {
        response_ports.push(53);
    }
    response_ports.sort_unstable();
    response_ports.dedup();
    if !response_ports.is_empty() {
        let ports: Vec<String> = response_ports.iter().map(u16::to_string).collect();
        inbound.push(format!("udp sport {{ {} }} {queue}", ports.join(", ")));
    }

    // QUIC'in ilk paketi en az 1200 baytlık bir UDP yüküdür (8 baytlık UDP başlığıyla 1208).
    if strategy.block_quic {
        out.push(format!("udp dport 443 udp length >= 1208 {queue}"));
    }

    let chain = |name: &str, hook: &str, priority: &str, lines: &[String]| {
        format!(
            "  chain {name} {{\n    type filter hook {hook} priority {priority}; policy accept;\n{}\n  }}\n",
            lines.iter().map(|line| format!("    {line}")).collect::<Vec<_>>().join("\n")
        )
    };
    // Gelen paketler güvenlik duvarından (öncelik 0) sonra okunur: yalnız zaten kabul edilmiş paketler görülür.
    format!("table inet {TABLE} {{\n{}{}}}\n", chain("out", "postrouting", "mangle", &out), chain("in", "input", "100", &inbound))
}

fn nft(arguments: &[&str], input: Option<&str>) -> Result<(), String> {
    let path = nft_path().ok_or("nft bulunamadı (nftables kurulu değil)")?;
    let mut child = Command::new(path)
        .args(arguments)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let output = child.wait_with_output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Kuralları yükler; önceki bir çalışmadan kalan tablo varsa önce silinir.
pub fn install_rules(text: &str) -> Result<(), String> {
    remove_rules();
    nft(&["-f", "-"], Some(text))
}

pub fn remove_rules() {
    let _ = nft(&["delete", "table", "inet", TABLE], None);
}

/// Kuyruk ve motorun paket gönderdiği ham yuvalar.
pub struct Capture {
    queue: Queue,
    raw4: OwnedFd,
    raw6: Option<OwnedFd>,
}

impl Capture {
    pub fn open() -> io::Result<Self> {
        let mut queue = Queue::open()?;
        queue.bind(QUEUE)?;
        // Paketin tamamı kopyalansın; kuyruk dolarsa paketler beklemeden geçsin.
        queue.set_copy_range(QUEUE, 0xffff)?;
        let _ = queue.set_fail_open(QUEUE, true);
        queue.set_nonblocking(true);
        // IPv6 kapalı bir sistemde ham IPv6 yuvası açılamaz; o zaman yalnız IPv4 paketleri gönderilir.
        Ok(Self { queue, raw4: raw_socket(libc::AF_INET)?, raw6: raw_socket(libc::AF_INET6).ok() })
    }

    /// Sıradaki paketi bekler; `timeout` içinde paket gelmezse None döner (durdurma isteği denetlensin diye).
    pub fn recv(&mut self, timeout: Duration) -> io::Result<Option<Message>> {
        let mut poll = libc::pollfd { fd: self.queue.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: tek bir pollfd yapısı verilir ve çağrı boyunca yaşar.
        let ready = unsafe { libc::poll(&mut poll, 1, timeout.as_millis() as libc::c_int) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::Interrupted { Ok(None) } else { Err(error) };
        }
        if ready == 0 {
            return Ok(None);
        }
        match self.queue.recv() {
            Ok(message) => Ok(Some(message)),
            // Kuyruk taştıysa (ENOBUFS) çekirdek paketleri kendisi geçirir; döngü sürer.
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ENOBUFS) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn verdict(&mut self, message: Message) -> io::Result<()> {
        self.queue.verdict(message)
    }

    /// Motorun ürettiği bir IP paketini gönderir. Paket işaretlidir: kurallar onu yeniden kuyruğa koymaz.
    pub fn send(&self, packet: &[u8]) -> io::Result<()> {
        match packet.first().map(|byte| byte >> 4) {
            Some(4) if packet.len() >= 20 => {
                // SAFETY: sockaddr_in düz veridir.
                let mut address: libc::sockaddr_in = unsafe { std::mem::zeroed() };
                address.sin_family = libc::AF_INET as libc::sa_family_t;
                address.sin_addr.s_addr = u32::from_ne_bytes([packet[16], packet[17], packet[18], packet[19]]);
                send_to(&self.raw4, packet, (&raw const address).cast(), size_of::<libc::sockaddr_in>())
            }
            Some(6) if packet.len() >= 40 => {
                let Some(raw6) = &self.raw6 else { return Err(io::Error::other("IPv6 ham yuvası yok")) };
                // SAFETY: sockaddr_in6 düz veridir.
                let mut address: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
                address.sin6_family = libc::AF_INET6 as libc::sa_family_t;
                address.sin6_addr.s6_addr.copy_from_slice(&packet[24..40]);
                send_to(raw6, packet, (&raw const address).cast(), size_of::<libc::sockaddr_in6>())
            }
            _ => Err(io::Error::other("geçersiz paket")),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.queue.unbind(QUEUE);
    }
}

/// Paketin giden yönde mi yakalandığı (kuralların "out" zinciri) yoksa gelen yönde mi ("in" zinciri).
pub fn outbound(message: &Message) -> bool {
    message.get_hook() >= NF_INET_LOCAL_OUT
}

/// IP başlığını kendimiz yazdığımız ham yuva (IPPROTO_RAW). Gönderdiği her paket MARK ile işaretlenir.
fn raw_socket(family: libc::c_int) -> io::Result<OwnedFd> {
    // SAFETY: socket yeni bir dosya tanıtıcısı döndürür; sahipliği OwnedFd'ye geçer.
    let socket = unsafe {
        let fd = libc::socket(family, libc::SOCK_RAW | libc::SOCK_CLOEXEC, libc::IPPROTO_RAW);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        OwnedFd::from_raw_fd(fd)
    };
    let mark: u32 = MARK;
    // SAFETY: seçenek değeri çağrı boyunca yaşayan bir u32'dir.
    let result = unsafe {
        libc::setsockopt(socket.as_raw_fd(), libc::SOL_SOCKET, libc::SO_MARK, (&raw const mark).cast(), size_of::<u32>() as libc::socklen_t)
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(socket)
}

fn send_to(socket: &OwnedFd, packet: &[u8], address: *const libc::sockaddr, length: usize) -> io::Result<()> {
    // SAFETY: packet ve address çağrı boyunca yaşar; length address'in gerçek boyudur.
    let sent = unsafe { libc::sendto(socket.as_raw_fd(), packet.as_ptr().cast(), packet.len(), 0, address, length as libc::socklen_t) };
    if sent < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use crate::plan::DnsTarget;

    fn request(method: &str, dns: bool, selected: bool) -> EngineRequest {
        EngineRequest {
            strategy: catalog::METHODS.iter().find(|m| m.id == method).expect("yöntem").strategy(),
            dns: dns.then(|| DnsTarget { v4: "9.9.9.9".into(), port: 9953, v6: Some("2620:fe::fe".into()), port6: 9953, all: false }),
            domains: selected.then(|| vec!["discord.com".to_string()]),
            excluded: Vec::new(),
        }
    }

    #[test]
    fn rules_follow_the_request() {
        let m1 = rules(&request("m1", true, true));
        assert!(m1.contains("table inet robzdpi"));
        assert!(m1.contains("tcp dport 443") && m1.contains("tcp dport 80"));
        assert!(m1.contains("udp dport 53") && m1.contains("udp sport { 9953 }"));
        assert!(!m1.contains("syn | ack) == syn | ack"), "sabit ömürde sunucunun yanıtı okunmaz");
        assert!(!m1.contains("udp dport 443"), "QUIC kapatılmıyor");

        let m2 = rules(&request("m2", false, true));
        assert!(m2.contains("tcp sport { 80, 443 } tcp flags & (syn | ack) == syn | ack"), "otomatik ömür");
        assert!(!m2.contains("udp dport 53"), "DNS'e dokunulmuyor");
        assert!(m2.contains("udp sport { 53 }"), "adresler DNS yanıtlarından öğrenilir");

        let m4 = rules(&request("m4", false, false));
        assert!(m4.contains("udp dport 443 udp length >= 1208"), "QUIC'in ilk paketi");
        assert!(!m4.contains("udp sport"), "bütün siteler kipinde adres öğrenmek gerekmez");
    }

    #[test]
    fn engine_packets_are_never_queued_again() {
        let text = rules(&request("m1", true, true));
        let mark = text.find("meta mark & 0x20000000 != 0 return").expect("işaret kuralı");
        let first_queue = text.find("queue num").expect("kuyruk kuralı");
        assert!(mark < first_queue, "işaret denetimi kuyruk kurallarından önce gelmeli");
        assert!(text.matches("bypass").count() == text.matches("queue num").count(), "motor yokken paketler geçmeli");
    }
}
