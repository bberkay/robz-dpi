//! Ham IP paketleri: ayrıştırma, yükü değiştirme, bölme ve sağlama toplamları.
//!
//! Bu kod ağdan gelen (güvenilmeyen) baytları SYSTEM yetkisiyle çalışan bir süreçte okur. Hiç `unsafe` içermez:
//! her erişim sınır denetimlidir, bozuk bir paket en kötü ihtimalle "tanımıyorum" (None) sonucunu verir.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub const TCP: u8 = 6;
pub const UDP: u8 = 17;

pub const FLAG_SYN: u8 = 0x02;
pub const FLAG_ACK: u8 = 0x10;

/// Bir paketin başlık bilgileri ve yükünün yeri. Konumlar paketin başından bayt cinsindendir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parsed {
    pub v6: bool,
    /// TCP ya da UDP
    pub proto: u8,
    pub src: IpAddr,
    pub dst: IpAddr,
    /// IPv4 TTL ya da IPv6 Hop Limit
    pub ttl: u8,
    /// TCP/UDP başlığının başladığı yer
    pub transport: usize,
    /// Yükün başladığı yer
    pub payload: usize,
    /// Paketin (IP başlığına göre) bittiği yer
    pub end: usize,
    pub src_port: u16,
    pub dst_port: u16,
    /// Yalnız TCP
    pub flags: u8,
    pub seq: u32,
    pub ack: u32,
}

