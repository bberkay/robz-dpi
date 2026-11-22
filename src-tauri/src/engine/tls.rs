//! TLS ClientHello: içindeki alan adını (SNI) okuma ve sahte bir ClientHello üretme.
//!
//! GoodbyeDPI alan adını bayt örüntüsü arayarak bulur; burada kayıt gerçekten ayrıştırılır. Böylece "alan adı bu
//! pakette yok çünkü ClientHello sonraki pakette sürüyor" durumu "alan adı hiç yok" durumundan ayrılır. Yeni
//! tarayıcıların ClientHello'su (kuantum sonrası anahtar paylaşımı yüzünden) tek pakete sığmaz ve uzantıların
//! sırası karıştırıldığı için alan adı ikinci pakette kalabilir.

/// Bir TCP yükünün başındaki ClientHello hakkında bildiğimiz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hello {
    /// Alan adı okundu. `offset`: alan adının yük içindeki yeri (tam önünden bölmek için).
    Sni { host: String, offset: usize },
    /// ClientHello bu pakette bitiyor ve alan adı uzantısı yok.
    NoSni,
    /// ClientHello bu paketten uzun; alan adı görünmüyor (sonraki pakette olabilir).
    Truncated,
    /// Bu bir TLS ClientHello değil.
    Other,
}

/// Sınır denetimli okuyucu: veri biterse None.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<usize> {
        let value = *self.data.get(self.at)?;
        self.at += 1;
        Some(usize::from(value))
    }

    fn u16(&mut self) -> Option<usize> {
        Some(self.u8()? << 8 | self.u8()?)
    }

    fn skip(&mut self, count: usize) -> Option<()> {
        (self.at + count <= self.data.len()).then(|| self.at += count)
    }
}

fn valid_host(bytes: &[u8]) -> Option<String> {
    let ok = (3..=253).contains(&bytes.len())
        && bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'.' || *b == b'-');
    ok.then(|| String::from_utf8_lossy(bytes).to_ascii_lowercase())
}

/// TCP yükünün başındaki TLS ClientHello'dan alan adını okur.
pub fn inspect(payload: &[u8]) -> Hello {
    // Kayıt başlığı: tür 0x16 (el sıkışma), sürüm 0x03 xx, uzunluk. Ardından el sıkışma türü 0x01 (ClientHello).
    if payload.len() < 6 || payload[0] != 0x16 || payload[1] != 0x03 || payload[5] != 0x01 {
        return Hello::Other;
    }
    let mut r = Reader { data: payload, at: 9 }; // kayıt başlığı (5) + el sıkışma başlığı (4)
    walk(&mut r).unwrap_or(Hello::Truncated)
}

/// None: veri ClientHello bitmeden bitti.
fn walk(r: &mut Reader) -> Option<Hello> {
    r.skip(2 + 32)?; // sürüm, rastgele
    let session = r.u8()?;
    r.skip(session)?;
    let ciphers = r.u16()?;
    r.skip(ciphers)?;
    let compression = r.u8()?;
    r.skip(compression)?;
    if r.at == r.data.len() {
        // Uzantısız (çok eski) ClientHello tam burada biter; ama yük tam burada kesilmiş de olabilir.
        return None;
    }
    let extensions_end = r.at + 2 + r.u16()?;
    while r.at + 4 <= extensions_end {
        let kind = r.u16()?;
        let length = r.u16()?;
        if kind != 0 {
            r.skip(length)?;
            continue;
        }
        // server_name: liste uzunluğu (2), ad türü (1, 0 = alan adı), ad uzunluğu (2), ad
        r.skip(2)?;
        if r.u8()? != 0 {
            return Some(Hello::NoSni);
        }
        let name_length = r.u16()?;
        let offset = r.at;
        let name = r.data.get(offset..offset + name_length)?;
        return Some(match valid_host(name) {
            Some(host) => Hello::Sni { host, offset },
            None => Hello::NoSni,
        });
    }
    Some(Hello::NoSni)
}

// ---- sahte ClientHello ----------------------------------------------------------------------------------------------

/// Küçük, hızlı bir sözde rastgele sayı üreteci (xorshift64*). Şifreleme için değildir: yalnızca sahte paketlerin
/// her seferinde farklı görünmesini sağlar.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn fill(&mut self, out: &mut Vec<u8>, count: usize) {
        for _ in 0..count {
            out.push((self.next_u64() >> 32) as u8);
        }
    }
}

fn extension(out: &mut Vec<u8>, kind: u16, body: &[u8]) {
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(body);
}

