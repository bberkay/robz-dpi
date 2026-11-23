// Dil: Türkçe, İngilizce ve Rusça. Ayar "auto" ise Windows'un dili; bu üçünden biri değilse İngilizce.
import { store } from './store.js';
import tr from './locales/tr.js';
import en from './locales/en.js';
import ru from './locales/ru.js';

const DICTS = { tr, en, ru };

// Arayüz dilleri, seçim kutularında görünecekleri sırayla. Her birinin adı sözlükte `lang.<kod>` anahtarındadır.
export const LANGUAGES = Object.keys(DICTS);

// Dil seçim kutusunun seçenekleri: [{ value, label, flag }]
export function languageOptions() {
  return LANGUAGES.map((code) => ({ value: code, label: t(`lang.${code}`), flag: code }));
}

export function lang() {
  const data = store.data;
  if (!data) return 'tr';
  const setting = data.settings.language;
  const code = setting === 'auto' ? data.systemLanguage : setting;
  return code in DICTS ? code : 'en';
}

// t('tab.home') ya da t('test.ms', { ms: 120 }). Sözlükte yoksa anahtarın kendisi döner.
// Windows dışında bir sistemde önce "anahtar@sistem" aranır ('power.admin@linux'): yönetici onayı, servis ve
// kaldırma gibi sisteme göre değişen metinler böyle ayrılır.
export function t(key, vars) {
  const platform = store.data?.platform;
  const dict = DICTS[lang()];
  const own = platform && platform !== 'windows' ? `${key}@${platform}` : null;
  let value = (own && (dict[own] ?? DICTS.tr[own])) ?? dict[key] ?? DICTS.tr[key] ?? key;
  if (typeof value === 'function') return value(vars || {});
  if (vars) value = value.replace(/\{(\w+)\}/g, (_, name) => (name in vars ? vars[name] : `{${name}}`));
  return value;
}

// ['Discord', 'Roblox', '2 site'] -> "Discord, Roblox ve 2 site" / "Discord, Roblox, and 2 sites"
export function list(items) {
  return new Intl.ListFormat(lang(), { style: 'long', type: 'conjunction' }).format(items);
}