impl Parsed {
    pub fn payload_len(&self) -> usize {
        self.end - self.payload
    }
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// IPv4 ya da IPv6 üstündeki TCP/UDP paketini ayrıştırır. Parçalanmış (IP fragment) paketler, başka protokoller
/// ve boyu tutmayan paketler None döner.
pub fn parse(packet: &[u8]) -> Option<Parsed> {
    let (v6, proto, src, dst, ttl, transport, end) = match packet.first()? >> 4 {
        4 => {
            let header = usize::from(packet[0] & 0x0f) * 4;
            let total = usize::from(be16(packet, 2)?);
            if header < 20 || total < header || total > packet.len() {
                return None;
            }
            // Parçalanmış paket: "daha var" bayrağı ya da sıfırdan farklı konum.
            if be16(packet, 6)? & 0x3fff != 0 {
                return None;
            }
            let src = Ipv4Addr::from(<[u8; 4]>::try_from(packet.get(12..16)?).ok()?);
            let dst = Ipv4Addr::from(<[u8; 4]>::try_from(packet.get(16..20)?).ok()?);
            (false, *packet.get(9)?, IpAddr::V4(src), IpAddr::V4(dst), *packet.get(8)?, header, total)
        }
        6 => {
            let end = 40 + usize::from(be16(packet, 4)?);
            if end > packet.len() {
                return None;
            }
            let src = Ipv6Addr::from(<[u8; 16]>::try_from(packet.get(8..24)?).ok()?);
            let dst = Ipv6Addr::from(<[u8; 16]>::try_from(packet.get(24..40)?).ok()?);
            // Uzantı başlıklarını atla (atlama seçenekleri, yönlendirme, hedef seçenekleri).
            let mut next = packet[6];
            let mut at = 40;
            for _ in 0..8 {
                match next {
                    0 | 43 | 60 => {
                        let length = (usize::from(*packet.get(at + 1)?) + 1) * 8;
                        next = *packet.get(at)?;
                        at += length;
                    }
                    _ => break,
                }
            }
            (true, next, IpAddr::V6(src), IpAddr::V6(dst), packet[7], at, end)
        }
        _ => return None,
    };

    let (payload, flags, seq, ack) = match proto {
        TCP => {
            let header = usize::from(*packet.get(transport + 12)? >> 4) * 4;
            if header < 20 {
                return None;
            }
            (transport + header, *packet.get(transport + 13)?, be32(packet, transport + 4)?, be32(packet, transport + 8)?)
        }
        UDP => (transport + 8, 0, 0, 0),
        _ => return None,
    };
    if payload > end {
        return None;
    }

    Some(Parsed {
        v6,
        proto,
        src,
        dst,
        ttl,
        transport,
        payload,
        end,
        src_port: be16(packet, transport)?,
        dst_port: be16(packet, transport + 2)?,
        flags,
        seq,
        ack,
    })
}

// ---- sağlama toplamları ---------------------------------------------------------------------------------------------

fn sum16(data: &[u8], mut sum: u32) -> u32 {
    let mut chunks = data.chunks_exact(2);
    for pair in &mut chunks {
        sum += u32::from(u16::from_be_bytes([pair[0], pair[1]]));
    }
    if let [last] = chunks.remainder() {
        sum += u32::from(*last) << 8;
    }
    sum
}

fn fold(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// IPv4 başlığının ve TCP/UDP'nin sağlama toplamlarını baştan hesaplar. Paket ayrıştırılamıyorsa dokunmaz.
pub fn fix_checksums(packet: &mut [u8]) {
    let Some(p) = parse(packet) else { return };
    if !p.v6 {
        packet[10..12].fill(0);
        let checksum = fold(sum16(&packet[..p.transport], 0));
        packet[10..12].copy_from_slice(&checksum.to_be_bytes());
    }

    let field = p.transport + if p.proto == TCP { 16 } else { 6 };
    packet[field..field + 2].fill(0);
    let length = (p.end - p.transport) as u32;
    // Sözde başlık: kaynak, hedef, protokol, uzunluk.
    let mut sum = u32::from(p.proto) + (length >> 16) + (length & 0xffff);
    sum = if p.v6 { sum16(&packet[8..40], sum) } else { sum16(&packet[12..20], sum) };
    let mut checksum = fold(sum16(&packet[p.transport..p.end], sum));
    if p.proto == UDP && checksum == 0 {
        checksum = 0xffff; // UDP'de 0 "sağlama toplamı yok" demektir
    }
    packet[field..field + 2].copy_from_slice(&checksum.to_be_bytes());
}

/// TCP sağlama toplamını bilerek bozar (bir eksiltir). Sunucu paketi atar; aradaki cihaz çoğunlukla bakmaz.
pub fn corrupt_tcp_checksum(packet: &mut [u8], p: &Parsed) {
    let field = p.transport + 16;
    if let Some(value) = be16(packet, field) {
        packet[field..field + 2].copy_from_slice(&value.wrapping_sub(1).to_be_bytes());
    }
}

// ---- paketi yeniden kurma -------------------------------------------------------------------------------------------

/// IP başlığındaki uzunluk alanını paketin yeni boyuna göre yazar.
fn set_length(packet: &mut [u8], v6: bool) {
    if v6 {
        let length = (packet.len() - 40) as u16;
        packet[4..6].copy_from_slice(&length.to_be_bytes());
    } else {
        let length = packet.len() as u16;
        packet[2..4].copy_from_slice(&length.to_be_bytes());
    }
}

/// Aynı başlıklarla (IP + TCP, seçenekler dahil) ama başka bir yükle yeni paket. Sağlama toplamları hesaplanmaz.
pub fn with_payload(packet: &[u8], p: &Parsed, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(p.payload + payload.len());
    out.extend_from_slice(&packet[..p.payload]);
    out.extend_from_slice(payload);
    set_length(&mut out, p.v6);
    out
}

pub fn set_ttl(packet: &mut [u8], p: &Parsed, ttl: u8) {
    packet[if p.v6 { 7 } else { 8 }] = ttl;
}

pub fn set_seq_ack(packet: &mut [u8], p: &Parsed, seq: u32, ack: u32) {
    packet[p.transport + 4..p.transport + 8].copy_from_slice(&seq.to_be_bytes());
    packet[p.transport + 8..p.transport + 12].copy_from_slice(&ack.to_be_bytes());
}

/// TCP paketini yükün `at` baytından ikiye böler: (baş, kuyruk). Kuyruğun sıra numarası `at` kadar ilerler.
/// İkisi de geçerli sağlama toplamı taşır. `at` yükün içinde değilse None.
pub fn split_tcp(packet: &[u8], p: &Parsed, at: usize) -> Option<(Vec<u8>, Vec<u8>)> {
    if p.proto != TCP || at == 0 || at >= p.payload_len() {
        return None;
    }
    let payload = &packet[p.payload..p.end];
    let mut head = with_payload(packet, p, &payload[..at]);
    let mut tail = with_payload(packet, p, &payload[at..]);
    set_seq_ack(&mut tail, p, p.seq.wrapping_add(at as u32), p.ack);
    fix_checksums(&mut head);
    fix_checksums(&mut tail);
    Some((head, tail))
}

/// Yükü `max` bayttan büyük bir TCP paketini, sırası korunarak, en çok `max` baytlık paketlere böler.
/// Ağ kartına bölme devredildiğinde (LSO) sürücü tek pakette birkaç bin bayt verebilir; değiştirilmiş paketi
/// geri bırakırken her parçanın yol üstündeki en küçük çerçeveye sığması gerekir.
pub fn segment_tcp(packet: Vec<u8>, max: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = packet;
    loop {
        let pieces = parse(&rest).and_then(|p| split_tcp(&rest, &p, max));
        match pieces {
            Some((head, tail)) => {
                out.push(head);
                rest = tail;
            }
            None => {
                out.push(rest);
                return out;
            }
        }
    }
}

/// Hedef (giden paket) ya da kaynak (gelen paket) adresini ve bağlantı noktasını değiştirir; sağlama toplamlarını
/// yeniden hesaplar. Adres ailesi paketle aynı değilse None.
pub fn readdress(packet: &[u8], p: &Parsed, destination: bool, address: IpAddr, port: u16) -> Option<Vec<u8>> {
    let mut out = packet[..p.end].to_vec();
    match (address, p.v6) {
        (IpAddr::V4(ip), false) => {
            let at = if destination { 16 } else { 12 };
            out[at..at + 4].copy_from_slice(&ip.octets());
        }
        (IpAddr::V6(ip), true) => {
            let at = if destination { 24 } else { 8 };
            out[at..at + 16].copy_from_slice(&ip.octets());
        }
        _ => return None,
    }
    let at = p.transport + if destination { 2 } else { 0 };
    out[at..at + 2].copy_from_slice(&port.to_be_bytes());
    fix_checksums(&mut out);
    Some(out)
}

#[cfg(test)]
pub mod testing {
    //! Testlerde kullanılan paket üreticileri.
    use super::*;

    #[allow(clippy::too_many_arguments)]
    pub fn tcp(src: IpAddr, dst: IpAddr, src_port: u16, dst_port: u16, flags: u8, seq: u32, ack: u32, payload: &[u8]) -> Vec<u8> {
        let mut segment = Vec::new();
        segment.extend_from_slice(&src_port.to_be_bytes());
        segment.extend_from_slice(&dst_port.to_be_bytes());
        segment.extend_from_slice(&seq.to_be_bytes());
        segment.extend_from_slice(&ack.to_be_bytes());
        segment.extend_from_slice(&[0x50, flags, 0xfa, 0xf0, 0, 0, 0, 0]); // başlık 20 bayt, pencere 64240
        segment.extend_from_slice(payload);
        ip(src, dst, TCP, 128, &segment)
    }

    pub fn udp(src: IpAddr, dst: IpAddr, src_port: u16, dst_port: u16, payload: &[u8]) -> Vec<u8> {
        let mut datagram = Vec::new();
        datagram.extend_from_slice(&src_port.to_be_bytes());
        datagram.extend_from_slice(&dst_port.to_be_bytes());
        datagram.extend_from_slice(&((payload.len() + 8) as u16).to_be_bytes());
        datagram.extend_from_slice(&[0, 0]);
        datagram.extend_from_slice(payload);
        ip(src, dst, UDP, 128, &datagram)
    }

    pub fn ip(src: IpAddr, dst: IpAddr, proto: u8, ttl: u8, body: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        match (src, dst) {
            (IpAddr::V4(src), IpAddr::V4(dst)) => {
                packet.extend_from_slice(&[0x45, 0]);
                packet.extend_from_slice(&((20 + body.len()) as u16).to_be_bytes());
                packet.extend_from_slice(&[0x12, 0x34, 0x40, 0x00, ttl, proto, 0, 0]); // parçalama yok (DF)
                packet.extend_from_slice(&src.octets());
                packet.extend_from_slice(&dst.octets());
            }
            (IpAddr::V6(src), IpAddr::V6(dst)) => {
                packet.extend_from_slice(&[0x60, 0, 0, 0]);
                packet.extend_from_slice(&(body.len() as u16).to_be_bytes());
                packet.extend_from_slice(&[proto, ttl]);
                packet.extend_from_slice(&src.octets());
                packet.extend_from_slice(&dst.octets());
            }
            _ => panic!("adres aileleri aynı olmalı"),
        }
        packet.extend_from_slice(body);
        fix_checksums(&mut packet);
        packet
    }

    /// Sağlama toplamları doğru mu? Doğru bir pakette yeniden hesaplamak hiçbir baytı değiştirmez.
    pub fn checksums_ok(packet: &[u8]) -> bool {
        let mut copy = packet.to_vec();
        fix_checksums(&mut copy);
        copy == packet
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn v6(last: u16) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, last))
    }

