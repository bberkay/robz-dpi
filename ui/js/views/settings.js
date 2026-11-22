// Ayarlar: yöntem, DNS, uygulama davranışı, motoru kaldırma ve hakkında.
import { html, useEffect, useState } from '../../vendor/preact-htm.js';
import { Icon } from '../icons.js';
import { t } from '../i18n.js';
import { toast, useStore } from '../store.js';
import * as actions from '../actions.js';
import { LanguageSelect, Toggle } from '../components.js';

function Row({ title, desc, children }) {
  return html`<div class="row">
    <div class="row-text">
      <div class="row-title">${title}</div>
      ${desc && html`<p class="muted">${desc}</p>`}
    </div>
    <div class="row-control">${children}</div>
  </div>`;
}

function Methods() {
  const { data } = useStore();
  const current = data.settings.method;
  return html`<section class="section" style=${{ marginTop: 0 }}>
    <div class="section-head"><h2 class="section-title">${t('settings.method')}</h2></div>
    <p class="muted section-note">${t('settings.method.desc')}</p>
    <div class="box" role="radiogroup" aria-label=${t('settings.method')}>
      ${data.catalog.methods.map(
        (method, index) => html`<button
          class="choice"
          role="radio"
          key=${method.id}
          aria-checked=${method.id === current}
          onClick=${() => method.id !== current && actions.set('method', method.id)}
        >
          <span class="choice-mark"></span>
          <span class="row-text">
            <span class="row-title">
              ${t('method.name', { n: index + 1 })}
              ${index === 0 && html`<span class="label label-success">${t('method.recommended')}</span>`}
              <span class="choice-flags">${method.flags}</span>
            </span>
            <p class="muted">${t(`method.${method.id}`)}</p>
          </span>
        </button>`,
      )}
    </div>
  </section>`;
}

// Özel DNS: adres ve bağlantı noktası. Kutudan çıkınca ya da Enter'a basınca kaydedilir.
function CustomDns() {
  const { data } = useStore();
  const { settings } = data;
  const [addr, setAddr] = useState(settings.dnsCustomAddr);
  const [port, setPort] = useState(String(settings.dnsCustomPort));
  useEffect(() => setAddr(settings.dnsCustomAddr), [settings.dnsCustomAddr]);
  useEffect(() => setPort(String(settings.dnsCustomPort)), [settings.dnsCustomPort]);

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
  const savePort = async () => {
    const value = Number(port);
    if (value === settings.dnsCustomPort) return;
    if (!Number.isInteger(value) || value < 1 || value > 65535) {
      setPort(String(settings.dnsCustomPort));
      return;
    }
    await actions.set('dnsCustomPort', value);
  };
  const onEnter = (event) => event.key === 'Enter' && event.target.blur();

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
        onKeyDown=${onEnter}
      />
      <input
        class="input mono"
        type="text"
        inputmode="numeric"
        aria-label=${t('settings.dns.customPort')}
        placeholder="53"
        value=${port}
        onInput=${(event) => setPort(event.target.value.replace(/\D/g, '').slice(0, 5))}
        onBlur=${savePort}
        onKeyDown=${onEnter}
      />
    </div>
    <p class="muted" style=${{ marginTop: '8px', fontSize: '13px' }}>${t('settings.dns.customHint')}</p>
  </div>`;
}

function Dns() {
  const { data } = useStore();
  const { settings, catalog } = data;
  return html`<section class="section">
    <div class="section-head"><h2 class="section-title">${t('settings.dns')}</h2></div>
    <div class="box">
      <${Row} title=${t('settings.dns.redirect')} desc=${t('settings.dns.redirect.desc')}>
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
        ${settings.dnsProvider === 'custom' && html`<${CustomDns} />`}`}
    </div>
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
          <li><a href=${links.kick}><${Icon} name="live" />${t('link.kick')}</a></li>
        </ul>
      </div>
    </div>
  </section>`;
}

export function Settings() {
  return html`<div class="page">
    <${Methods} />
    <${Dns} />
    <${AppSettings} />
    <${About} />
  </div>`;
}
