# Üçüncü taraf yazılımlar / Third-party software

Robz DPI'ın kendi kodu MIT lisanslıdır (bkz. [LICENSE](LICENSE)). Kurulum paketi aşağıdaki yazılımları **değiştirmeden**
içerir; her biri kendi lisansıyla dağıtılır.

Robz DPI's own code is MIT licensed (see [LICENSE](LICENSE)). The installer bundles the following software
**unmodified**; each is distributed under its own license.

| Yazılım / Software | Sürüm / Version | Lisans / License | Kaynak / Source |
|---|---|---|---|
| GoodbyeDPI (`goodbyedpi.exe`) — ValdikSS | 0.2.3rc3-2 | Apache-2.0 | https://github.com/ValdikSS/GoodbyeDPI |
| WinDivert (`WinDivert.dll`, `WinDivert64.sys`) — basil00 | 2.2.0-D | LGPL-3.0 ya da / or GPL-2.0 | https://github.com/basil00/WinDivert |
| uthash (GoodbyeDPI içinde / inside GoodbyeDPI) | 2.3.0 | BSD | https://troydhanson.github.io/uthash/ |
| getline (GoodbyeDPI içinde / inside GoodbyeDPI) | — | BSD-2-Clause | NetBSD |
| Preact + HTM (arayüz / interface) | — | MIT, Apache-2.0 | https://github.com/developit/htm |
| Mona Sans (yazı tipi / typeface) | — | SIL OFL 1.1 | https://github.com/github/mona-sans |

Lisans metinleri kurulumla birlikte `engine/licenses/` klasörüne gelir.
The license texts are installed into the `engine/licenses/` folder.

Motor dosyaları bu depoda tutulmaz. `npm run engine` onları ValdikSS'in resmi sürümünden indirir ve
[`engine.lock.json`](engine.lock.json) dosyasındaki SHA-256 özetleriyle doğrular; özet tutmazsa hiçbir dosya yazılmaz.

The engine files are not stored in this repository. `npm run engine` downloads them from ValdikSS's official
release and verifies them against the SHA-256 digests in [`engine.lock.json`](engine.lock.json); nothing is
written if a digest does not match.
