// README başlık görselini ve sosyal medya önizlemesini (Open Graph) üretir:  node scripts/render-banners.mjs
//   docs/banner-<dil>.png   1280×640 (2x çizilir: 2560×1280), README'nin en üstünde
//   docs/og-<dil>.png       1200×630, bağlantı paylaşılınca görünen önizleme
// Diller: tr, en, ru. Her görsel kendi dilindeki ekran görüntüsünü kullanır.
// Microsoft Edge'in başsız kipiyle çizer (Windows'ta zaten kurulu); ekran görüntüleri docs/screenshots'tan alınır
// (önce `node scripts/screenshots.mjs`).
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const EDGE = ['C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', 'C:/Program Files/Microsoft/Edge/Application/msedge.exe'].find(existsSync);
if (!EDGE) throw new Error('Microsoft Edge bulunamadı');

const file = (p) => pathToFileURL(join(root, p)).href;
const font = (subset) => file(`node_modules/@fontsource-variable/mona-sans/files/mona-sans-${subset}-standard-normal.woff2`);

const TEXT = {
  tr: {
    eyebrow: 'Windows için açık kaynak ağ aracı',
    title: 'Yalnız seçtiğiniz sitelerde çalışan DPI aracı',
    sub: 'Tek düğmeyle açılır. Trafiğinizi bir sunucudan geçirmez, hızınızı etkilemez.',
    chips: ['Tek düğme', 'Seçili siteler', 'Veri toplamaz'],
    shot: 'docs/screenshots/tr/02-on.png',
    size: 54,
  },
  en: {
    eyebrow: 'Open-source network tool for Windows',
    title: 'A DPI tool that works only on the sites you choose',
    sub: 'One button turns it on. It does not route your traffic through a server or affect your speed.',
    chips: ['One button', 'Selected sites', 'No data collected'],
    shot: 'docs/screenshots/en/02-on.png',
    size: 52,
  },
  ru: {
    eyebrow: 'Сетевой инструмент с открытым кодом для Windows',
    title: 'Инструмент DPI, который работает только на выбранных сайтах',
    sub: 'Включается одной кнопкой. Трафик не идёт через сторонний сервер, скорость не меняется.',
    chips: ['Одна кнопка', 'Выбранные сайты', 'Без сбора данных'],
    shot: 'docs/screenshots/ru/02-on.png',
    size: 36, // Mona Sans'ta Kiril harfleri yok; yedek yazı tipi daha geniştir
  },
};

function html(t, { width, height }) {
  const scale = width / 1280;
  return `<!doctype html><html><head><meta charset="utf-8"><style>
@font-face { font-family: 'Mona Sans'; font-weight: 200 900; font-stretch: 75% 125%; src: url('${font('latin')}') format('woff2'); unicode-range: U+0000-00FF, U+0131, U+2000-206F; }
@font-face { font-family: 'Mona Sans'; font-weight: 200 900; font-stretch: 75% 125%; src: url('${font('latin-ext')}') format('woff2'); unicode-range: U+0100-02BA; }
* { box-sizing: border-box; margin: 0; }
html, body { width: ${width}px; height: ${height}px; overflow: hidden; background: #0d1117; }
.stage { position: relative; width: 1280px; height: ${Math.round(height / scale)}px; transform: scale(${scale}); transform-origin: 0 0; overflow: hidden;
  font-family: 'Mona Sans', 'Segoe UI Variable Display', 'Segoe UI', sans-serif; color: #f0f6fc; -webkit-font-smoothing: antialiased; }
.text { position: absolute; left: 72px; top: 72px; width: 690px; }
.brand { display: flex; align-items: center; gap: 14px; font-size: 26px; font-weight: 700; font-stretch: 112%; }
.brand img { width: 48px; height: 48px; border-radius: 11px; }
.eyebrow { margin-top: 52px; color: #ab7df8; font-family: 'Cascadia Code', 'Cascadia Mono', Consolas, monospace; font-size: 17px; letter-spacing: 0.02em; }
h1 { margin-top: 16px; font-size: ${t.size}px; line-height: 1.06; font-weight: 820; font-stretch: 125%; letter-spacing: -0.025em; text-wrap: balance; }
p { margin-top: 22px; max-width: 620px; color: #9198a1; font-size: 21px; line-height: 1.45; }
.chips { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 34px; }
.chips span { display: inline-flex; align-items: center; gap: 10px; height: 44px; padding: 0 18px; border: 1px solid #3d444d; border-radius: 999px;
  background: #151b23; color: #f0f6fc; font-size: 18px; font-weight: 550; white-space: nowrap; }
.chips span::before { content: ''; width: 8px; height: 8px; border-radius: 50%; background: #ab7df8; }
.repo { position: absolute; left: 72px; bottom: 44px; color: #6e7681; font-family: 'Cascadia Code', Consolas, monospace; font-size: 16px; }
.window { position: absolute; left: 836px; top: 64px; width: 384px; overflow: hidden; border: 1px solid #3d444d; border-radius: 12px;
  background: #010409; box-shadow: 0 30px 80px rgba(1, 4, 9, 0.9); }
.bar { display: flex; align-items: center; gap: 9px; height: 34px; padding-left: 14px; border-bottom: 1px solid rgba(61,68,77,.7);
  background: #161b22; color: #9198a1; font-size: 13px; }
.bar img { width: 15px; height: 15px; border-radius: 3px; }
.window > img { display: block; width: 100%; }
.line { position: absolute; left: 0; right: 0; bottom: 0; height: 6px; background: #ab7df8; }
</style></head><body><div class="stage">
  <div class="text">
    <div class="brand"><img src="${file('ui/assets/icon.png')}" alt="">Robz DPI</div>
    <div class="eyebrow">${t.eyebrow}</div>
    <h1>${t.title}</h1>
    <p>${t.sub}</p>
    <div class="chips">${t.chips.map((chip) => `<span>${chip}</span>`).join('')}</div>
  </div>
  <div class="repo">github.com/kutsalkiremitci/robz-dpi</div>
  <div class="window"><div class="bar"><img src="${file('ui/assets/icon.png')}" alt="">Robz DPI</div><img src="${file(t.shot)}" alt=""></div>
  <div class="line"></div>
</div></body></html>`;
}

function render(name, t, size, deviceScale) {
  const work = mkdtempSync(join(tmpdir(), 'robzdpi-banner-'));
  const page = join(work, 'banner.html');
  writeFileSync(page, html(t, size));
  const output = join(root, 'docs', name);
  execFileSync(
    EDGE,
    [
      '--headless=new',
      '--disable-gpu',
      '--hide-scrollbars',
      '--no-first-run',
      `--user-data-dir=${join(work, 'profile')}`,
      `--force-device-scale-factor=${deviceScale}`,
      `--window-size=${size.width},${size.height}`,
      '--virtual-time-budget=3000',
      `--screenshot=${output}`,
      pathToFileURL(page).href,
    ],
    { stdio: 'ignore', timeout: 60000 },
  );
  rmSync(work, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  console.log(`docs/${name}`);
}

for (const lang of Object.keys(TEXT)) {
  render(`banner-${lang}.png`, TEXT[lang], { width: 1280, height: 640 }, 2);
  render(`og-${lang}.png`, TEXT[lang], { width: 1200, height: 630 }, 1);
}
