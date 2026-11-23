// Web sitesini üretir:  node site/build.mjs   ->  site/dist/ (Türkçe), site/dist/en/ (İngilizce), site/dist/ru/ (Rusça)
//
// Tek sayfalık bir sitedir; her dilin kendi sayfası vardır ve o dilin ekran görüntülerini kullanır
// (../docs/screenshots/<dil>/, `node scripts/screenshots.mjs` üretir). Metinler content.mjs'tedir.
// Sürüm numarası, kurulum dosyasının boyutu ve yıldız sayısı derleme sırasında GitHub'dan okunur (ağ yoksa ya da
// depo gizliyse boş kalır) ve sayfada site.js ile güncellenir. GitHub Pages iş akışı (pages.yml) bunu çalıştırır.
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import content from './content.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const out = join(here, 'dist');

const REPO = 'kutsalkiremitci/robz-dpi';
const SITE = 'https://kutsalkiremitci.github.io/robz-dpi/';
const REPO_URL = `https://github.com/${REPO}`;
const SETUP = 'RobzDPI-Setup.exe';
const DOWNLOAD = `${REPO_URL}/releases/latest/download/${SETUP}`;
// Linux paketleri: sürüm iş akışı her sürüme bu sabit adlarla da yükler.
const LINUX = [
  ['.deb', `${REPO_URL}/releases/latest/download/RobzDPI-Linux-amd64.deb`, 'Debian, Ubuntu, Mint'],
  ['.rpm', `${REPO_URL}/releases/latest/download/RobzDPI-Linux-x86_64.rpm`, 'Fedora, openSUSE'],
  ['AppImage', `${REPO_URL}/releases/latest/download/RobzDPI-Linux-x86_64.AppImage`, 'Arch, …'],
];
const DISCORD = 'https://discord.gg/UyyEn7SHm4';
const ROBZ_HUB = 'https://kutsalkiremitci.github.io/robz-hub/';

const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

// Diller: sıra, dil menüsündeki sıradır. İlki sitenin kökünde, diğerleri kendi klasöründe yayınlanır.
const LANGS = [
  { code: 'tr', name: 'Türkçe', dir: '', number: 'tr-TR' },
  { code: 'en', name: 'English', dir: 'en/', number: 'en-US' },
  { code: 'ru', name: 'Русский', dir: 'ru/', number: 'ru-RU' },
];

