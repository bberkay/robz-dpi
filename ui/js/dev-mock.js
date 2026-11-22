// Geliştirme için sahte çekirdek. Yalnız arayüz Tauri dışında, düz bir tarayıcıda açıldığında yüklenir
// (bkz. bridge.js); kurulu uygulamada hiç çalışmaz. Bilgisayara dokunmaz: motoru açmış gibi yapar.
//
// Adres çubuğundan başlangıç durumu seçilebilir:
//   ?lang=en            İngilizce
//   ?on                 motor açık
//   ?conflict           başka bir DPI aracı çalışıyor
//   ?error=<kod>        hata kutusu (örnek: engine-stopped)
const query = new URLSearchParams(location.search);
const listeners = new Set();
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const catalog = {
  services: [
    { id: 'discord', name: 'Discord', domains: Array.from({ length: 22 }, (_, i) => `d${i}.example`) },
    { id: 'roblox', name: 'Roblox', domains: Array.from({ length: 8 }, (_, i) => `r${i}.example`) },
  ],
  methods: [
    { id: 'm1', flags: '-5 --set-ttl 5' },
    { id: 'm2', flags: '-5' },
    { id: 'm3', flags: '--set-ttl 3' },
    { id: 'm4', flags: '-9' },
  ],
  dns: [
    { id: 'quad9', name: 'Quad9', address: '9.9.9.9', port: 9953 },
    { id: 'yandex', name: 'Yandex', address: '77.88.8.8', port: 1253 },
    { id: 'adguard', name: 'AdGuard', address: '94.140.14.140', port: 5353 },
    { id: 'opendns', name: 'OpenDNS', address: '208.67.222.222', port: 5353 },
  ],
};

const settings = {
  language: query.get('lang') || 'tr',
  closeToTray: true,
  launchAtLogin: false,
  enabled: query.has('on'),
  scope: 'selected',
  services: ['discord', 'roblox'],
  customDomains: [],
  method: 'm1',
  dnsEnabled: true,
  dnsProvider: 'quad9',
  dnsCustomAddr: '',
  dnsCustomPort: 53,
};

const ENGINE_KEYS = ['scope', 'services', 'customDomains', 'method', 'dnsEnabled', 'dnsProvider', 'dnsCustomAddr', 'dnsCustomPort'];
const fingerprint = () => JSON.stringify(ENGINE_KEYS.map((key) => settings[key]));

const engine = {
  installed: query.has('on'),
  state: query.has('on') ? 'running' : 'stopped',
  busy: null,
  applied: query.has('on') ? fingerprint() : null,
  error: query.get('error') ? { code: query.get('error'), detail: null } : null,
};
let conflicts = query.has('conflict') ? [{ name: 'goodbyedpi.exe', pid: 4408 }] : [];
const probe = { running: false, results: [] };

function problem() {
  if (settings.scope === 'selected' && !settings.services.length && !settings.customDomains.length) return 'nothing-selected';
  if (settings.dnsEnabled && settings.dnsProvider === 'custom' && !settings.dnsCustomAddr) return 'bad-dns';
  return null;
}

function snapshot() {
  const matches = engine.installed && engine.applied === fingerprint();
  return structuredClone({
    version: '0.1.0',
    packaged: true,
    systemLanguage: 'tr',
    supported: true,
    engineReady: true,
    settings,
    catalog,
    links: {
      github: 'https://github.com/kutsalkiremitci/robz-dpi',
      discord: 'https://discord.gg/UyyEn7SHm4',
      kick: 'https://kick.com/robzsogood',
    },
    engine: {
      installed: engine.installed,
      state: engine.state,
      busy: engine.busy,
      pending: engine.state === 'running' && !problem() && !matches,
      needsAdmin: !matches,
      problem: problem(),
      error: engine.error,
    },
    conflicts,
    probe,
  });
}

function emit() {
  const payload = snapshot();
  for (const listener of listeners) listener({ payload });
}

async function operate(busy, work) {
  engine.error = null;
  engine.busy = busy;
  emit();
  await wait(900);
  work();
  engine.busy = null;
  emit();
}

function normalize(text) {
  const host = text.trim().toLowerCase().replace(/^[a-z]+:\/\//, '').replace(/[/?#].*$/, '').replace(/^www\./, '');
  return /^([a-z0-9-]+\.)+[a-z][a-z0-9-]*$/.test(host) ? host : null;
}

const methods = {
  getState: () => snapshot(),
  turnOn: () =>
    operate('starting', () => {
      Object.assign(engine, { installed: true, state: 'running', applied: fingerprint() });
      settings.enabled = true;
    }),
  turnOff: () =>
    operate('stopping', () => {
      engine.state = 'stopped';
      settings.enabled = false;
    }),
  apply: () => operate('applying', () => (engine.applied = fingerprint())),
  removeAll: () => operate('removing', () => Object.assign(engine, { installed: false, state: 'stopped', applied: null })),
  stopConflicts: () => operate('fixing', () => (conflicts = [])),
  dismissError: () => {
    engine.error = null;
    emit();
  },
  setSetting: (key, value) => {
    if (!(key in settings)) throw { code: 'bad-request', detail: key };
    settings[key] = value;
    emit();
  },
  setService: (id, on) => {
    settings.services = catalog.services.map((s) => s.id).filter((s) => (s === id ? on : settings.services.includes(s)));
    emit();
  },
  addDomains: (text) => {
    const added = [];
    const rejected = [];
    for (const part of text.split(/[\s,;]+/).filter(Boolean)) {
      const domain = normalize(part);
      if (!domain) rejected.push(part);
      else if (!settings.customDomains.includes(domain)) {
        settings.customDomains.push(domain);
        added.push(domain);
      }
    }
    emit();
    return { added, rejected };
  },
  removeDomain: (domain) => {
    settings.customDomains = settings.customDomains.filter((d) => d !== domain);
    emit();
  },
  test: async () => {
    probe.running = true;
    emit();
    await wait(1200);
    const on = engine.state === 'running';
    const names = [
      ...catalog.services.filter((s) => settings.scope === 'all' || settings.services.includes(s.id)).map((s) => [s.id, s.name]),
      ...settings.customDomains.map((d) => [d, d]),
    ];
    probe.results = names.map(([id, label], i) => ({ id, label, ok: on, ms: on ? 140 + i * 37 : 6000 }));
    probe.running = false;
    emit();
  },
  openExternal: (url) => window.open(url, '_blank', 'noopener'),
};

window.__TAURI__ = {
  core: {
    async invoke(_command, { method, args }) {
      if (!methods[method]) throw { code: 'bad-request', detail: method };
      return (await methods[method](...args)) ?? null;
    },
  },
  event: {
    async listen(_name, callback) {
      listeners.add(callback);
      return () => listeners.delete(callback);
    },
  },
};
