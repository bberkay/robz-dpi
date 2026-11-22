// WinDivert sürücüsünü resmi sürümünden indirir ve src-tauri/engine altına çıkarır. (Motorun kendisi bu
// depodaki Rust kodudur; dışarıdan gelen tek ikili dosya imzalı sürücüdür.)
//
//   npm run engine                     indir, doğrula, çıkar
//   node scripts/fetch-engine.mjs --optional   (npm install sonunda) indirilemezse uyarır, kurulumu durdurmaz
//
// Güven zinciri engine.lock.json'dadır: indirilen zip'in ve içinden alınan her dosyanın SHA-256 özeti orada yazılı
// değerle aynı değilse hiçbir dosya yazılmaz. Dosyalar zaten yerinde ve özetleri doğruysa indirme yapılmaz.
// Ek paket gerekmez: zip okuyucu aşağıdadır (yalnız "stored" ve "deflate" girdileri).
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const lock = JSON.parse(readFileSync(join(root, 'engine.lock.json'), 'utf8'));
const outDir = join(root, 'src-tauri', 'engine');
const optional = process.argv.includes('--optional');

const sha256 = (buffer) => createHash('sha256').update(buffer).digest('hex');

function alreadyThere() {
  return lock.files.every((file) => {
    const path = join(outDir, file.to);
    return existsSync(path) && sha256(readFileSync(path)) === file.sha256;
  });
}

// Zip'in merkezi dizinini okur: ad -> { method, compressedSize, size, offset }
function readZipIndex(zip) {
  let end = zip.length - 22;
  while (end >= 0 && zip.readUInt32LE(end) !== 0x06054b50) end -= 1;
  if (end < 0) throw new Error('zip sonu bulunamadı');
  const count = zip.readUInt16LE(end + 10);
  let pos = zip.readUInt32LE(end + 16);
  const index = new Map();
  for (let i = 0; i < count; i += 1) {
    if (zip.readUInt32LE(pos) !== 0x02014b50) throw new Error('zip dizini bozuk');
    const nameLength = zip.readUInt16LE(pos + 28);
    const extraLength = zip.readUInt16LE(pos + 30);
    const commentLength = zip.readUInt16LE(pos + 32);
    index.set(zip.toString('utf8', pos + 46, pos + 46 + nameLength), {
      method: zip.readUInt16LE(pos + 10),
      compressedSize: zip.readUInt32LE(pos + 20),
      size: zip.readUInt32LE(pos + 24),
      offset: zip.readUInt32LE(pos + 42),
    });
    pos += 46 + nameLength + extraLength + commentLength;
  }
  return index;
}

function readZipEntry(zip, entry) {
  if (zip.readUInt32LE(entry.offset) !== 0x04034b50) throw new Error('zip girdisi bozuk');
  const start = entry.offset + 30 + zip.readUInt16LE(entry.offset + 26) + zip.readUInt16LE(entry.offset + 28);
  const raw = zip.subarray(start, start + entry.compressedSize);
  if (entry.method === 0) return Buffer.from(raw);
  if (entry.method === 8) return inflateRawSync(raw, { maxOutputLength: 64 * 1024 * 1024 });
  throw new Error(`desteklenmeyen sıkıştırma: ${entry.method}`);
}

async function main() {
  if (alreadyThere()) {
    console.log(`engine: ${lock.name} zaten yerinde ve doğrulandı -> src-tauri/engine`);
    return;
  }

  console.log(`engine: indiriliyor: ${lock.url}`);
  const response = await fetch(lock.url, { redirect: 'follow' });
  if (!response.ok) throw new Error(`indirilemedi: HTTP ${response.status}`);
  const zip = Buffer.from(await response.arrayBuffer());

  const digest = sha256(zip);
  if (digest !== lock.sha256) {
    throw new Error(`zip özeti tutmuyor\n  beklenen: ${lock.sha256}\n  gelen:    ${digest}`);
  }

  const index = readZipIndex(zip);
  const extracted = lock.files.map((file) => {
    const entry = index.get(file.from);
    if (!entry) throw new Error(`zip içinde yok: ${file.from}`);
    const data = readZipEntry(zip, entry);
    if (sha256(data) !== file.sha256) throw new Error(`dosya özeti tutmuyor: ${file.from}`);
    return [file.to, data];
  });

  // Bütün özetler tuttuktan sonra yaz.
  for (const [to, data] of extracted) {
    const path = join(outDir, to);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, data);
  }
  console.log(`engine: ${extracted.length} dosya doğrulandı ve yazıldı -> src-tauri/engine`);
}

main().catch((error) => {
  if (optional) {
    console.warn(`engine: atlandı (${error.message}). Sonra elle çalıştır: npm run engine`);
  } else {
    console.error(`engine: ${error.message}`);
    process.exitCode = 1;
  }
});
