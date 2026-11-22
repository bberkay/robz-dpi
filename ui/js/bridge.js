// Arayüz ile Rust çekirdeği arasındaki tek köprü (Tauri).
//   window.dpi.call(method, ...args)  -> { ok: true, value } | { ok: false, error: { code, detail } }
//   window.dpi.on('state', callback)  -> dinlemeyi bırakan işlev döner
//
// Arayüz Tauri dışında (düz bir tarayıcıda) açılırsa çekirdeğin yerine dev-mock.js geçer: tasarım, bilgisayara
// dokunmadan denenir.
if (!window.__TAURI__) await import('./dev-mock.js');

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const EVENTS = new Set(['state']);

window.dpi = {
  async call(method, ...args) {
    try {
      return { ok: true, value: await invoke('dpi_call', { method, args }) };
    } catch (error) {
      const known = error && typeof error === 'object' && 'code' in error;
      return { ok: false, error: known ? error : { code: 'unknown', detail: String(error) } };
    }
  },
  on(event, callback) {
    if (!EVENTS.has(event)) return () => {};
    const unlisten = listen(`dpi:${event}`, (e) => callback(e.payload));
    return () => unlisten.then((stop) => stop());
  },
};
