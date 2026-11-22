//! DNS iletileri: sorgudaki alan adını ve yanıttaki adresleri okuma.
//!
//! Motor DNS iletisinin içeriğini değiştirmez; yalnızca okur. Sorgudaki ad, o sorgunun yönlendirilip
//! yönlendirilmeyeceğine karar vermek içindir. Yanıttaki adresler, hedef sitelerin IP adreslerini öğrenmek içindir
//! (alan adı görünmeyen bağlantılarda karar verebilmek için).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const HEADER: usize = 12;
const TYPE_A: u16 = 1;
const TYPE_AAAA: u16 = 28;

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

/// İletinin kimliği (sorgu ile yanıtı eşlemek için).
pub fn id(message: &[u8]) -> Option<u16> {
    be16(message, 0)
}

/// `at` konumundaki adı okur: (ad, addan sonraki konum). Sıkıştırma işaretçileri izlenir, döngüye girilmez.
fn name(message: &[u8], mut at: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut after = None;
    for _ in 0..64 {
        let length = usize::from(*message.get(at)?);
        match length {
            0 => return Some((out, after.unwrap_or(at + 1))),
            1..=63 => {
                let label = message.get(at + 1..at + 1 + length)?;
                if !out.is_empty() {
                    out.push('.');
                }
                for byte in label {
                    // Yalnız alan adlarında görülen karakterler; gerisi bizi ilgilendiren bir ad değildir.
                    if !(byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_') {
                        return None;
                    }
                    out.push(byte.to_ascii_lowercase() as char);
                }
                if out.len() > 253 {
                    return None;
                }
                at += 1 + length;
            }
            _ if length & 0xc0 == 0xc0 => {
                let pointer = usize::from(be16(message, at)? & 0x3fff);
                after.get_or_insert(at + 2);
                if pointer >= at {
                    return None; // ileriye işaret eden işaretçi: bozuk ya da kötü niyetli
                }
                at = pointer;
            }
            _ => return None,
        }
    }
    None
}

/// Tek sorulu standart bir sorguysa sorulan alan adı.
pub fn query_name(message: &[u8]) -> Option<String> {
    let flags = be16(message, 2)?;
    // QR = 0 (sorgu), işlem kodu = 0 (standart), tam bir soru
    if flags & 0xf800 != 0 || be16(message, 4)? != 1 {
        return None;
    }
    name(message, HEADER).map(|(name, _)| name).filter(|name| !name.is_empty())
}

/// Yanıtsa: sorulan alan adı ve yanıttaki A/AAAA adresleri.
pub fn answers(message: &[u8]) -> Option<(String, Vec<IpAddr>)> {
    let flags = be16(message, 2)?;
    if flags & 0x8000 == 0 || be16(message, 4)? != 1 {
        return None;
    }
    let (question, mut at) = name(message, HEADER)?;
    at += 4; // soru türü ve sınıfı
    let count = usize::from(be16(message, 6)?).min(64);
    let mut addresses = Vec::new();
    for _ in 0..count {
        let Some((_, next)) = name(message, at) else { break };
        let (Some(kind), Some(length)) = (be16(message, next), be16(message, next + 8)) else { break };
        let data_at = next + 10;
        let Some(data) = message.get(data_at..data_at + usize::from(length)) else { break };
        match (kind, data.len()) {
            (TYPE_A, 4) => addresses.push(IpAddr::V4(Ipv4Addr::new(data[0], data[1], data[2], data[3]))),
            (TYPE_AAAA, 16) => {
                let bytes: [u8; 16] = data.try_into().ok()?;
                addresses.push(IpAddr::V6(Ipv6Addr::from(bytes)));
            }
            _ => {}
        }
        at = data_at + usize::from(length);
    }
    Some((question, addresses))
}

#[cfg(test)]
pub mod testing {
    //! Testlerde kullanılan DNS iletisi üreticileri.
    use super::*;

    fn labels(out: &mut Vec<u8>, name: &str) {
        for label in name.split('.') {
            out.push(label.len() as u8);
            out.extend_from_slice(label.as_bytes());
        }
        out.push(0);
    }

    pub fn query(id: u16, name: &str) -> Vec<u8> {
        let mut out = id.to_be_bytes().to_vec();
        out.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
        labels(&mut out, name);
        out.extend_from_slice(&[0, 1, 0, 1]);
        out
    }

    /// CNAME + adresler içeren yanıt; adlar sıkıştırma işaretçisiyle sorudaki ada bağlanır.
    pub fn response(id: u16, name: &str, addresses: &[IpAddr]) -> Vec<u8> {
        let mut out = id.to_be_bytes().to_vec();
        out.extend_from_slice(&[0x81, 0x80, 0, 1]);
        out.extend_from_slice(&((addresses.len() + 1) as u16).to_be_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        labels(&mut out, name);
        out.extend_from_slice(&[0, 1, 0, 1]);
        // CNAME kaydı: ad -> cdn.<ad> (hedef, sorudaki ada işaret eder)
        out.extend_from_slice(&[0xc0, 12, 0, 5, 0, 1, 0, 0, 0, 60, 0, 6, 3, b'c', b'd', b'n', 0xc0, 12]);
        for address in addresses {
            out.extend_from_slice(&[0xc0, 12]);
            match address {
                IpAddr::V4(ip) => {
                    out.extend_from_slice(&[0, 1, 0, 1, 0, 0, 0, 60, 0, 4]);
                    out.extend_from_slice(&ip.octets());
                }
                IpAddr::V6(ip) => {
                    out.extend_from_slice(&[0, 28, 0, 1, 0, 0, 0, 60, 0, 16]);
                    out.extend_from_slice(&ip.octets());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn reads_queries() {
        let message = query(0xbeef, "Gateway.Discord.gg");
        assert_eq!(id(&message), Some(0xbeef));
        assert_eq!(query_name(&message).as_deref(), Some("gateway.discord.gg"));
        assert_eq!(answers(&message), None, "sorgu yanıt değildir");
    }

    #[test]
    fn reads_answers_through_compression() {
        let v4 = IpAddr::V4(Ipv4Addr::new(162, 159, 128, 233));
        let v6 = IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 1));
        let message = response(7, "discord.com", &[v4, v6]);
        assert_eq!(query_name(&message), None, "yanıt sorgu değildir");
        assert_eq!(answers(&message), Some(("discord.com".to_string(), vec![v4, v6])));
    }

    #[test]
    fn survives_broken_messages() {
        assert_eq!(query_name(&[]), None);
        assert_eq!(query_name(&[0; 12]), None);
        // Kendine işaret eden işaretçi: sonsuz döngü yok.
        let mut looped = query(1, "a.com");
        looped[12] = 0xc0;
        looped[13] = 12;
        assert_eq!(query_name(&looped), None);

        let good = response(1, "roblox.com", &[IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))]);
        for cut in 0..good.len() {
            let _ = answers(&good[..cut]);
            let _ = query_name(&good[..cut]);
        }
        for i in 0..good.len() {
            let mut bad = good.clone();
            bad[i] ^= 0xff;
            let _ = answers(&bad);
        }
        // Kesilmiş yanıt, o ana kadar okunan adresleri verir.
        let two = response(1, "roblox.com", &[IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8))]);
        let (_, partial) = answers(&two[..two.len() - 3]).unwrap();
        assert_eq!(partial, vec![IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))]);
    }
}
