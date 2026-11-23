// Web sitesinin metinleri: Türkçe, İngilizce, Rusça. Üçü de aynı alanlara sahip olmalıdır (build.mjs denetler).
// Ekran görüntüleri dosya adıyla verilir; her dil kendi klasöründekini kullanır (docs/screenshots/<dil>/).
//
// "wire" bölümü 1. yöntemin gerçekte yaptığını gösterir (src-tauri/src/plan.rs, Strategy::default): 2 bayttan böl,
// parçaları ters sırayla gönder, önüne www.w3.org adlı ve 5 sıçramada ölen sahte istek koy. Yöntem değişirse
// burası da değişmeli.
const wire = (t) => ({
  target: 'discord.com:443',
  caption: t.caption,
  cols: t.cols,
  cases: [
    {
      title: t.off,
      state: 'off',
      rows: [{ tag: 'ClientHello', bytes: ['16 03 01 … ', 'discord.com', ' …'], sees: t.seesName('discord.com'), result: t.cut, kind: 'bad' }],
    },
    {
      title: t.on,
      state: 'on',
      rows: [
        { tag: t.fake, meta: t.ttl, bytes: ['16 03 01 … ', 'www.w3.org', ' …'], sees: t.seesName('www.w3.org'), result: t.dies, kind: 'muted' },
        { tag: t.part(2), bytes: ['01 02 00 … ', 'discord.com', ' …'], sees: t.noHeader, result: t.arrives, kind: 'ok' },
        { tag: t.part(1), meta: t.twoBytes, bytes: ['16 03', '', ''], sees: t.noName, result: t.joined, kind: 'ok' },
      ],
    },
  ],
});

