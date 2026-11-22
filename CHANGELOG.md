# Sürüm notları

Her başlık bir sürümdür; sürüm iş akışı GitHub'daki sürüm notunu buradan alır.

## 0.1.0

İlk sürüm. Deneyseldir: motor her servis sağlayıcıda denenmedi.

- **Tek düğme:** motoru açar ve kapatır. Windows ilk açışta ve ayar değişince yönetici izni ister; açıp kapamak
  izin istemez.
- **Yalnız seçtiklerinde:** Discord ve Roblox hazır gelir, başka siteler adresi yazılarak eklenir. "Bütün sitelerde"
  kipi her bağlantıya uygular.
- **Kendi motoru:** paketleri işleyen kod Rust ile yazıldı; yöntemler GoodbyeDPI'dan alındı. Dört hazır yöntem ve
  alan alan düzenlenebilen "Özel" yöntem.
- **DNS:** seçili sitelerin sorguları Quad9, Yandex, AdGuard, OpenDNS ya da yazdığın sunucuya yönlendirilir.
- **Bağlantı testi:** seçtiklerine ulaşılıp ulaşılamadığını gösterir.
- **Gelişmiş bölüm:** yöntemin ayrıntıları, motorun sayaçları ve günlüğü.
- **Her zaman açık:** Windows açılınca tepside başlar, en son açık bıraktıysan kendiliğinden açılır.
- **Çakışma uyarısı:** çalışan GoodbyeDPI'ı görür ve istersen durdurur.
- Türkçe ve İngilizce arayüz.

Bilinen eksikler:

- Robz DPI kaldırılırken motor kendiliğinden silinmez; önce Ayarlar → "Motoru kaldır" gerekir.
- Kurulum dosyası imzalı değildir; Windows SmartScreen uyarı gösterir.
- Yalnız Windows 10 ve 11 (64 bit).
