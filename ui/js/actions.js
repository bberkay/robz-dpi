// Kullanıcı eylemleri: gerekirse onay ister, ana süreci çağırır, hatayı okunur bir bildirime çevirir.
import { call, openDialog, store, toast } from './store.js';
import { list, t } from './i18n.js';

export function errorText(error) {
  const code = error?.code || 'unknown';
  const key = `error.${code}`;
  const message = t(key);
  return message === key ? t('error.unknown') : message;
}

export async function run(method, ...args) {
  try {
    return await call(method, ...args);
  } catch (error) {
    toast(errorText(error), { kind: 'error' });
    return undefined;
  }
}

export const set = (key, value) => run('setSetting', key, value);

// Büyük düğme: çalışıyorsa kapatır, duruyorsa açar. Yönetici izni gerekiyorsa Windows kendisi sorar.
export function toggle() {
  return run(store.data.engine.state === 'running' ? 'turnOff' : 'turnOn');
}

export const apply = () => run('apply');
export const test = () => run('test');
export const dismissError = () => run('dismissError');
export const setService = (id, on) => run('setService', id, on);
export const removeDomain = (domain) => run('removeDomain', domain);
export const openExternal = (url) => run('openExternal', url);

// Yazılan ya da yapıştırılan adresleri ekler. Eklenen olduysa true (kutu temizlensin diye).
export async function addDomains(text) {
  if (!text.trim()) return false;
  const result = await run('addDomains', text);
  if (!result) return false;
  if (result.rejected.length) toast(t('custom.rejected', { list: list(result.rejected) }), { kind: 'error' });
  else if (!result.added.length) toast(t('custom.exists'));
  return result.added.length > 0;
}

// Bilgisayarda çalışan GoodbyeDPI'ı durdurmadan önce sorar.
export function stopConflicts() {
  openDialog({
    title: t('dialog.conflict.title'),
    body: t('dialog.conflict.body'),
    confirm: t('dialog.conflict.confirm'),
    danger: true,
    onConfirm: () => run('stopConflicts'),
  });
}

export function removeAll() {
  openDialog({
    title: t('dialog.remove.title'),
    body: t('dialog.remove.body'),
    confirm: t('dialog.remove.confirm'),
    danger: true,
    onConfirm: () => run('removeAll'),
  });
}
