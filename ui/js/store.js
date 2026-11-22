// Arayüzün tek durum deposu. Ana süreçten gelen durum (data), açık sekme (tab), bildirim baloncukları (toasts)
// ve onay penceresi (dialog) burada tutulur.
import { useEffect, useState } from '../vendor/preact-htm.js';

const listeners = new Set();
let toastId = 0;

export const store = {
  data: null,
  tab: 'home', // 'home' | 'settings'
  toasts: [],
  dialog: null,
  log: null, // açık günlük penceresinin metni
};

let version = 0;

export function update(patch) {
  Object.assign(store, patch);
  version += 1;
  for (const listener of listeners) listener();
}

export function useStore() {
  const [, rerender] = useState(0);
  const seen = version;
  useEffect(() => {
    const listener = () => rerender((n) => n + 1);
    listeners.add(listener);
    // İlk çizim ile abonelik arasında durum değiştiyse (ana süreç çok hızlı yanıt verdiyse) yeniden çiz.
    if (seen !== version) listener();
    return () => listeners.delete(listener);
  }, []);
  return store;
}

export function showTab(tab) {
  update({ tab });
  document.querySelector('.content')?.scrollTo(0, 0);
}

export function toast(message, { kind = 'info', timeout = 6000 } = {}) {
  const id = ++toastId;
  update({ toasts: [...store.toasts, { id, message, kind }] });
  if (timeout) setTimeout(() => dismissToast(id), timeout);
}

export function dismissToast(id) {
  update({ toasts: store.toasts.filter((t) => t.id !== id) });
}

export function openDialog(dialog) {
  update({ dialog });
}

export function closeDialog() {
  update({ dialog: null });
}

// Ana süreç çağrısı. Hata olursa { code, detail } fırlatır.
export async function call(method, ...args) {
  const result = await window.dpi.call(method, ...args);
  if (!result.ok) throw result.error;
  return result.value;
}