    #[test]
    fn known_ipv4_header_checksum() {
        // RFC 1071'deki gibi elle doğrulanabilen örnek: 45 00 00 73 00 00 40 00 40 11 [b8 61] c0 a8 00 01 c0 a8 00 c7
        let mut packet = vec![0x45, 0, 0, 0x73, 0, 0, 0x40, 0, 0x40, 0x11, 0, 0, 0xc0, 0xa8, 0, 1, 0xc0, 0xa8, 0, 0xc7];
        packet.resize(0x73, 0);
        packet[24..26].copy_from_slice(&(0x73u16 - 20).to_be_bytes()); // UDP uzunluğu
        fix_checksums(&mut packet);
        assert_eq!(&packet[10..12], &[0xb8, 0x61]);
    }

    #[test]
    fn parses_tcp_and_udp_on_both_families() {
        let packet = tcp(v4(192, 168, 1, 10), v4(162, 159, 128, 233), 50000, 443, 0x18, 1000, 2000, b"hello");
        let p = parse(&packet).unwrap();
        assert_eq!((p.v6, p.proto, p.src_port, p.dst_port, p.seq, p.ack, p.ttl), (false, TCP, 50000, 443, 1000, 2000, 128));
        assert_eq!(&packet[p.payload..p.end], b"hello");
        assert_eq!(p.dst, v4(162, 159, 128, 233));

        let packet = udp(v6(1), v6(2), 5353, 53, b"query");
        let p = parse(&packet).unwrap();
        assert_eq!((p.v6, p.proto, p.dst_port, p.payload_len()), (true, UDP, 53, 5));
        assert!(checksums_ok(&packet));
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x45]), None);
        assert_eq!(parse(&[0xff; 64]), None);
        let good = tcp(v4(1, 1, 1, 1), v4(2, 2, 2, 2), 1, 2, 0x10, 0, 0, b"abcdef");
        // Her kesilmiş uzunluk ve her tek bayt bozulması güvenle ele alınır.
        for cut in 0..good.len() {
            let _ = parse(&good[..cut]);
        }
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] ^= 0xff;
            let _ = parse(&bad);
            fix_checksums(&mut bad);
        }
        // IP parçası (fragment) işlenmez.
        let mut fragment = good.clone();
        fragment[6] = 0x20;
        assert_eq!(parse(&fragment), None);
    }

    #[test]
    fn splits_a_segment_in_two() {
        for (src, dst) in [(v4(10, 0, 0, 2), v4(162, 159, 128, 233)), (v6(1), v6(2))] {
            let packet = tcp(src, dst, 50000, 443, 0x18, 0xffff_fffe, 77, b"0123456789");
            let p = parse(&packet).unwrap();
            let (head, tail) = split_tcp(&packet, &p, 2).unwrap();
            let (hp, tp) = (parse(&head).unwrap(), parse(&tail).unwrap());
            assert_eq!(&head[hp.payload..hp.end], b"01");
            assert_eq!(&tail[tp.payload..tp.end], b"23456789");
            assert_eq!(hp.seq, 0xffff_fffe);
            assert_eq!(tp.seq, 0, "sıra numarası 32 bitte döner");
            assert_eq!((tp.ack, tp.flags, tp.dst_port), (77, 0x18, 443));
            assert!(checksums_ok(&head) && checksums_ok(&tail));
            assert_eq!(head.len() + tail.len(), packet.len() + p.payload);
        }
        let packet = tcp(v4(1, 1, 1, 1), v4(2, 2, 2, 2), 1, 443, 0x18, 0, 0, b"abc");
        let p = parse(&packet).unwrap();
        assert!(split_tcp(&packet, &p, 0).is_none());
        assert!(split_tcp(&packet, &p, 3).is_none());
        assert!(split_tcp(&packet, &p, 99).is_none());
    }

    #[test]
    fn replaces_payload_and_readdresses() {
        let packet = tcp(v4(10, 0, 0, 2), v4(9, 9, 9, 9), 50000, 443, 0x18, 5, 6, b"real");
        let p = parse(&packet).unwrap();
        let mut fake = with_payload(&packet, &p, b"a much longer fake payload");
        set_ttl(&mut fake, &p, 5);
        fix_checksums(&mut fake);
        let fp = parse(&fake).unwrap();
        assert_eq!((fp.ttl, fp.seq, fp.payload_len()), (5, 5, 26));
        assert!(checksums_ok(&fake));
        corrupt_tcp_checksum(&mut fake, &fp);
        assert!(!checksums_ok(&fake));

        let query = udp(v4(10, 0, 0, 2), v4(192, 168, 1, 1), 61000, 53, b"dns?");
        let p = parse(&query).unwrap();
        let moved = readdress(&query, &p, true, v4(9, 9, 9, 9), 9953).unwrap();
        let mp = parse(&moved).unwrap();
        assert_eq!((mp.dst, mp.dst_port, mp.src, mp.src_port), (v4(9, 9, 9, 9), 9953, v4(10, 0, 0, 2), 61000));
        assert!(checksums_ok(&moved));
        assert!(readdress(&query, &p, true, v6(1), 53).is_none(), "IPv4 paketine IPv6 adresi yazılmaz");
    }
}