const esc = (value) =>
  String(value ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

// ---- içerik denetimi: üç dil aynı alanlara sahip mi? ------------------------------------------------------------------

function shape(value) {
  if (typeof value === 'function') return 'fn';
  if (Array.isArray(value)) return `[${value.map(shape).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.keys(value).sort().map((key) => `${key}:${shape(value[key])}`).join(',')}}`;
  return typeof value;
}
for (const { code } of LANGS) {
  if (!content[code]) throw new Error(`site/content.mjs: "${code}" dili yok`);
  // Metinlerin kendisi değil, biçimi karşılaştırılır: eksik alan, eksik adım ya da soru burada yakalanır.
  if (shape(content[code]) !== shape(content.tr)) throw new Error(`site/content.mjs: "${code}" ile "tr" aynı alanlara sahip değil`);
}

// ---- simgeler (16'lık ızgara) ---------------------------------------------------------------------------------------

const ICONS = {
  download: '<path d="M8 2.25v8M4.75 7 8 10.25 11.25 7M2.75 13.25h10.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>',
  star: '<path d="M8 1.6l1.95 3.95 4.35.63-3.15 3.07.74 4.33L8 11.53l-3.89 2.05.74-4.33L1.7 6.18l4.35-.63z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>',
  github:
    '<path fill="currentColor" d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"/>',
  tag: '<path d="M2.5 2.5h5.1l5.9 5.9-5.1 5.1-5.9-5.9zM5.4 5.4h.01" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>',
  info: '<path d="M8 2.25a5.75 5.75 0 1 0 0 11.5 5.75 5.75 0 0 0 0-11.5zM8 7.4v3.6M8 5.1h.01" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>',
  alert: '<path d="M8 2.2 14.2 13H1.8zM8 6.4v3.2M8 11.4h.01" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>',
  check: '<path d="M3.25 8.4 6.5 11.5l6.25-7" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>',
  x: '<path d="m4 4 8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
  fade: '<path d="M2.5 8h7M11.5 8h.01M13.5 8h.01" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
  arrow: '<path d="M3 8h9.5M8.75 4.25 12.5 8l-3.75 3.75" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>',
  expand: '<path d="M9.5 2.5h4v4M13.5 2.5 9 7M6.5 13.5h-4v-4M2.5 13.5 7 9" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>',
};
// Marka logosu (Simple Icons, CC0): 24'lük ızgara.
const BRANDS = {
  discord:
    'M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z',
};
const icon = (name, size = 16) =>
  BRANDS[name]
    ? `<svg class="i i-${name}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="${BRANDS[name]}"/></svg>`
    : `<svg class="i" width="${size}" height="${size}" viewBox="0 0 16 16" aria-hidden="true">${ICONS[name]}</svg>`;

// Bayraklar SVG: Windows bayrak emojilerini "TR", "GB" harfleri olarak gösterir.
const STAR = Array.from({ length: 10 }, (_, k) => {
  const angle = Math.PI + (k * Math.PI) / 5;
  const radius = k % 2 ? 0.96 : 2.5;
  return `${(16.1 + radius * Math.cos(angle)).toFixed(2)},${(10 + radius * Math.sin(angle)).toFixed(2)}`;
}).join(' ');
const FLAGS = {
  tr: `<svg class="flag" width="21" height="14" viewBox="0 0 30 20" aria-hidden="true"><rect width="30" height="20" fill="#E30A17"/><circle cx="10.5" cy="10" r="5" fill="#fff"/><circle cx="11.75" cy="10" r="4" fill="#E30A17"/><polygon points="${STAR}" fill="#fff"/></svg>`,
  en: '<svg class="flag" width="21" height="14" viewBox="0 0 45 30" aria-hidden="true"><rect width="45" height="30" fill="#012169"/><path d="M0,0 L45,30 M45,0 L0,30" stroke="#fff" stroke-width="6"/><path d="M0,0 L45,30 M45,0 L0,30" stroke="#C8102E" stroke-width="2.2"/><path d="M22.5,0 V30 M0,15 H45" stroke="#fff" stroke-width="9"/><path d="M22.5,0 V30 M0,15 H45" stroke="#C8102E" stroke-width="5.4"/></svg>',
  ru: '<svg class="flag" width="21" height="14" viewBox="0 0 30 20" aria-hidden="true"><rect width="30" height="20" fill="#fff"/><rect y="6.667" width="30" height="13.333" fill="#0039A6"/><rect y="13.333" width="30" height="6.667" fill="#D52B1E"/></svg>',
};

// ---- GitHub'dan derleme anındaki bilgiler (ağ yoksa ya da depo gizliyse boş) -------------------------------------------

async function github(path) {
  try {
    const headers = { Accept: 'application/vnd.github+json', 'User-Agent': 'robz-dpi-site' };
    if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
    const res = await fetch(`https://api.github.com/${path}`, { headers, signal: AbortSignal.timeout(8000) });
    return res.ok ? await res.json() : null;
  } catch {
    return null;
  }
}

const live = { stars: null, version: pkg.version, size: null };
{
  const [repo, release] = await Promise.all([github(`repos/${REPO}`), github(`repos/${REPO}/releases/latest`)]);
  if (repo) live.stars = repo.stargazers_count;
  if (release) {
    live.version = release.tag_name.replace(/^v/, '');
    live.size = release.assets.find((asset) => asset.name === SETUP)?.size ?? null;
  }
}
const sizeText = (bytes, locale) => `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / 1048576)} MB`;

// ---- dosyalar -------------------------------------------------------------------------------------------------------

const SHOT_WIDTH = 1040; // docs/screenshots: 520 × 800 pencere, 2x çizilir
// "Ekranlar"da uzun görüntüler pencere boyunda kesilir. Gelişmiş bölümün görüntüsü ayarlarla aynı başladığı için
// üstünden değil, gelişmiş ayarların başladığı yerden gösterilir.
const SCREEN_FOCUS = { '04-advanced.png': 40 };
const shotSize = {}; // 'tr/03-settings.png' -> yükseklik (uzun sayfalar pencereden uzundur)

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'assets', 'img'), { recursive: true });
mkdirSync(join(out, 'assets', 'fonts'), { recursive: true });
cpSync(join(here, 'assets'), join(out, 'assets'), { recursive: true });
cpSync(join(root, 'assets', 'logo.svg'), join(out, 'assets', 'logo.svg'));
cpSync(join(root, 'src-tauri', 'icons', '32x32.png'), join(out, 'assets', 'favicon-32.png'));
cpSync(join(root, 'src-tauri', 'icons', '128x128@2x.png'), join(out, 'assets', 'apple-touch-icon.png'));
for (const { code, dir } of LANGS) {
  const shots = join(root, 'docs', 'screenshots', code);
  if (!existsSync(shots)) throw new Error(`docs/screenshots/${code} yok: önce "node scripts/screenshots.mjs"`);
  cpSync(shots, join(out, 'assets', 'img', code), { recursive: true });
  const og = join(root, 'docs', `og-${code}.png`);
  if (existsSync(og)) cpSync(og, join(out, 'assets', 'img', `og-${code}.png`));
  if (dir) mkdirSync(join(out, dir), { recursive: true });
}
const fontDir = join(root, 'node_modules', '@fontsource-variable', 'mona-sans', 'files');
for (const subset of ['latin', 'latin-ext']) {
  cpSync(join(fontDir, `mona-sans-${subset}-standard-normal.woff2`), join(out, 'assets', 'fonts', `mona-sans-${subset}.woff2`));
}

