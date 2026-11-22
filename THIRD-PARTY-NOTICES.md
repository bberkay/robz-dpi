# Üçüncü taraf yazılımlar / Third-party software

Robz DPI'ın kendi kodu MIT lisanslıdır (bkz. [LICENSE](LICENSE)).
Robz DPI's own code is MIT licensed (see [LICENSE](LICENSE)).

## Pakete giren ikili dosyalar / Bundled binaries

Kurulum paketine dışarıdan giren tek ikili dosya WinDivert'tir; **değiştirilmeden** eklenir.
The only third-party binaries in the installer are WinDivert's; they are bundled **unmodified**.

| Yazılım / Software | Sürüm / Version | Lisans / License | Kaynak / Source |
|---|---|---|---|
| WinDivert (`WinDivert.dll`, `WinDivert64.sys`) — basil00 | 2.2.2-A | LGPL-3.0 ya da / or GPL-2.0 | https://github.com/basil00/WinDivert |

Bu dosyalar depoda tutulmaz. `npm run engine` onları WinDivert'in resmi sürümünden indirir ve
[`engine.lock.json`](engine.lock.json) dosyasındaki SHA-256 özetleriyle doğrular; özet tutmazsa hiçbir dosya yazılmaz.
Lisans metni kurulumla birlikte `engine/licenses/` klasörüne gelir.

These files are not stored in this repository. `npm run engine` downloads them from WinDivert's official release and
verifies them against the SHA-256 digests in [`engine.lock.json`](engine.lock.json); nothing is written if a digest
does not match. The license text is installed into the `engine/licenses/` folder.

## Yöntemlerin kaynağı / Where the techniques come from

Motor (`src-tauri/src/engine`) bu depoda Rust ile yazılmıştır; içinde başka bir projenin derlenmiş kodu yoktur.
Kullandığı yöntemler **GoodbyeDPI**'dan (ValdikSS ve katkıda bulunanlar, Apache-2.0) alınmış ve onun kaynak kodu
okunarak yeniden yazılmıştır: ClientHello'yu iki TCP parçasına bölmek, önüne sunucuya ulaşmayan sahte bir istek
göndermek (kısa ömür, bozuk sağlama toplamı, eski sıra numarası), otomatik ömür hesabı, DNS sorgularını
yönlendirmek ve QUIC'in ilk paketini düşürmek. GoodbyeDPI'ın lisans metni
[`licenses/GoodbyeDPI-LICENSE.txt`](licenses/GoodbyeDPI-LICENSE.txt) dosyasındadır.

The engine (`src-tauri/src/engine`) is written in Rust in this repository and contains no compiled code from any
other project. Its techniques come from **GoodbyeDPI** (ValdikSS and contributors, Apache-2.0) and were
re-implemented by reading its source: splitting the ClientHello into two TCP segments, sending a fake request that
never reaches the server first (short TTL, bad checksum, old sequence number), the auto-TTL calculation, redirecting
DNS queries and dropping the first QUIC packet. GoodbyeDPI's license text is in
[`licenses/GoodbyeDPI-LICENSE.txt`](licenses/GoodbyeDPI-LICENSE.txt).

- GoodbyeDPI: https://github.com/ValdikSS/GoodbyeDPI
- GoodbyeDPI-Turkey (Türkiye için bayrak birleşimleri / flag sets for Turkey): https://github.com/cagritaskn/GoodbyeDPI-Turkey

## Arayüz / Interface

| Yazılım / Software | Lisans / License | Kaynak / Source |
|---|---|---|
| Preact + HTM | MIT, Apache-2.0 | https://github.com/developit/htm |
| Mona Sans (yazı tipi / typeface) | SIL OFL 1.1 | https://github.com/github/mona-sans |
| Tauri | MIT, Apache-2.0 | https://github.com/tauri-apps/tauri |
