// Ayarlar: yöntem, DNS, uygulama davranışı, meraklısı için gelişmiş bölüm ve hakkında.
import { html, useEffect, useState } from '../../vendor/preact-htm.js';
import { Icon } from '../icons.js';
import { list, t } from '../i18n.js';
import { toast, useStore } from '../store.js';
import * as actions from '../actions.js';
import { LanguageSelect, NumberField, Toggle } from '../components.js';

function Row({ title, desc, children }) {
  return html`<div class="row">
    <div class="row-text">
      <div class="row-title">${title}</div>
      ${desc && html`<p class="muted">${desc}</p>`}
    </div>
    <div class="row-control">${children}</div>
  </div>`;
}

// Seçili yöntemin stratejisi: hazır yöntemse katalogdaki, "özel"se kullanıcının kurduğu.
function currentStrategy(data) {
  const preset = data.catalog.methods.find((method) => method.id === data.settings.method);
  return preset ? preset.strategy : data.settings.custom;
}

// "2 bayttan böl · ters sıra · sahte paket (ömür 5)": stratejinin tek satırlık özeti.
function summary(strategy) {
  const parts = [];
  if (strategy.splitAtSni) parts.push(t('summary.splitAtSni'));
  else if (strategy.split > 0) parts.push(t('summary.split', { n: strategy.split }));
  if ((strategy.split > 0 || strategy.splitAtSni) && strategy.reverse) parts.push(t('summary.reverse'));
  if (strategy.fakeTtlMode === 'fixed') parts.push(t('summary.fakeFixed', { n: strategy.fakeTtl }));
  if (strategy.fakeTtlMode === 'auto') parts.push(t('summary.fakeAuto'));
  if (strategy.fakeBadChecksum) parts.push(t('summary.badChecksum'));
  if (strategy.fakeBadSeq) parts.push(t('summary.badSeq'));
  if (strategy.blockQuic) parts.push(t('summary.blockQuic'));
  return parts.join(' · ') || t('summary.nothing');
}

function Methods() {
  const { data } = useStore();
  const current = data.settings.method;
  const choice = (id, title, desc, extra) => html`<button
    class="choice"
    role="radio"
    key=${id}
    aria-checked=${id === current}
    onClick=${() => id !== current && actions.set('method', id)}
  >
    <span class="choice-mark"></span>
    <span class="row-text">
      <span class="row-title">${title}${extra}</span>
      <p class="muted">${desc}</p>
    </span>
  </button>`;

  return html`<section class="section" style=${{ marginTop: 0 }}>
    <div class="section-head"><h2 class="section-title">${t('settings.method')}</h2></div>
    <p class="muted section-note">${t('settings.method.desc')}</p>
    <div class="box" role="radiogroup" aria-label=${t('settings.method')}>
      ${data.catalog.methods.map((method, index) =>
        choice(
          method.id,
          t('method.name', { n: index + 1 }),
          t(`method.${method.id}`),
          index === 0 && html`<span class="label label-success">${t('method.recommended')}</span>`,
        ),
      )}
      ${(data.settings.advanced || current === 'custom') && choice('custom', t('method.custom'), summary(data.settings.custom))}
    </div>
  </section>`;
}

// Özel DNS: adres ve bağlantı noktası. Kutudan çıkınca ya da Enter'a basınca kaydedilir.
function CustomDns() {
  const { data } = useStore();
  const { settings } = data;
  const [addr, setAddr] = useState(settings.dnsCustomAddr);
  useEffect(() => setAddr(settings.dnsCustomAddr), [settings.dnsCustomAddr]);

  const saveAddr = async () => {
    const value = addr.trim();
    if (value === settings.dnsCustomAddr) return;
    if (!/^\d{1,3}(\.\d{1,3}){3}$/.test(value) || value.split('.').some((part) => Number(part) > 255)) {
      toast(t('settings.dns.customInvalid'), { kind: 'error' });
      setAddr(settings.dnsCustomAddr);
      return;
    }
    await actions.set('dnsCustomAddr', value);
  };

  return html`<div class="row row-stack">
    <div class="dns-custom">
      <input
        class="input mono"
        type="text"
        inputmode="decimal"
        spellcheck=${false}
        aria-label=${t('settings.dns.customAddr')}
        placeholder="9.9.9.9"
        value=${addr}
        onInput=${(event) => setAddr(event.target.value)}
        onBlur=${saveAddr}
        onKeyDown=${(event) => event.key === 'Enter' && event.target.blur()}
      />
      <${NumberField}
        label=${t('settings.dns.customPort')}
        value=${settings.dnsCustomPort}
        min=${1}
        max=${65535}
        onCommit=${(value) => actions.set('dnsCustomPort', value)}
      />
    </div>
    <p class="muted" style=${{ marginTop: '8px', fontSize: '13px' }}>${t('settings.dns.customHint')}</p>
  </div>`;
}

