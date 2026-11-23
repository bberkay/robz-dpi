# Sürüm notları

Her başlık bir sürümdür; sürüm iş akışı GitHub'daki sürüm notunu buradan alır.

## 0.1.0

İlk sürüm. Motor her servis sağlayıcıda denenmemiştir.

- **Tek düğme:** motoru açar ve kapatır. Windows ilk açılışta ve ayar değişikliğinde yönetici onayı ister; açıp
  kapamak onay gerektirmez.
- **Yalnız seçili siteler:** Discord ve Roblox hazır gelir, başka siteler adresi yazılarak eklenir. "Bütün sitelerde"
  kipi her bağlantıya uygular.
- **Kendi motoru:** paketleri işleyen kod bu depodadır; yöntemler GoodbyeDPI'dan alınmıştır. Dört hazır yöntem ve
  alan alan düzenlenebilen "Özel" yöntem.
- **DNS yönlendirme:** seçili sitelerin sorguları Quad9, Yandex, AdGuard, OpenDNS ya da belirlediğiniz sunucuya
  yönlendirilir.
- **Bağlantı testi:** seçili sitelere ulaşılıp ulaşılamadığını gösterir.
- **Gelişmiş bölüm:** yöntemin ayrıntıları, motorun sayaçları ve günlüğü.
- **Arka planda çalışma:** Windows açılışında tepside başlar ve son durumunu hatırlar.
- **Çakışma uyarısı:** çalışan GoodbyeDPI'ı algılar ve onayınızla durdurur.
- **Tek adımda kaldırma:** Ayarlar → "Robz DPI'ı kaldır" uygulamayı, arka planda çalışan parçasını ve ayarları
  birlikte siler. Windows'un Uygulamalar listesinden kaldırmak da arka plandaki parçayı siler.
- **İlk açılış adımları:** dil, sorumluluk reddinin kabulü ve nerede çalışacağı; bir kez sorulur.
- Türkçe, İngilizce ve Rusça arayüz.

Bilinen sınırlar:

- Kurulum dosyası imzalı değildir; Windows SmartScreen uyarı gösterebilir.
- Bazı antivirüs yazılımları (Kaspersky başta olmak üzere) WinDivert sürücüsünü engelleyebilir; ayrıntılar README'de.
- Yalnız 64 bit Windows 10 ve 11.

Eğitim ve test amaçlıdır; kullanmadan önce [sorumluluk reddini](https://github.com/kutsalkiremitci/robz-dpi/blob/main/DISCLAIMER.md) okuyunuz.
