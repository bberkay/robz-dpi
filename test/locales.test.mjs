// Türkçe ve İngilizce arayüz metinleri aynı anahtarlara sahip olmalı ve arayüzde kullanılan her anahtar bulunmalı.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import tr from '../ui/js/locales/tr.js';
import en from '../ui/js/locales/en.js';

const root = fileURLToPath(new URL('../ui/js/', import.meta.url));

function sources(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return entry.name === 'locales' ? [] : sources(path);
    return entry.name.endsWith('.js') ? [readFileSync(path, 'utf8')] : [];
  });
}

test('iki dilde aynı anahtarlar var', () => {
  assert.deepEqual(Object.keys(en).sort(), Object.keys(tr).sort());
});

test('yer tutucular iki dilde aynı', () => {
  for (const key of Object.keys(tr)) {
    if (typeof tr[key] !== 'string') continue;
    const vars = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    assert.deepEqual(vars(en[key]), vars(tr[key]), key);
  }
});

test("arayüzde t('…') ile kullanılan her anahtar sözlükte var", () => {
  const used = new Set();
  for (const text of sources(root)) {
    for (const m of text.matchAll(/\bt\('([a-zA-Z0-9.-]+)'/g)) used.add(m[1]);
  }
  const missing = [...used].filter((key) => !(key in tr));
  assert.deepEqual(missing, []);
  assert.ok(used.size > 50, `yalnız ${used.size} anahtar bulundu`);
});

// Kod, anahtarı çalışma anında kuruyor: t(`power.${busy}`), t(`method.${id}`), t(`problem.${code}`),
// t(`engineCode.${kod}`), t(`error.${code}`).
test('çalışma anında kurulan anahtarlar sözlükte var', () => {
  const dynamic = [
    ...['starting', 'stopping', 'applying', 'removing', 'fixing'].map((busy) => `power.${busy}`),
    ...['m1', 'm2', 'm3', 'm4'].map((id) => `method.${id}`),
    ...['nothing-selected', 'bad-dns', 'empty-strategy', 'bad-strategy'].map((code) => `problem.${code}`),
    ...['home', 'settings'].map((tab) => `tab.${tab}`),
    // Motorun çıkış kodları: WinDivert'in bilinen hataları ve engine/service.rs'teki EXIT_* sabitleri.
    ...[2, 5, 577, 654, 1275, 1753, 9001, 9002, 9003].map((code) => `engineCode.${code}`),
  ];
  assert.deepEqual(dynamic.filter((key) => !(key in tr)), []);
});

test('motorun çıkış kodlarının metni var', () => {
  const service = readFileSync(fileURLToPath(new URL('../src-tauri/src/engine/service.rs', import.meta.url)), 'utf8');
  const codes = [...service.matchAll(/pub const EXIT_[A-Z_]+: u32 = ([0-9]+);/g)].map((m) => m[1]);
  assert.ok(codes.length >= 3, `yalnız ${codes.length} sabit bulundu`);
  assert.deepEqual(codes.filter((code) => !(`engineCode.${code}` in tr)), []);
});

// Rust tarafının üretebildiği her hata kodunun bir metni olmalı; yoksa kullanıcı "Bir şeyler ters gitti" görür.
test('Rust kodundaki hata kodlarının metni var', () => {
  const rust = fileURLToPath(new URL('../src-tauri/src/', import.meta.url));
  const files = readdirSync(rust, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
    .map((entry) => readFileSync(join(entry.parentPath, entry.name), 'utf8'));
  const codes = new Set();
  for (const text of files) {
    for (const m of text.matchAll(/DpiError::(?:new|with)\(\s*"([a-z-]+)"/g)) codes.add(m[1]);
    for (const m of text.matchAll(/io_error\("([a-z-]+)"\)/g)) codes.add(m[1]);
  }
  codes.delete('cancelled'); // yönetici onayı reddedildi: hata olarak gösterilmez
  assert.ok(codes.size > 10, `yalnız ${codes.size} kod bulundu`);
  assert.deepEqual([...codes].filter((code) => !(`error.${code}` in tr)).sort(), []);
});