/// Tarayıcılarınkine benzeyen, `host` alan adını taşıyan 517 baytlık bir ClientHello kaydı.
/// Rastgele alanlar (rastgele değer, oturum kimliği, anahtar paylaşımı) her çağrıda farklıdır.
pub fn fake_hello(host: &str, rng: &mut Rng) -> Vec<u8> {
    const RECORD: usize = 517; // tarayıcıların dolgu ile ulaştığı alışılmış boy

    let mut extensions = Vec::new();
    let mut sni = Vec::new();
    sni.extend_from_slice(&((host.len() + 3) as u16).to_be_bytes());
    sni.push(0);
    sni.extend_from_slice(&(host.len() as u16).to_be_bytes());
    sni.extend_from_slice(host.as_bytes());
    extension(&mut extensions, 0x0000, &sni);
    extension(&mut extensions, 0x0017, &[]); // extended_master_secret
    extension(&mut extensions, 0xff01, &[0]); // renegotiation_info
    extension(&mut extensions, 0x000a, &[0, 6, 0, 0x1d, 0, 0x17, 0, 0x18]); // supported_groups
    extension(&mut extensions, 0x000b, &[1, 0]); // ec_point_formats
    extension(&mut extensions, 0x0023, &[]); // session_ticket
    extension(&mut extensions, 0x0010, b"\x00\x0c\x02h2\x08http/1.1"); // ALPN
    extension(&mut extensions, 0x0005, &[1, 0, 0, 0, 0]); // status_request
    extension(
        &mut extensions,
        0x000d, // signature_algorithms
        &[0, 16, 4, 3, 8, 4, 4, 1, 5, 3, 8, 5, 5, 1, 8, 6, 6, 1],
    );
    extension(&mut extensions, 0x002b, &[4, 3, 4, 3, 3]); // supported_versions: TLS 1.3, 1.2
    extension(&mut extensions, 0x002d, &[1, 1]); // psk_key_exchange_modes
    let mut key_share = vec![0, 36, 0, 0x1d, 0, 32];
    rng.fill(&mut key_share, 32);
    extension(&mut extensions, 0x0033, &key_share);

    let mut body = vec![0x03, 0x03];
    rng.fill(&mut body, 32);
    body.push(32);
    rng.fill(&mut body, 32);
    let ciphers: [u16; 15] = [
        0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8, 0xc013, 0xc014, 0x009c, 0x009d, 0x002f,
        0x0035,
    ];
    body.extend_from_slice(&((ciphers.len() * 2) as u16).to_be_bytes());
    for cipher in ciphers {
        body.extend_from_slice(&cipher.to_be_bytes());
    }
    body.extend_from_slice(&[1, 0]); // sıkıştırma yok

    // Dolgu uzantısı: kayıt toplam RECORD bayt olsun (kayıt 5 + el sıkışma 4 + gövde + uzantı uzunluğu 2).
    let used = 5 + 4 + body.len() + 2 + extensions.len();
    if used + 4 <= RECORD {
        extension(&mut extensions, 0x0015, &vec![0; RECORD - used - 4]);
    }
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    let mut record = vec![0x16, 0x03, 0x01];
    record.extend_from_slice(&((body.len() + 4) as u16).to_be_bytes());
    record.push(0x01);
    record.extend_from_slice(&[0, (body.len() >> 8) as u8, body.len() as u8]);
    record.extend_from_slice(&body);
    record
}

/// Sahte, şifresiz HTTP isteği.
pub fn fake_http(host: &str) -> Vec<u8> {
    format!("GET / HTTP/1.1\r\nHost: {host}\r\nUser-Agent: curl/8.5.0\r\nAccept: */*\r\n\r\n").into_bytes()
}

// ---- HTTP -----------------------------------------------------------------------------------------------------------

const METHODS: [&[u8]; 7] = [b"GET ", b"POST ", b"HEAD ", b"PUT ", b"DELETE ", b"OPTIONS ", b"CONNECT "];

