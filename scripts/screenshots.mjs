// Arayüzün ekran görüntülerini üç dilde alır:  node scripts/screenshots.mjs
//
//   docs/screenshots/tr/…   docs/screenshots/en/…   docs/screenshots/ru/…
//
// Arayüzü (ui/) geçici bir yerel sunucudan, sahte çekirdekle (ui/js/dev-mock.js) açar. Uygulamayı ve motoru
// çalıştırmaz, bilgisayarın ağ ayarlarına dokunmaz: görüntüdeki sayılar ve alan adları örnek veridir.
// Microsoft Edge'i başsız kipte açar ve hata ayıklama bağlantı noktasıyla (Chrome DevTools Protocol) yönetir;
// ek paket gerekmez. README'ler ve web sitesi her dilde o dilin görüntülerini kullanır.
import { spawn } from 'node:child_process';
import { createReadStream, existsSync, mkdirSync, mkdtempSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const ui = join(root, 'ui');
const out = join(root, 'docs', 'screenshots');
const EDGE = ['C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', 'C:/Program Files/Microsoft/Edge/Application/msedge.exe'].find(existsSync);
if (!EDGE) throw new Error('Microsoft Edge bulunamadı');

const DEBUG_PORT = 9335;
const WIDTH = 520;
const HEIGHT = 800; // uygulama penceresinin boyu
const LANGS = ['tr', 'en', 'ru'];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.woff2': 'font/woff2', '.json': 'application/json' };
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// [dosya adı, adres sorgusu, sayfanın tamamı mı?]
// "Tamamı" seçilirse görünür alan içeriğin boyuna uzatılır: uzun sayfalar (ayarlar) kaydırmadan tek görüntüde çıkar.
const SHOTS = [
  ['01-off.png', 'sites', false],
  ['02-on.png', 'on', false],
  ['03-settings.png', 'on&tab=settings', true],
  ['04-advanced.png', 'on&tab=settings&advanced', true],
  ['05-welcome.png', 'wizard', false],
  ['06-terms.png', 'wizard&step=2', false],
];

const server = createServer((request, response) => {
  const path = normalize(join(ui, decodeURIComponent(new URL(request.url, 'http://localhost').pathname)));
  const file = path.startsWith(ui) && existsSync(path) && statSync(path).isDirectory() ? join(path, 'index.html') : path;
  if (!file.startsWith(ui) || !existsSync(file)) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { 'Content-Type': TYPES[extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
  createReadStream(file).pipe(response);
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'robzdpi-shot-'));
const browser = spawn(
  EDGE,
  ['--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run', `--user-data-dir=${profile}`, `--remote-debugging-port=${DEBUG_PORT}`, 'about:blank'],
  { stdio: 'ignore' },
);

async function connect() {
  for (let i = 0; i < 80; i += 1) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${DEBUG_PORT}/json/list`)).json();
      const page = targets.find((target) => target.type === 'page');
      if (page) return new WebSocket(page.webSocketDebuggerUrl);
    } catch {
      // henüz açılmadı
    }
    await sleep(250);
  }
  throw new Error('tarayıcıya bağlanılamadı');
}

let failed = false;
try {
  const socket = await connect();
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = reject;
  });
  let id = 0;
  const pending = new Map();
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    pending.get(message.id)?.(message);
    pending.delete(message.id);
  };
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      id += 1;
      pending.set(id, resolve);
      socket.send(JSON.stringify({ id, method, params }));
    });
  const viewport = (height) => send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height, deviceScaleFactor: 2, mobile: false });

  for (const lang of LANGS) {
    mkdirSync(join(out, lang), { recursive: true });
    for (const [name, query, whole] of SHOTS) {
      await viewport(HEIGHT);
      await send('Page.navigate', { url: `${base}?lang=${lang}${query ? `&${query}` : ''}` });
      await sleep(1500); // yazı tipi, ilk durum ve (varsa) sekme geçişi
      if (whole) {
        // Kaydırılan bölümün başladığı yer + içeriğinin tam boyu
        const measured = await send('Runtime.evaluate', {
          expression: `(() => { const c = document.querySelector('.content'); return Math.ceil(c.getBoundingClientRect().top + c.scrollHeight); })()`,
          returnByValue: true,
        });
        await viewport(Math.min(Math.max(measured.result.result.value, HEIGHT), 6000));
        await sleep(400);
      }
      const shot = await send('Page.captureScreenshot', { format: 'png' });
      writeFileSync(join(out, lang, name), Buffer.from(shot.result.data, 'base64'));
      console.log(`docs/screenshots/${lang}/${name}`);
    }
  }
  socket.close();
} catch (error) {
  console.error(`screenshots: ${error.message}`);
  failed = true;
} finally {
  browser.kill();
  server.close();
  await sleep(500);
  rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
}
process.exit(failed ? 1 : 0);