// PNG'nin boyu başlığının 20. baytındadır; sayfa kaymasın diye <img> boyutları buradan yazılır.
function shotHeight(lang, file) {
  const key = `${lang}/${file}`;
  if (!shotSize[key]) {
    const path = join(root, 'docs', 'screenshots', lang, file);
    if (!existsSync(path)) throw new Error(`docs/screenshots/${key} yok`);
    shotSize[key] = readFileSync(path).readUInt32BE(20);
  }
  return shotSize[key];
}

// ---- sayfa ----------------------------------------------------------------------------------------------------------

function page(t) {
  const lang = t.lang;
  const self = LANGS.find((entry) => entry.code === lang);
  const base = self.dir ? '../' : '';
  const url = `${SITE}${self.dir}`;
  const home = base || './';
  const img = (file) => `${base}assets/img/${lang}/${file}`;
  // Bu sayfadan başka bir dilin sayfasına göreli yol
  const langHref = (entry) => (entry.code === lang ? './' : `${base}${entry.dir}` || './');

  // Uygulama penceresi: Windows başlık çubuğuyla. "crop" uzun görüntüyü pencere boyunda keser.
  const windowFrame = (file, alt, { eager = false, cls = '', crop = false } = {}) => {
    const height = shotHeight(lang, file);
    return `<figure class="window ${cls}${crop ? ' window-crop' : ''}">
      <div class="window-bar" aria-hidden="true">
        <img src="${base}assets/logo.svg" width="14" height="14" alt="">
        <span>Robz DPI</span>
        <span class="window-controls"><i></i><i></i><i></i></span>
      </div>
      <img src="${img(file)}" width="${SHOT_WIDTH}" height="${height}" alt="${esc(alt)}" ${eager ? 'fetchpriority="high"' : 'loading="lazy"'}>
    </figure>`;
  };

  const downloadButtons = ({ big = false } = {}) => `<div class="cta">
      <a class="btn btn-primary ${big ? 'btn-xl' : 'btn-lg'}" href="${DOWNLOAD}" data-download>
        ${icon('download', 18)}<span>${esc(t.download)}</span><span class="btn-meta" data-size ${live.size == null ? 'hidden' : ''}>${live.size == null ? '' : sizeText(live.size, self.number)}</span>
      </a>
      <a class="btn btn-star ${big ? 'btn-xl' : 'btn-lg'}" href="${REPO_URL}" title="${esc(t.starTitle)}">
        ${icon('star', 18)}<span>${esc(t.star)}</span><span class="btn-count" data-stars ${live.stars == null ? 'hidden' : ''}>${live.stars ?? ''}</span>
      </a>
    </div>
    <p class="cta-linux">${esc(t.linuxLabel)} ${LINUX.map(([label, href, distros]) => `<a href="${href}" title="${esc(distros)}">${label}</a> <span>(${esc(distros)})</span>`).join(' · ')}</p>`;

  const diff = () => {
    const n = t.diff.del.length;
    const rows = [
      ...t.diff.del.map((text, i) => ({ kind: 'del', old: i + 1, now: '', text })),
      ...t.diff.add.map((text, i) => ({ kind: 'add', old: '', now: i + 1, text })),
    ];
    return `<figure class="diff" aria-label="${esc(t.diffLabel)}">
      <figcaption class="diff-head">
        <span class="diff-file">${esc(t.diffFile)}</span>
        <span class="diff-stat"><span class="diff-stat-add">+${t.diff.add.length}</span> <span class="diff-stat-del">−${n}</span>
          <span class="diff-blocks" aria-hidden="true">${'<i class="a"></i>'.repeat(t.diff.add.length)}${'<i class="d"></i>'.repeat(n)}</span></span>
      </figcaption>
      <div class="diff-hunk" aria-hidden="true">@@ -1,${n} +1,${t.diff.add.length} @@</div>
      <ol class="diff-lines">
        ${rows
          .map(
            (row, i) => `<li class="diff-${row.kind}" style="--i:${i}">
          <span class="diff-num" aria-hidden="true">${row.old}</span><span class="diff-num" aria-hidden="true">${row.now}</span>
          <span class="diff-sign" aria-hidden="true">${row.kind === 'del' ? '-' : '+'}</span>
          <span class="diff-text"><span class="sr-only">${row.kind === 'del' ? '− ' : '+ '}</span>${esc(row.text)}</span>
        </li>`,
          )
          .join('')}
      </ol>
    </figure>`;
  };

  // Paketlerin yolu: kapalıyken tek paket, açıkken sahte istek ve ters sırayla iki parça.
  const wire = () => {
    const [colPacket, colSees, colResult] = t.wire.cols;
    const mark = { bad: 'x', ok: 'check', muted: 'fade' };
    return `<figure class="wire">
      <figcaption class="wire-head"><span class="wire-target">${esc(t.wire.target)}</span><span>${esc(t.wire.caption)}</span></figcaption>
      <div class="wire-cols" aria-hidden="true"><span>${esc(colPacket)}</span><span>${esc(colSees)}</span><span>${esc(colResult)}</span></div>
      ${t.wire.cases
        .map(
          (group) => `<section class="wire-case wire-${group.state}">
        <h3 class="wire-title"><i aria-hidden="true"></i>${esc(group.title)}</h3>
        <ol>
          ${group.rows
            .map(
              (row) => `<li class="wire-row wire-${row.kind}">
            <div class="wire-packet">
              <span class="sr-only">${esc(colPacket)}: </span>
              <span class="wire-tag">${esc(row.tag)}</span>
              <span class="wire-body"><code>${esc(row.bytes[0])}${row.bytes[1] ? `<b>${esc(row.bytes[1])}</b>` : ''}${esc(row.bytes[2])}</code>${row.meta ? `<span class="wire-meta">${esc(row.meta)}</span>` : ''}</span>
            </div>
            <div class="wire-sees"><span class="wire-label">${esc(colSees)}</span>${esc(row.sees)}</div>
            <div class="wire-result"><span class="wire-label">${esc(colResult)}</span><span class="wire-mark">${icon(mark[row.kind], 14)}</span><span>${esc(row.result)}</span></div>
          </li>`,
            )
            .join('')}
        </ol>
      </section>`,
        )
        .join('')}
    </figure>`;
  };

  return `<!doctype html>
<html lang="${lang}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${esc(t.title)}</title>
  <meta name="description" content="${esc(t.description)}">
  <meta name="theme-color" content="#0d1117">
  <meta name="color-scheme" content="dark">
  <link rel="canonical" href="${url}">
  ${LANGS.map((entry) => `<link rel="alternate" hreflang="${entry.code}" href="${SITE}${entry.dir}">`).join('\n  ')}
  <link rel="alternate" hreflang="x-default" href="${SITE}">
  <link rel="icon" href="${base}assets/logo.svg" type="image/svg+xml">
  <link rel="icon" href="${base}assets/favicon-32.png" sizes="32x32" type="image/png">
  <link rel="apple-touch-icon" href="${base}assets/apple-touch-icon.png">
  <link rel="preload" href="${base}assets/fonts/mona-sans-latin.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="stylesheet" href="${base}assets/site.css">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Robz DPI">
  <meta property="og:title" content="${esc(t.title)}">
  <meta property="og:description" content="${esc(t.description)}">
  <meta property="og:url" content="${url}">
  <meta property="og:image" content="${SITE}assets/img/og-${lang}.png">
  <meta property="og:image:width" content="1200">
  <meta property="og:image:height" content="630">
  <meta property="og:locale" content="${t.locale}">
  <meta name="twitter:card" content="summary_large_image">
  <script type="application/ld+json">${JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'Robz DPI',
    operatingSystem: 'Windows 10, Windows 11, Linux',
    applicationCategory: 'UtilitiesApplication',
    description: t.description,
    downloadUrl: DOWNLOAD,
    softwareVersion: live.version,
    license: 'https://opensource.org/licenses/MIT',
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
  }).replace(/</g, '\\u003c')}</script>
</head>
<body>
  <a class="skip" href="#main">${esc(t.skip)}</a>
  <header class="top">
    <div class="wrap top-inner">
      <a class="brand" href="${home}"><img src="${base}assets/logo.svg" width="28" height="28" alt=""><span>Robz <b>DPI</b></span></a>
      <nav class="top-nav" aria-label="${esc(t.title)}">
        <a href="#how">${esc(t.nav.how)}</a>
        <a href="#usage">${esc(t.nav.usage)}</a>
        <a href="#faq">${esc(t.nav.faq)}</a>
        <a href="${REPO_URL}">${icon('github')}<span>${esc(t.nav.github)}</span></a>
      </nav>
      <details class="lang" data-lang-menu>
        <summary aria-label="${esc(t.langLabel)}: ${self.name}">${FLAGS[lang]}<span>${self.name}</span><svg class="i lang-chevron" width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 6 8 10.5 12.5 6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg></summary>
        <ul>
          ${LANGS.map(
            (entry) =>
              `<li><a href="${langHref(entry)}" hreflang="${entry.code}" lang="${entry.code}"${entry.code === lang ? ' aria-current="page"' : ''}>${FLAGS[entry.code]}<span>${entry.name}</span></a></li>`,
          ).join('\n          ')}
        </ul>
      </details>
    </div>
  </header>

  <main id="main">
    <section class="hero">
      <div class="wrap hero-grid">
        <div class="hero-text">
          <a class="pill" href="#legal">
            <span class="pill-tag">${icon('alert', 12)}<span>${esc(t.pillTag)}</span></span>
            <span>${esc(t.pillText)}</span>
          </a>
          <h1>${esc(t.heroTitle)}</h1>
          <p class="lead">${esc(t.heroLead)}</p>
          ${downloadButtons({ big: true })}
          <ul class="facts">
            <li class="fact-version">${icon('tag', 12)}<a href="${REPO_URL}/releases/latest" data-release-link><span data-version>v${esc(live.version)}</span></a></li>
            ${t.facts.map((fact) => `<li>${esc(fact)}</li>`).join('')}
          </ul>
        </div>
        <div class="hero-shots">
          ${windowFrame('01-off.png', t.shotBackAlt, { cls: 'shot-back' })}
          ${windowFrame('02-on.png', t.shotAlt, { eager: true, cls: 'shot-front' })}
        </div>
      </div>
    </section>

    <section class="band" aria-labelledby="why-title">
      <div class="wrap split">
        <div>
          <h2 id="why-title">${esc(t.whyTitle)}</h2>
          <p class="section-lead">${esc(t.whyLead)}</p>
        </div>
        ${diff()}
      </div>
    </section>

    <section class="section" id="how" aria-labelledby="how-title">
      <div class="wrap">
        <h2 id="how-title">${esc(t.howTitle)}</h2>
        <p class="section-lead">${esc(t.howLead)}</p>
        ${wire()}
        <ol class="how">
          ${t.how.map(([title, text], i) => `<li><span class="how-num" aria-hidden="true">${i + 1}</span><h3>${esc(title)}</h3><p>${esc(text)}</p></li>`).join('')}
        </ol>
        <p class="note how-note">${icon('info')}<span>${esc(t.howNote)}</span></p>
      </div>
    </section>

    <section class="section" id="usage" aria-labelledby="usage-title">
      <div class="wrap">
        <h2 id="usage-title">${esc(t.usageTitle)}</h2>
        <p class="section-lead">${esc(t.usageLead)}</p>
        <ol class="steps">
          ${t.steps
            .map(
              (step, i) => `<li class="step">
            <div class="step-text">
              <span class="step-num" aria-hidden="true">${i + 1}</span>
              <h3>${esc(step.title)}</h3>
              <p>${esc(step.text)}</p>
              <p class="note">${icon('info')}<span>${esc(step.note)}</span></p>
            </div>
            ${windowFrame(step.shot, step.shotAlt, { crop: true })}
          </li>`,
            )
            .join('')}
        </ol>
      </div>
    </section>

    <section class="section" aria-labelledby="details-title">
      <div class="wrap">
        <h2 id="details-title">${esc(t.detailsTitle)}</h2>
        <dl class="details">
          ${t.details.map(([title, text]) => `<div><dt>${esc(title)}</dt><dd>${esc(text)}</dd></div>`).join('')}
        </dl>
      </div>
    </section>

    <section class="section" aria-labelledby="screens-title">
      <div class="wrap">
        <h2 id="screens-title">${esc(t.screensTitle)}</h2>
        <div class="screens">
          ${t.screens
            .map(([file, caption]) => {
              const height = shotHeight(lang, file);
              const long = height > 1600;
              const focus = SCREEN_FOCUS[file]; // uzun görüntünün gösterilecek yeri (yüzde); yoksa en üstü
              return `<figure>
            <a class="${long ? 'screen-long' : ''}${focus ? ' screen-mid' : ''}" href="${img(file)}" title="${esc(t.openImage)}">
              <img src="${img(file)}" width="${SHOT_WIDTH}" height="${height}" alt="${esc(caption)}" loading="lazy"${focus ? ` style="object-position: 0 ${focus}%"` : ''}>
              ${long ? `<span class="screen-more">${icon('expand', 14)}${esc(t.openImage)}</span>` : ''}
            </a>
            <figcaption>${esc(caption)}</figcaption>
          </figure>`;
            })
            .join('')}
        </div>
      </div>
    </section>

    <section class="section" id="faq" aria-labelledby="faq-title">
      <div class="wrap split split-top">
        <h2 id="faq-title">${esc(t.faqTitle)}</h2>
        <div class="faq">
          ${t.faq.map(([question, answer]) => `<details><summary>${esc(question)}</summary><p>${esc(answer)}</p></details>`).join('')}
        </div>
      </div>
    </section>

    <section class="section" id="legal" aria-labelledby="legal-title">
      <div class="wrap">
        <div class="legal">
          <div class="legal-head">
            <span class="legal-icon">${icon('alert', 20)}</span>
            <div>
              <h2 id="legal-title">${esc(t.legalTitle)}</h2>
              <p>${esc(t.legalLead)}</p>
            </div>
          </div>
          <dl class="legal-points">
            ${t.legal.map(([title, text]) => `<div><dt>${esc(title)}</dt><dd>${esc(text)}</dd></div>`).join('')}
          </dl>
          <p class="legal-link"><a href="${REPO_URL}/blob/main/DISCLAIMER.md#${encodeURIComponent(t.legalAnchor)}">${esc(t.legalLink)}${icon('arrow', 14)}</a></p>
        </div>
      </div>
    </section>

    <section class="final" aria-labelledby="final-title">
      <div class="wrap">
        <h2 id="final-title">${esc(t.finalTitle)}</h2>
        ${downloadButtons({ big: true })}
        <p class="final-text">${esc(t.finalText)}</p>
        <p class="final-links">
          <a href="${DISCORD}">${icon('discord')}${esc(t.discord)}</a>
          <a href="${REPO_URL}/issues">${icon('github')}${esc(t.issues)}</a>
        </p>
      </div>
    </section>
  </main>

  <footer class="foot">
    <div class="wrap foot-inner">
      <a class="brand" href="${home}"><img src="${base}assets/logo.svg" width="24" height="24" alt=""><span>Robz <b>DPI</b></span></a>
      <p>${esc(t.footerLicense)} <a href="${REPO_URL}">${REPO}</a></p>
      <p class="foot-small">${esc(t.footerDisclaimer)}</p>
      <p><a href="${ROBZ_HUB}">${esc(t.footerSibling)}</a></p>
      <p class="foot-langs">${LANGS.map((entry) =>
        entry.code === lang ? `<span aria-current="page">${entry.name}</span>` : `<a href="${langHref(entry)}" hreflang="${entry.code}" lang="${entry.code}">${entry.name}</a>`,
      ).join('<span aria-hidden="true"> · </span>')}</p>
    </div>
  </footer>
  <script src="${base}assets/site.js" defer></script>
</body>
</html>
`;
}

for (const { code, dir } of LANGS) writeFileSync(join(out, dir, 'index.html'), page(content[code]));
writeFileSync(join(out, '.nojekyll'), '');
console.log(`site/dist yazıldı (Robz DPI ${live.version}, diller: ${LANGS.map((entry) => entry.code).join(', ')}, yıldız: ${live.stars ?? '-'})`);