export default {
  tr: {
    lang: 'tr',
    locale: 'tr_TR',
    title: 'Robz DPI: yalnız seçtiğin sitelerde çalışan, tek düğmeli DPI aracı',
    description:
      'Robz DPI; Discord, Roblox ve eklediğin siteler için tek düğmeyle çalışan açık kaynak DPI aracıdır. Motoru Rust ile yazıldı. Eğitim ve test amaçlıdır, ücretsizdir.',
    skip: 'İçeriğe geç',
    langLabel: 'Dil',
    nav: { how: 'Nasıl çalışır', usage: 'Kullanım', faq: 'Sık sorulanlar', github: 'GitHub' },
    pillTag: 'Önemli',
    pillText: 'Eğitim ve test amaçlıdır: sorumluluk reddini oku',
    heroTitle: 'Seçtiğini aç, gerisine dokunma.',
    heroLead:
      'Robz DPI, bağlantının ilk paketini servis sağlayıcının derin paket inceleme (DPI) cihazının tanıyamayacağı biçimde gönderir. Tek düğmeyle açılır; yalnız Discord, Roblox ve senin eklediğin sitelerde çalışır.',
    download: 'Windows için indir',
    star: 'Yıldız ver',
    starTitle: "GitHub'da yıldız ver",
    facts: ['Windows 10 ve 11', 'VPN değil, hız düşmez', 'Motoru Rust ile yazıldı', 'Ücretsiz ve açık kaynak'],
    shotAlt: 'Robz DPI açıkken ana ekran: düğme yeşil, Discord ve Roblox için çalışıyor.',
    shotBackAlt: 'Robz DPI kapalıyken ana ekran.',

    whyTitle: 'Neden Robz DPI?',
    whyLead:
      'GoodbyeDPI işini yapıyor; ama kullanmak için arşiv açmak, betik seçmek ve bayrak düzenlemek gerekiyor, üstelik her siteye uygulandığı için bazı siteler bozuluyor. Robz DPI aynı yöntemleri bir düğmenin arkasına koyar ve yalnız seçtiğin sitelerde çalıştırır.',
    diffFile: 'dpi-kurulumu.diff',
    diffLabel: 'Eskiden ve Robz DPI ile kurulum karşılaştırması',
    diff: {
      del: [
        'Zip indir, hangi .cmd dosyasını çalıştıracağını bul, yönetici olarak aç',
        'Bütün sitelere uygulanır: bazı siteler açılmaz, neden olduğunu bilemezsin',
        'Başka bir site eklemek için metin dosyası, başka bir yöntem için bayrak düzenle',
      ],
      add: [
        "Robz DPI'ı kur, düğmeye bas",
        'Yalnız Discord, Roblox ve eklediğin sitelere uygulanır; gerisine dokunmaz',
        'Site eklemek bir kutuya adres yazmak, yöntem değiştirmek bir tık',
      ],
    },

    howTitle: 'Nasıl çalışır',
    howLead:
      'Bir siteye bağlanırken tarayıcın önce sitenin adını taşıyan bir paket gönderir (TLS ClientHello). Sağlayıcının cihazı bu paketteki adı okur; site engelliyse bağlantıyı keser. Robz DPI yalnız bu ilk paketi değiştirir: sitenin sunucusu onu rahatça anlar, aradaki cihaz tanıyamaz.',
    wire: wire({
      caption: '1. yöntemle bağlanırken',
      cols: ['Giden paket', 'Aradaki cihaz ne görür', 'Ne olur'],
      off: 'Robz DPI kapalı',
      on: 'Robz DPI açık',
      fake: 'Sahte istek',
      ttl: 'ömrü 5 sıçrama',
      twoBytes: '2 bayt',
      part: (n) => `${n}. parça`,
      seesName: (name) => `Site adı: ${name}`,
      noHeader: 'Paket başı yok, tanımaz',
      noName: 'İçinde ad yok',
      cut: 'Bağlantı kesilir',
      dies: 'Yolda söner, sunucuya varmaz',
      arrives: 'Sunucuya varır',
      joined: 'Sunucu iki parçayı sıraya dizer, bağlantı kurulur',
    }),
    how: [
      ['Böler', 'İlk paket iki parçaya ayrılır ve ikinci parça önce gönderilir. Sunucu parçaları sıraya dizer; aradaki cihaz adı bütün olarak göremez.'],
      ['Önüne sahte istek koyar', 'Engelli olmayan bir sitenin adını taşıyan sahte bir paket yollanır. Bu paket ya yolda söner ya da sunucunun çöpe atacağı biçimde bozuktur; aradaki cihaz onu gerçek sanar.'],
      ["DNS'i düzeltir", 'Engelli sitenin adres sorgusu, sağlayıcının karışmadığı bir sunucuya ve bağlantı noktasına yönlendirilir; yanıt, uygulamaya sorduğu yerden gelmiş gibi geri verilir.'],
    ],
    howNote:
      'Bağlantının geri kalanına dokunulmaz: hız düşmez, trafik başka bir sunucudan geçmez. Bu yüzden VPN değildir; IP adresin gizlenmez.',

    usageTitle: 'Nasıl kullanılır',
    usageLead: 'Kurulumdan çalışır hâle üç adım.',
    steps: [
      {
        title: 'İndir ve kur',
        text: 'RobzDPI-Setup.exe dosyasını çalıştır; kurulum yönetici izni istemez. İlk açılışta bir kez üç kısa adım sorulur: dil, sorumluluk reddinin kabulü ve nerede çalışacağı.',
        note: 'Dosya imzalı olmadığı için Windows "kişisel bilgisayarınızı korudu" uyarısı gösterebilir: Ek bilgi › Yine de çalıştır.',
        shot: '06-terms.png',
        shotAlt: 'İlk açılış: sorumluluk reddinin özeti ve onay kutusu',
      },
      {
        title: 'Düğmeye bas',
        text: 'Windows "Bu uygulamanın değişiklik yapmasına izin veriyor musunuz?" diye sorar: Evet de. Düğme yeşile döner. Sonraki açıp kapamalar izin istemez.',
        note: "GoodbyeDPI ya da benzeri bir araç çalışıyorsa önce onu kapat. Robz DPI çalışan GoodbyeDPI'ı görür, uyarır ve istersen durdurur.",
        shot: '02-on.png',
        shotAlt: 'Ana ekran: düğme yeşil, Discord ve Roblox için çalışıyor',
      },
      {
        title: 'Açılmazsa yöntemi değiştir',
        text: "Sağlayıcılar farklı engelleme yöntemleri kullanır. Bağlantı testi \"Açılmıyor\" diyorsa Ayarlar'dan sıradaki yöntemi seç, \"Uygula\"ya bas ve yeniden dene.",
        note: 'Dört yöntem de olmadıysa DNS sunucusunu değiştir. Yine olmazsa sağlayıcını ve motor günlüğünü ekleyerek sorun bildir.',
        shot: '03-settings.png',
        shotAlt: 'Ayarlar: dört hazır yöntem ve DNS sunucusu',
      },
    ],

    detailsTitle: 'Ayrıntılar',
    details: [
      ['Yalnız seçtiklerinde', 'Discord ve Roblox hazır gelir. Diğer sitelere dokunmadığı için "şu site açılmaz oldu" sorunu yaşanmaz.'],
      ['Kendi sitelerin', 'Adresini kutuya yaz, ekle; alt adresler kendiliğinden kapsanır. İstersen "Bütün sitelerde" kipi her bağlantıya uygular.'],
      ['Dört hazır yöntem', 'GoodbyeDPI-Turkey betiklerindeki bayrak takımlarıyla aynı davranır. Meraklısı için alan alan düzenlenen "Özel" yöntem de var.'],
      ["DNS'i de düzeltir", 'Seçili sitelerin DNS sorgularını Quad9, Yandex, AdGuard, OpenDNS ya da yazdığın sunucuya yönlendirir.'],
      ['Motoru bu depoda', 'Paketleri işleyen kod Rust ile sıfırdan yazıldı ve ağa dokunmadan sınanır. Tek üçüncü taraf dosya, SHA-256 özeti sabitlenmiş WinDivert sürücüsüdür.'],
      ['Veri toplamaz', 'Hesap, reklam, kullanım istatistiği ya da güncelleme denetimi yok. Paketlerin içeriğini okumaz, saklamaz, bir yere göndermez.'],
    ],

    screensTitle: 'Ekranlar',
    screens: [
      ['05-welcome.png', 'İlk açılış: dil seçimi.'],
      ['01-off.png', 'Kapalıyken: nerede çalışacağı ve kendi sitelerin.'],
      ['03-settings.png', 'Ayarlar: yöntem, DNS, dil ve kaldırma.'],
      ['04-advanced.png', 'Gelişmiş bölüm: yöntemin ayrıntıları, motor etkinliği ve günlüğü.'],
    ],
    openImage: 'Görselin tamamını aç',

    faqTitle: 'Sık sorulanlar',
    faq: [
      ['Robz DPI ücretli mi?', 'Hayır. Ücretsiz ve açık kaynaktır (MIT). Ücretli sürüm, reklam ya da hesap yok.'],
      [
        'Bu bir VPN mi? İnternetim yavaşlar mı?',
        'VPN değildir. Trafiğin başka bir sunucudan geçmez, IP adresin değişmez, hızın düşmez. Robz DPI yalnız her bağlantının ilk paketine dokunur.',
      ],
      [
        "Discord'da ses, Roblox'ta oyun çalışır mı?",
        'Ses ve oyun trafiği ayrı bir yoldan (UDP) gider ve Robz DPI ona dokunmaz. Engel çoğunlukla siteye ve oturum sunucularına bağlanırken uygulandığı için, onlar açılınca sesin ve oyunun da çalışması beklenir; ama bu her bağlantıda denenmedi.',
      ],
      ['Hangi siteleri ekleyebilirim?', 'İstediğin siteyi. Robz DPI\'da bir engelli siteler listesi yoktur; Discord ve Roblox yalnız hazır kısayollardır.'],
      [
        'GoodbyeDPI ile birlikte çalışır mı?',
        'Hayır, ikisini aynı anda çalıştırma: iki araç aynı paketi iki kez değiştirir ve bağlantılar bozulur. Robz DPI çalışan GoodbyeDPI\'ı görür, uyarır ve istersen senin yerine durdurur.',
      ],
      [
        'Antivirüsüm uyarı verdi, neden?',
        'Robz DPI paketleri yakalamak için WinDivert sürücüsünü kullanır. Ağ trafiğini değiştirebilen her araç gibi WinDivert de bazı antivirüslerce "riskli araç" diye işaretlenir; GoodbyeDPI dahil onu kullanan bütün programlarda durum budur. Kurulumdaki WinDivert dosyaları resmi sürümüdür ve SHA-256 özetleriyle doğrulanır.',
      ],
      [
        "Robz DPI'ı nasıl kaldırırım?",
        'Ayarlar › "Robz DPI\'ı kaldır". Uygulamayı, arka planda çalışan parçasını (Windows hizmeti ve sürücü) ve ayarlarını tek adımda siler. Windows\'un Uygulamalar listesinden kaldırmak da aynı işi yapar.',
      ],
      [
        "macOS ya da Linux'ta çalışır mı?",
        'Şimdilik yalnız 64 bit Windows 10 ve 11. Paketleri işleyen mantık her platformda derlenir ve sınanır; eksik olan, o sistemlerde paketleri yakalayan katmandır. Sıradaki hedef macOS.',
      ],
      [
        'Yasal mı?',
        'Bu, bulunduğun yere ve ne için kullandığına bağlıdır; bu site hukuki danışmanlık vermez. Robz DPI eğitim ve test amaçlı bir ağ aracıdır: nasıl ve nerede kullanıldığı kullanıcının sorumluluğundadır. Emin değilsen kullanma ve bir hukukçuya danış.',
      ],
    ],

    legalTitle: 'Eğitim ve test amaçlıdır',
    legalLead:
      'Robz DPI, ağ protokollerinin ve DPI sistemlerinin nasıl çalıştığını öğrenmek, incelemek ve kendi ağında denemek için yazılmış açık kaynak bir projedir. İndirerek, kurarak ya da kullanarak sorumluluk reddini kabul etmiş olursun.',
    legal: [
      ['Garanti yoktur', 'Yazılım "olduğu gibi" sunulur. Motoru her servis sağlayıcıda denenmedi.'],
      ['Sorumluluk kullanıcıdadır', 'Kullanımın bütün sonuçları ve bulunduğun yerdeki yasalara, mahkeme ve idare kararlarına uymak sana aittir.'],
      ['Hizmet değildir', 'VPN, vekil sunucu ya da erişim hizmeti sunmaz; IP adresini gizlemez, anonimlik ya da güvenlik sağlamaz.'],
      ['Kimseyi yönlendirmez', 'Yazılımda "engelli siteler" listesi yoktur; hangi alan adlarında çalışacağını kullanıcı seçer.'],
    ],
    legalLink: 'Sorumluluk reddinin tamamını oku',
    legalAnchor: 'türkçe',

    finalTitle: "Robz DPI'ı kur, düğmeye bas.",
    finalText:
      'Sende çalışmadıysa haber ver: hangi yöntemin hangi bağlantıda çalıştığını öğrenmenin tek yolu bu. Kurulumda takılırsan Discord sunucusunda ücretsiz destek var.',
    discord: 'Discord',
    issues: 'Sorun bildir',
    footerLicense: 'MIT lisanslı açık kaynak.',
    footerDisclaimer:
      'Robz DPI bağımsız bir projedir. Discord ve Roblox kendi sahiplerinin ticari markalarıdır; projenin onlarla bir bağlantısı yoktur.',
    footerSibling: 'Diğer Robz uygulamaları: Robz Hub',
  },

  en: {
    lang: 'en',
    locale: 'en_US',
    title: 'Robz DPI: a one-button DPI tool that works only on the sites you pick',
    description:
      'Robz DPI is an open-source, one-button DPI tool for Discord, Roblox and the sites you add. Its engine is written in Rust. For education and testing, free.',
    skip: 'Skip to content',
    langLabel: 'Language',
    nav: { how: 'How it works', usage: 'How to use', faq: 'FAQ', github: 'GitHub' },
    pillTag: 'Notice',
    pillText: 'For education and testing: read the disclaimer',
    heroTitle: 'Open what you pick, leave the rest alone.',
    heroLead:
      "Robz DPI sends the first packet of a connection in a form your provider's deep packet inspection (DPI) box cannot recognize. One button turns it on, and it works only for Discord, Roblox and the sites you add.",
    download: 'Download for Windows',
    star: 'Star',
    starTitle: 'Star on GitHub',
    facts: ['Windows 10 and 11', 'Not a VPN, no slowdown', 'Engine written in Rust', 'Free and open source'],
    shotAlt: 'Robz DPI turned on: the button is green and it is working for Discord and Roblox.',
    shotBackAlt: 'Robz DPI turned off: the main screen.',

    whyTitle: 'Why Robz DPI?',
    whyLead:
      'GoodbyeDPI does its job, but using it means unpacking an archive, choosing a script and editing flags, and because it applies to every site, some sites break. Robz DPI puts the same methods behind one button and runs them only on the sites you pick.',
    diffFile: 'dpi-setup.diff',
    diffLabel: 'Setup before and with Robz DPI',
    diff: {
      del: [
        'Download a zip, work out which .cmd file to run, open it as administrator',
        'It applies to every site: some sites stop loading and you cannot tell why',
        'Edit a text file to add a site, edit flags to try another method',
      ],
      add: [
        'Install Robz DPI, press the button',
        'It applies only to Discord, Roblox and the sites you add; the rest is left alone',
        'Adding a site is typing an address into a box; changing the method is one click',
      ],
    },

    howTitle: 'How it works',
    howLead:
      "When you connect to a site, your browser first sends a packet that carries the site's name (the TLS ClientHello). The provider's device reads the name in that packet and cuts the connection if the site is blocked. Robz DPI changes only this first packet: the site's server understands it easily, the device in between does not.",
    wire: wire({
      caption: 'Connecting with Method 1',
      cols: ['Packet sent', 'What the device in between sees', 'What happens'],
      off: 'Robz DPI off',
      on: 'Robz DPI on',
      fake: 'Fake request',
      ttl: 'lives 5 hops',
      twoBytes: '2 bytes',
      part: (n) => `Part ${n}`,
      seesName: (name) => `Site name: ${name}`,
      noHeader: 'No packet header, not recognized',
      noName: 'No name in it',
      cut: 'Connection is cut',
      dies: 'Dies on the way, never reaches the server',
      arrives: 'Reaches the server',
      joined: 'The server puts the two parts in order and connects',
    }),
    how: [
      ['Splits it', 'The first packet is cut in two and the second part is sent first. The server puts the parts back in order; the device in between never sees the whole name.'],
      ['Puts a fake request in front', 'A fake packet carrying the name of a site that is not blocked is sent. It either dies on the way or is broken so that the server throws it away; the device in between takes it for the real one.'],
      ['Fixes DNS', "The address lookup for a blocked site is redirected to a server and port the provider does not interfere with; the answer is handed back to the app as if it came from where it asked."],
    ],
    howNote:
      'The rest of the connection is left alone: no slowdown, no traffic through another server. That is why it is not a VPN; your IP address is not hidden.',

    usageTitle: 'How to use',
    usageLead: 'Three steps from install to working.',
    steps: [
      {
        title: 'Download and install',
        text: 'Run RobzDPI-Setup.exe; installing needs no administrator rights. The first run asks three short steps, once: language, accepting the disclaimer and where it should work.',
        note: 'The file is not signed, so Windows may show "Windows protected your PC": More info › Run anyway.',
        shot: '06-terms.png',
        shotAlt: 'First run: the disclaimer summary and the acceptance checkbox',
      },
      {
        title: 'Press the button',
        text: 'Windows asks "Do you want to allow this app to make changes to your device?": say Yes. The button turns green. Turning it on and off later asks for nothing.',
        note: 'If GoodbyeDPI or a similar tool is running, close it first. Robz DPI notices a running GoodbyeDPI, warns you and can stop it for you.',
        shot: '02-on.png',
        shotAlt: 'Main screen: the button is green, working for Discord and Roblox',
      },
      {
        title: 'If it does not load, change the method',
        text: 'Providers block in different ways. If the connection test says "Not reachable", pick the next method in Settings, press "Apply" and try again.',
        note: 'If none of the four methods works, change the DNS server. If that fails too, report it with your provider and the engine log.',
        shot: '03-settings.png',
        shotAlt: 'Settings: four ready-made methods and the DNS server',
      },
    ],

    detailsTitle: 'Details',
    details: [
      ['Only what you pick', 'Discord and Roblox come ready. Other sites are left alone, so "that site stopped loading" does not happen.'],
      ['Your own sites', 'Type the address into the box and add it; subdomains are covered automatically. "All sites" mode applies it to every connection if you want.'],
      ['Four ready-made methods', 'They behave like the flag sets in the GoodbyeDPI-Turkey scripts. For the curious there is also a "Custom" method you edit field by field.'],
      ['Fixes DNS too', 'Redirects the DNS lookups of the selected sites to Quad9, Yandex, AdGuard, OpenDNS or a server you type.'],
      ['The engine is in this repository', 'The code that handles packets was written from scratch in Rust and is tested without touching the network. The only third-party file is the WinDivert driver, pinned by SHA-256.'],
      ['Collects no data', 'No account, ads, usage statistics or update checks. It does not read, store or send the contents of your packets.'],
    ],

    screensTitle: 'Screens',
    screens: [
      ['05-welcome.png', 'First run: choosing the language.'],
      ['01-off.png', 'Turned off: where it works and your own sites.'],
      ['03-settings.png', 'Settings: method, DNS, language and uninstall.'],
      ['04-advanced.png', 'Advanced section: method details, engine activity and log.'],
    ],
    openImage: 'Open the full image',

    faqTitle: 'FAQ',
    faq: [
      ['Is Robz DPI paid?', 'No. It is free and open source (MIT). There is no paid version, no ads and no account.'],
      [
        'Is this a VPN? Will my internet slow down?',
        'It is not a VPN. Your traffic does not go through another server, your IP address does not change and your speed does not drop. Robz DPI only touches the first packet of each connection.',
      ],
      [
        'Does voice on Discord and gameplay on Roblox work?',
        'Voice and game traffic travels a separate way (UDP) and Robz DPI does not touch it. Blocking is usually applied while connecting to the site and the session servers, so once those open, voice and gameplay are expected to work too; this has not been tried on every connection.',
      ],
      ['Which sites can I add?', 'Any site. Robz DPI has no list of blocked sites; Discord and Roblox are only ready-made shortcuts.'],
      [
        'Can it run together with GoodbyeDPI?',
        'No, do not run both at once: two tools change the same packet twice and connections break. Robz DPI notices a running GoodbyeDPI, warns you and can stop it for you.',
      ],
      [
        'My antivirus raised a warning. Why?',
        'Robz DPI captures packets with the WinDivert driver. Like any tool that can change network traffic, WinDivert is flagged as a "risky tool" by some antivirus programs; that is true for every program that uses it, GoodbyeDPI included. The WinDivert files in the installer are the official release and are verified by SHA-256.',
      ],
      [
        'How do I uninstall Robz DPI?',
        'Settings › "Uninstall Robz DPI". It removes the app, its background part (the Windows service and the driver) and your settings in one step. Uninstalling from the Windows Apps list does the same.',
      ],
      [
        'Does it work on macOS or Linux?',
        'Only 64-bit Windows 10 and 11 for now. The packet logic builds and is tested on every platform; what is missing is the layer that captures packets on those systems. macOS is next.',
      ],
      [
        'Is it legal?',
        'That depends on where you are and what you use it for; this site does not give legal advice. Robz DPI is a network tool for education and testing: how and where it is used is the responsibility of the user. If you are not sure, do not use it and ask a lawyer.',
      ],
    ],

    legalTitle: 'For education and testing',
    legalLead:
      'Robz DPI is an open-source project written to learn and study how network protocols and DPI systems work, and to try this on your own network. By downloading, installing or using it you accept the disclaimer.',
    legal: [
      ['No warranty', 'The software is provided "as is". Its engine has not been tried on every internet provider.'],
      ['The user is responsible', 'Every consequence of using it, and following the laws and the court and authority decisions where you are, is yours.'],
      ['Not a service', 'It is not a VPN, a proxy or an access service; it does not hide your IP address and provides no anonymity or security.'],
      ['Directs no one', 'The software has no list of "blocked sites"; the user chooses which domains it works on.'],
    ],
    legalLink: 'Read the full disclaimer',
    legalAnchor: 'english',

    finalTitle: 'Install Robz DPI, press the button.',
    finalText:
      'If it does not work for you, tell us: that is the only way to learn which method works on which connection. If you get stuck installing, there is free support on the Discord server.',
    discord: 'Discord',
    issues: 'Report an issue',
    footerLicense: 'Open source under the MIT license.',
    footerDisclaimer:
      'Robz DPI is an independent project. Discord and Roblox are trademarks of their owners; the project is not affiliated with them.',
    footerSibling: 'Other Robz apps: Robz Hub',
  },

  ru: {
    lang: 'ru',
    locale: 'ru_RU',
    title: 'Robz DPI: инструмент DPI с одной кнопкой, который работает только на выбранных сайтах',
    description:
      'Robz DPI — инструмент DPI с открытым кодом для Discord, Roblox и добавленных вами сайтов. Включается одной кнопкой, движок написан на Rust. Для обучения и тестирования, бесплатно.',
    skip: 'Перейти к содержимому',
    langLabel: 'Язык',
    nav: { how: 'Как это работает', usage: 'Как пользоваться', faq: 'Вопросы', github: 'GitHub' },
    pillTag: 'Важно',
    pillText: 'Для обучения и тестирования: прочитайте отказ от ответственности',
    heroTitle: 'Открывай выбранное, остальное не трогай.',
    heroLead:
      'Robz DPI отправляет первый пакет соединения в таком виде, что устройство глубокой инспекции пакетов (DPI) у провайдера его не распознаёт. Включается одной кнопкой и работает только для Discord, Roblox и добавленных вами сайтов.',
    download: 'Скачать для Windows',
    star: 'Звезда',
    starTitle: 'Поставить звезду на GitHub',
    facts: ['Windows 10 и 11', 'Не VPN, скорость не падает', 'Движок написан на Rust', 'Бесплатно, открытый код'],
    shotAlt: 'Robz DPI включён: кнопка зелёная, работает для Discord и Roblox.',
    shotBackAlt: 'Robz DPI выключен: главный экран.',

    whyTitle: 'Зачем нужен Robz DPI?',
    whyLead:
      'GoodbyeDPI делает своё дело, но чтобы им пользоваться, нужно распаковать архив, выбрать скрипт и править флаги; к тому же он применяется ко всем сайтам, и часть из них ломается. Robz DPI прячет те же методы за одной кнопкой и применяет их только к выбранным сайтам.',
    diffFile: 'dpi-setup.diff',
    diffLabel: 'Настройка раньше и с Robz DPI',
    diff: {
      del: [
        'Скачать архив, понять, какой .cmd запускать, открыть от имени администратора',
        'Применяется ко всем сайтам: часть сайтов перестаёт открываться, и непонятно почему',
        'Чтобы добавить сайт, править текстовый файл; чтобы сменить метод, править флаги',
      ],
      add: [
        'Установить Robz DPI и нажать кнопку',
        'Применяется только к Discord, Roblox и добавленным вами сайтам; остальное не затрагивается',
        'Добавить сайт — ввести адрес в поле; сменить метод — один щелчок',
      ],
    },

    howTitle: 'Как это работает',
    howLead:
      'При подключении к сайту браузер сначала отправляет пакет с именем сайта (TLS ClientHello). Устройство провайдера читает имя в этом пакете и обрывает соединение, если сайт заблокирован. Robz DPI меняет только этот первый пакет: сервер сайта понимает его без труда, а промежуточное устройство не распознаёт.',
    wire: wire({
      caption: 'Подключение с методом 1',
      cols: ['Отправленный пакет', 'Что видит промежуточное устройство', 'Что происходит'],
      off: 'Robz DPI выключен',
      on: 'Robz DPI включён',
      fake: 'Ложный запрос',
      ttl: 'живёт 5 переходов',
      twoBytes: '2 байта',
      part: (n) => `Часть ${n}`,
      seesName: (name) => `Имя сайта: ${name}`,
      noHeader: 'Нет заголовка пакета, не распознаёт',
      noName: 'Имени внутри нет',
      cut: 'Соединение обрывается',
      dies: 'Исчезает по пути, до сервера не доходит',
      arrives: 'Доходит до сервера',
      joined: 'Сервер расставляет две части по порядку, соединение установлено',
    }),
    how: [
      ['Делит', 'Первый пакет разрезается на две части, и вторая отправляется первой. Сервер расставляет части по порядку; промежуточное устройство не видит имя целиком.'],
      ['Ставит перед ним ложный запрос', 'Отправляется ложный пакет с именем незаблокированного сайта. Он либо исчезает по пути, либо испорчен так, что сервер его отбрасывает; промежуточное устройство принимает его за настоящий.'],
      ['Исправляет DNS', 'Запрос адреса заблокированного сайта перенаправляется на сервер и порт, в которые провайдер не вмешивается; ответ возвращается приложению так, будто пришёл оттуда, куда оно обращалось.'],
    ],
    howNote:
      'Остальная часть соединения не затрагивается: скорость не падает, трафик не идёт через другой сервер. Поэтому это не VPN; IP-адрес не скрывается.',

    usageTitle: 'Как пользоваться',
    usageLead: 'Три шага от установки до работы.',
    steps: [
      {
        title: 'Скачайте и установите',
        text: 'Запустите RobzDPI-Setup.exe; установка не требует прав администратора. При первом запуске один раз задаются три коротких шага: язык, согласие с отказом от ответственности и где программа должна работать.',
        note: 'Файл не подписан, поэтому Windows может показать «Система Windows защитила ваш компьютер»: Подробнее › Выполнить в любом случае.',
        shot: '06-terms.png',
        shotAlt: 'Первый запуск: краткий отказ от ответственности и флажок согласия',
      },
      {
        title: 'Нажмите кнопку',
        text: 'Windows спросит «Разрешить этому приложению вносить изменения на вашем устройстве?» — ответьте «Да». Кнопка станет зелёной. Дальнейшие включения и выключения разрешения не требуют.',
        note: 'Если работает GoodbyeDPI или похожая программа, сначала закройте её. Robz DPI замечает работающий GoodbyeDPI, предупреждает и может остановить его за вас.',
        shot: '02-on.png',
        shotAlt: 'Главный экран: кнопка зелёная, работает для Discord и Roblox',
      },
      {
        title: 'Если не открывается, смените метод',
        text: 'Провайдеры блокируют по-разному. Если проверка соединения пишет «Не открывается», выберите в настройках следующий метод, нажмите «Применить» и попробуйте снова.',
        note: 'Если не помог ни один из четырёх методов, смените DNS-сервер. Если и это не помогло, сообщите о проблеме, указав провайдера и приложив журнал движка.',
        shot: '03-settings.png',
        shotAlt: 'Настройки: четыре готовых метода и DNS-сервер',
      },
    ],

    detailsTitle: 'Подробности',
    details: [
      ['Только выбранное', 'Discord и Roblox уже готовы. Остальные сайты не затрагиваются, поэтому «этот сайт перестал открываться» не случается.'],
      ['Свои сайты', 'Введите адрес в поле и добавьте; поддомены включаются автоматически. Режим «Все сайты» при желании применяется к каждому соединению.'],
      ['Четыре готовых метода', 'Ведут себя как наборы флагов из скриптов GoodbyeDPI-Turkey. Для любопытных есть метод «Свой», который настраивается по полям.'],
      ['Исправляет и DNS', 'Перенаправляет DNS-запросы выбранных сайтов на Quad9, Yandex, AdGuard, OpenDNS или указанный вами сервер.'],
      ['Движок в этом репозитории', 'Код, обрабатывающий пакеты, написан с нуля на Rust и тестируется без сети. Единственный сторонний файл — драйвер WinDivert с закреплённой суммой SHA-256.'],
      ['Данные не собираются', 'Нет учётной записи, рекламы, статистики и проверки обновлений. Содержимое пакетов не читается, не сохраняется и никуда не отправляется.'],
    ],

    screensTitle: 'Экраны',
    screens: [
      ['05-welcome.png', 'Первый запуск: выбор языка.'],
      ['01-off.png', 'Выключено: где работает и свои сайты.'],
      ['03-settings.png', 'Настройки: метод, DNS, язык и удаление.'],
      ['04-advanced.png', 'Раздел «Дополнительно»: подробности метода, работа движка и журнал.'],
    ],
    openImage: 'Открыть изображение целиком',

    faqTitle: 'Частые вопросы',
    faq: [
      ['Robz DPI платный?', 'Нет. Он бесплатный и с открытым исходным кодом (MIT). Нет платной версии, рекламы и учётной записи.'],
      [
        'Это VPN? Интернет станет медленнее?',
        'Это не VPN. Трафик не идёт через другой сервер, IP-адрес не меняется, скорость не падает. Robz DPI затрагивает только первый пакет каждого соединения.',
      ],
      [
        'Работают ли голос в Discord и игра в Roblox?',
        'Голосовой и игровой трафик идёт отдельным путём (UDP), и Robz DPI его не трогает. Блокировка обычно применяется при подключении к сайту и серверам входа, поэтому ожидается, что после их открытия заработают и голос, и игра; но это проверялось не на всех соединениях.',
      ],
      ['Какие сайты можно добавить?', 'Любые. В Robz DPI нет списка заблокированных сайтов; Discord и Roblox — лишь готовые ярлыки.'],
      [
        'Можно ли запускать вместе с GoodbyeDPI?',
        'Нет, не запускайте обе программы одновременно: они меняют один и тот же пакет дважды, и соединения ломаются. Robz DPI замечает работающий GoodbyeDPI, предупреждает и может остановить его за вас.',
      ],
      [
        'Антивирус выдал предупреждение. Почему?',
        'Robz DPI перехватывает пакеты с помощью драйвера WinDivert. Как и любой инструмент, способный менять сетевой трафик, WinDivert помечается некоторыми антивирусами как «рискованная программа»; так происходит со всеми программами, которые его используют, включая GoodbyeDPI. Файлы WinDivert в установщике — официальный выпуск, они проверяются по SHA-256.',
      ],
      [
        'Как удалить Robz DPI?',
        'Настройки › «Удалить Robz DPI». Одним действием удаляются приложение, его фоновая часть (служба Windows и драйвер) и настройки. Удаление через список «Приложения» Windows делает то же самое.',
      ],
      [
        'Работает ли в macOS или Linux?',
        'Пока только 64-битные Windows 10 и 11. Логика работы с пакетами собирается и тестируется на любой платформе; не хватает слоя, который перехватывает пакеты в этих системах. Следующая цель — macOS.',
      ],
      [
        'Это законно?',
        'Это зависит от того, где вы находитесь и для чего используете программу; этот сайт не даёт юридических консультаций. Robz DPI — сетевой инструмент для обучения и тестирования: за то, как и где он используется, отвечает пользователь. Если сомневаетесь, не используйте его и обратитесь к юристу.',
      ],
    ],

    legalTitle: 'Для обучения и тестирования',
    legalLead:
      'Robz DPI — проект с открытым исходным кодом, созданный для изучения работы сетевых протоколов и систем DPI и для проверки этого в собственной сети. Скачивая, устанавливая или используя его, вы принимаете отказ от ответственности.',
    legal: [
      ['Гарантий нет', 'Программа предоставляется «как есть». Движок не проверялся у всех провайдеров.'],
      ['Отвечает пользователь', 'Все последствия использования и соблюдение законов, решений судов и органов власти там, где вы находитесь, лежат на вас.'],
      ['Это не услуга', 'Не VPN, не прокси и не сервис доступа; IP-адрес не скрывается, анонимность и безопасность не обеспечиваются.'],
      ['Никого не побуждает', 'В программе нет списка «заблокированных сайтов»; на каких доменах она работает, выбирает пользователь.'],
    ],
    legalLink: 'Прочитать отказ от ответственности полностью',
    legalAnchor: 'русский',

    finalTitle: 'Установите Robz DPI и нажмите кнопку.',
    finalText:
      'Если у вас не работает, сообщите: только так можно узнать, какой метод работает на каком соединении. Если не получается установить, на сервере Discord есть бесплатная поддержка.',
    discord: 'Discord',
    issues: 'Сообщить о проблеме',
    footerLicense: 'Открытый код под лицензией MIT.',
    footerDisclaimer:
      'Robz DPI — независимый проект. Discord и Roblox — товарные знаки их владельцев; проект с ними не связан.',
    footerSibling: 'Другие приложения Robz: Robz Hub',
  },
};