function Dns() {
  const { data } = useStore();
  const { settings, catalog } = data;
  const selected = settings.scope === 'selected';
  return html`<section class="section">
    <div class="section-head"><h2 class="section-title">${t('settings.dns')}</h2></div>
    <div class="box">
      <${Row} title=${t('settings.dns.redirect')} desc=${selected && !settings.dnsAll ? t('settings.dns.redirect.selected') : t('settings.dns.redirect.all')}>
        <${Toggle} checked=${settings.dnsEnabled} label=${t('settings.dns.redirect')} onChange=${(value) => actions.set('dnsEnabled', value)} />
      <//>
      ${settings.dnsEnabled &&
      html`<${Row} title=${t('settings.dns.provider')} desc=${t('settings.dns.provider.desc')}>
          <select
            class="select"
            aria-label=${t('settings.dns.provider')}
            value=${settings.dnsProvider}
            onChange=${(event) => actions.set('dnsProvider', event.target.value)}
          >
            ${catalog.dns.map((dns) => html`<option value=${dns.id} key=${dns.id}>${dns.name} · ${dns.address}:${dns.port}</option>`)}
            <option value="custom">${t('settings.dns.custom')}</option>
          </select>
        <//>
        ${settings.dnsProvider === 'custom' && html`<${CustomDns} />`}
        ${settings.advanced &&
        selected &&
        html`<${Row} title=${t('settings.dns.all')} desc=${t('settings.dns.all.desc')}>
          <${Toggle} checked=${settings.dnsAll} label=${t('settings.dns.all')} onChange=${(value) => actions.set('dnsAll', value)} />
        <//>`}`}
    </div>
  </section>`;
}

// ---- gelişmiş: strateji, motor etkinliği ---------------------------------------------------------------------------

// Sahte paketin içindeki site adı. Kutudan çıkınca kaydedilir.
function FakeHost({ value }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const save = async () => {
    if (text.trim() === value) return;
    if ((await actions.setCustom('fakeHost', text.trim())) === undefined) setText(value);
  };
  return html`<input
    class="input mono field-host"
    type="text"
    spellcheck=${false}
    aria-label=${t('adv.fakeHost')}
    value=${text}
    onInput=${(event) => setText(event.target.value)}
    onBlur=${save}
    onKeyDown=${(event) => event.key === 'Enter' && event.target.blur()}
  />`;
}