/// Şifresiz HTTP isteğindeki Host başlığının değeri.
pub fn http_host(payload: &[u8]) -> Option<String> {
    if !METHODS.iter().any(|method| payload.starts_with(method)) {
        return None;
    }
    const NEEDLE: &[u8] = b"\r\nHost: ";
    let start = payload.windows(NEEDLE.len()).position(|w| w.eq_ignore_ascii_case(NEEDLE))? + NEEDLE.len();
    let rest = &payload[start..];
    let end = rest.windows(2).position(|w| w == b"\r\n")?;
    // Bağlantı noktası varsa at: "example.com:8080"
    let value = &rest[..end];
    let host = value.split(|b| *b == b':').next()?;
    valid_host(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_hello_is_a_real_looking_client_hello() {
        let mut rng = Rng::new(42);
        let hello = fake_hello("www.w3.org", &mut rng);
        assert_eq!(hello.len(), 517);
        assert_eq!(&hello[..3], &[0x16, 0x03, 0x01]);
        assert_eq!(usize::from(u16::from_be_bytes([hello[3], hello[4]])), hello.len() - 5, "kayıt uzunluğu tutarlı");
        match inspect(&hello) {
            Hello::Sni { host, offset } => {
                assert_eq!(host, "www.w3.org");
                assert_eq!(&hello[offset..offset + 10], b"www.w3.org");
            }
            other => panic!("alan adı okunamadı: {other:?}"),
        }
        // Rastgele alanlar her seferinde değişir, alan adı değişmez.
        let again = fake_hello("www.w3.org", &mut rng);
        assert_ne!(hello, again);
        assert_eq!(inspect(&again), inspect(&hello));
        // Çok uzun alan adında dolgu sığmaz ama kayıt yine geçerlidir.
        let long = format!("{}.example.com", "a".repeat(200));
        assert!(matches!(inspect(&fake_hello(&long, &mut rng)), Hello::Sni { host, .. } if host == long));
    }

    #[test]
    fn tells_truncated_from_absent() {
        let mut rng = Rng::new(7);
        let hello = fake_hello("discord.com", &mut rng);
        let Hello::Sni { offset, .. } = inspect(&hello) else { panic!() };

        // Alan adından önce kesilen her parça "sürüyor" der; asla yanlış bir ad uydurmaz.
        for cut in 6..offset + 11 {
            assert_eq!(inspect(&hello[..cut]), Hello::Truncated, "kesme noktası {cut}");
        }
        // Alan adı tam göründüğü andan itibaren okunur.
        for cut in offset + 11..hello.len() {
            assert!(matches!(inspect(&hello[..cut]), Hello::Sni { ref host, .. } if host == "discord.com"), "{cut}");
        }
    }

    #[test]
    fn hello_without_sni() {
        // server_name uzantısı olmayan tam bir ClientHello.
        let mut body = vec![0x03, 0x03];
        body.extend_from_slice(&[7; 32]);
        body.extend_from_slice(&[0, 0, 2, 0x13, 0x01, 1, 0]); // oturum yok, 1 şifre, sıkıştırma yok
        body.extend_from_slice(&[0, 5, 0xff, 0x01, 0, 1, 0]); // tek uzantı: renegotiation_info
        let mut record = vec![0x16, 0x03, 0x03, 0, (body.len() + 4) as u8, 0x01, 0, 0, body.len() as u8];
        record.extend_from_slice(&body);
        assert_eq!(inspect(&record), Hello::NoSni);
    }

    #[test]
    fn rejects_everything_else() {
        assert_eq!(inspect(b""), Hello::Other);
        assert_eq!(inspect(b"GET / HTTP/1.1\r\n"), Hello::Other);
        assert_eq!(inspect(&[0x17, 0x03, 0x03, 0, 5, 1, 2, 3, 4, 5]), Hello::Other, "uygulama verisi");
        assert_eq!(inspect(&[0x16, 0x03, 0x03, 0, 5, 0x02, 0, 0, 1, 0]), Hello::Other, "ServerHello");
        // Rastgele ve bozuk girdide panik yok.
        let mut rng = Rng::new(99);
        for length in 0..300 {
            let mut noise = vec![0x16, 0x03, 0x01, 0, 0, 0x01];
            rng.fill(&mut noise, length);
            let _ = inspect(&noise);
        }
        let mut hello = fake_hello("example.com", &mut rng);
        for i in 0..hello.len() {
            let saved = hello[i];
            hello[i] = 0xff;
            let _ = inspect(&hello);
            hello[i] = saved;
        }
    }

    #[test]
    fn reads_http_host() {
        assert_eq!(http_host(b"GET / HTTP/1.1\r\nHost: Example.com\r\nAccept: */*\r\n\r\n").as_deref(), Some("example.com"));
        assert_eq!(http_host(b"POST /x HTTP/1.1\r\nhost: a.example.org:8080\r\n\r\n").as_deref(), Some("a.example.org"));
        assert_eq!(http_host(b"GET / HTTP/1.1\r\nAccept: */*\r\n\r\n"), None);
        assert_eq!(http_host(b"HELLO / HTTP/1.1\r\nHost: example.com\r\n\r\n"), None);
        assert_eq!(http_host(b"GET / HTTP/1.1\r\nHost: exa mple\r\n\r\n"), None);
        assert_eq!(http_host(&fake_http("www.w3.org")).as_deref(), Some("www.w3.org"));
    }
}