function Strategy() {
  const { data } = useStore();
  const strategy = currentStrategy(data);
  const set = actions.setCustom;
  const toggle = (field, label) => html`<${Toggle} checked=${strategy[field]} label=${label} onChange=${(value) => set(field, value)} />`;
  const fakes = strategy.fakeTtlMode !== 'off' || strategy.fakeBadChecksum || strategy.fakeBadSeq;

  return html`<div class="box">
    <${Row} title=${t('adv.split')} desc=${t('adv.split.desc')}>
      <${NumberField} label=${t('adv.split')} value=${strategy.split} min=${0} max=${1200} onCommit=${(value) => set('split', value)} />
    <//>
    <${Row} title=${t('adv.splitAtSni')} desc=${t('adv.splitAtSni.desc')}>${toggle('splitAtSni', t('adv.splitAtSni'))}<//>
    <${Row} title=${t('adv.reverse')} desc=${t('adv.reverse.desc')}>${toggle('reverse', t('adv.reverse'))}<//>
    <${Row} title=${t('adv.httpSplit')} desc=${t('adv.httpSplit.desc')}>
      <${NumberField} label=${t('adv.httpSplit')} value=${strategy.httpSplit} min=${0} max=${1200} onCommit=${(value) => set('httpSplit', value)} />
    <//>
    <${Row} title=${t('adv.fakeTtl')} desc=${t('adv.fakeTtl.desc')}>
      <select class="select" aria-label=${t('adv.fakeTtl')} value=${strategy.fakeTtlMode} onChange=${(event) => set('fakeTtlMode', event.target.value)}>
        <option value="off">${t('adv.fakeTtl.off')}</option>
        <option value="fixed">${t('adv.fakeTtl.fixed')}</option>
        <option value="auto">${t('adv.fakeTtl.auto')}</option>
      </select>
      ${strategy.fakeTtlMode === 'fixed' &&
      html`<${NumberField} label=${t('adv.fakeTtl.hops')} value=${strategy.fakeTtl} min=${1} max=${64} onCommit=${(value) => set('fakeTtl', value)} />`}
    <//>
    <${Row} title=${t('adv.badChecksum')} desc=${t('adv.badChecksum.desc')}>${toggle('fakeBadChecksum', t('adv.badChecksum'))}<//>
    <${Row} title=${t('adv.badSeq')} desc=${t('adv.badSeq.desc')}>${toggle('fakeBadSeq', t('adv.badSeq'))}<//>
    ${fakes &&
    html`<${Row} title=${t('adv.fakeHost')} desc=${t('adv.fakeHost.desc')}><${FakeHost} value=${strategy.fakeHost} /><//>
      <${Row} title=${t('adv.repeats')} desc=${t('adv.repeats.desc')}>
        <${NumberField} label=${t('adv.repeats')} value=${strategy.fakeRepeats} min=${1} max=${10} onCommit=${(value) => set('fakeRepeats', value)} />
      <//>`}
    <${Row} title=${t('adv.blockQuic')} desc=${t('adv.blockQuic.desc')}>${toggle('blockQuic', t('adv.blockQuic'))}<//>
    <${Row} title=${t('adv.noSni')} desc=${t('adv.noSni.desc')}>
      <select class="select" aria-label=${t('adv.noSni')} value=${strategy.noSni} onChange=${(event) => set('noSni', event.target.value)}>
        <option value="known">${t('adv.noSni.known')}</option>
        <option value="skip">${t('adv.noSni.skip')}</option>
        <option value="always">${t('adv.noSni.always')}</option>
      </select>
    <//>
  </div>`;
}

// Motorun sayaçları: gerçekten çalıştığını ve neye uyguladığını gösterir.
function Activity() {
  const { data } = useStore();
  const stats = data.engine.stats;
  const cell = (value, label) => html`<div class="stat"><div class="stat-value mono">${value}</div><div class="stat-label muted">${label}</div></div>`;
  return html`<div class="box">
    ${stats
      ? html`<div class="stats">
            ${cell(stats.tls, t('adv.stat.tls'))} ${cell(stats.dns, t('adv.stat.dns'))} ${cell(stats.known, t('adv.stat.known'))}
            ${cell(stats.quic, t('adv.stat.quic'))}
          </div>
          ${stats.recent.length > 0 &&
          html`<div class="row row-stack">
            <div class="row-title">${t('adv.recent')}</div>
            <p class="muted mono selectable" style=${{ fontSize: '12px' }}>${list(stats.recent)}</p>
          </div>`}`
      : html`<div class="row"><p class="muted">${t('adv.activity.off')}</p></div>`}
    <${Row} title=${t('adv.log')} desc=${t('adv.log.desc')}>
      <button class="btn" disabled=${!data.engine.installed} onClick=${actions.showLog}>${t('adv.log.show')}</button>
    <//>
  </div>`;
}

function Advanced() {
  const { data } = useStore();
  const open = data.settings.advanced;
  return html`<section class="section">
    <div class="section-head"><h2 class="section-title">${t('adv.title')}</h2></div>
    <div class="box">
      <${Row} title=${t('adv.show')} desc=${t('adv.show.desc')}>
        <${Toggle} checked=${open} label=${t('adv.show')} onChange=${(value) => actions.set('advanced', value)} />
      <//>
    </div>
    ${open &&
    html`<h3 class="subsection-title">${t('adv.strategy')}</h3>
      <p class="muted section-note">${data.settings.method === 'custom' ? t('adv.strategy.custom') : t('adv.strategy.preset')}</p>
      <${Strategy} />
      <h3 class="subsection-title">${t('adv.activity')}</h3>
      <${Activity} />`}
  </section>`;
}

function AppSettings() {
  const { data } = useStore();
  const { settings } = data;
  return html`<section class="section">
    <div class="section-head"><h2 class="section-title">${t('settings.app')}</h2></div>
    <div class="box">
      <${Row} title=${t('settings.closeToTray')} desc=${t('settings.closeToTray.desc')}>
        <${Toggle} checked=${settings.closeToTray} label=${t('settings.closeToTray')} onChange=${(value) => actions.set('closeToTray', value)} />
      <//>
      <${Row} title=${t('settings.language')} desc=${t('settings.language.desc')}>
        <${LanguageSelect}
          label=${t('settings.language')}
          value=${settings.language}
          onChange=${(value) => actions.set('language', value)}
          options=${[
            { value: 'auto', label: t('settings.language.auto'), flag: 'auto' },
            { value: 'tr', label: t('lang.tr'), flag: 'tr' },
            { value: 'en', label: t('lang.en'), flag: 'en' },
          ]}
        />
      <//>
      <${Row} title=${t('settings.remove')} desc=${data.engine.installed ? t('settings.remove.desc') : t('settings.remove.none')}>
        <button class="btn btn-danger" disabled=${!data.engine.installed || Boolean(data.engine.busy)} onClick=${actions.removeAll}>
          <${Icon} name="trash" />${t('settings.remove.button')}
        </button>
      <//>
    </div>
  </section>`;
}

function About() {
  const { data } = useStore();
  const { links } = data;
  return html`<section class="section">
    <div class="section-head"><h2 class="section-title">${t('settings.about')}</h2></div>
    <div class="box about">
      <img src="assets/icon.png" width="56" height="56" alt="" />
      <div>
        <div class="about-name">Robz DPI <span class="mono muted">${data.version}</span></div>
        <p>${t('settings.about.body')}</p>
        <p class="muted">${t('settings.about.disclaimer')}</p>
        <ul class="about-links">
          <li><a href=${links.github}><${Icon} name="github" />${t('link.github')}</a></li>
          <li><a href=${links.discord}><${Icon} name="chat" />${t('link.discord')}</a></li>
        </ul>
      </div>
    </div>
  </section>`;
}

export function Settings() {
  return html`<div class="page">
    <${Methods} />
    <${Dns} />
    <${Advanced} />
    <${AppSettings} />
    <${About} />
  </div>`;
}
