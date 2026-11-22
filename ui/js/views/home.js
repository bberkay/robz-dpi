// Genel: aç/kapa düğmesi, nerede çalışacağı (uygulamalar ve siteler) ve bağlantı testi.
import { html, useState } from '../../vendor/preact-htm.js';
import { Icon } from '../icons.js';
import { list, t } from '../i18n.js';
import { showTab, useStore } from '../store.js';
import * as actions from '../actions.js';
import { Flash, Segmented, ServiceBadge, Spinner, Toggle } from '../components.js';

// "Discord, Roblox ve 2 site": seçili uygulamaların ve eklenen sitelerin kısa özeti.
function scopeList(data) {
  const { settings, catalog } = data;
  const names = catalog.services.filter((s) => settings.services.includes(s.id)).map((s) => s.name);
  const custom = settings.customDomains;
  if (custom.length === 1) names.push(custom[0]);
  else if (custom.length > 1) names.push(t('scope.sites', { n: custom.length }));
  return list(names);
}

function PowerCard() {
  const { data } = useStore();
  const { engine, settings } = data;
  const on = engine.state === 'running';
  const busy = engine.busy || (engine.state === 'starting' || engine.state === 'stopping' ? engine.state : null);
  const usable = data.supported && data.engineReady;
  const blocked = !usable || (!on && Boolean(engine.problem));
  const all = settings.scope === 'all';

  let subtitle = '';
  if (on) subtitle = all ? t('power.onScope.all') : t('power.onScope.selected', { list: scopeList(data) });
  else if (!engine.problem) subtitle = all ? t('power.offScope.all') : t('power.offScope.selected', { list: scopeList(data) });

  return html`<section class=${`power ${on && !busy ? 'is-on' : ''}`}>
    <button
      class=${`power-button ${busy ? 'is-working' : ''}`}
      role="switch"
      aria-checked=${on}
      aria-label=${on ? t('power.turnOff') : t('power.turnOn')}
      title=${on ? t('power.turnOff') : t('power.turnOn')}
      disabled=${Boolean(busy) || blocked}
      onClick=${actions.toggle}
    >
      ${busy ? html`<${Spinner} size=${28} />` : html`<${Icon} name="power" size=${34} />`}
    </button>
    <div class="power-text">
      <div class="power-state" aria-live="polite">${busy ? t(`power.${busy}`) : on ? t('power.on') : t('power.off')}</div>
      ${subtitle && html`<p class="power-sub">${subtitle}</p>`}
      ${on &&
      !busy &&
      engine.stats &&
      html`<p class="power-stats muted">${t('power.stats', { tls: engine.stats.tls, dns: engine.stats.dns })}</p>`}
      ${!on &&
      !busy &&
      !blocked &&
      engine.needsAdmin &&
      html`<p class="power-admin muted"><${Icon} name="shield" size=${14} />${t('power.admin')}</p>`}
    </div>
  </section>`;
}

// Düğmenin altındaki uyarılar: desteklenmeyen sistem, hata, çakışan araç, uygulanmamış ayar.
function Notices() {
  const { data } = useStore();
  const { engine, conflicts } = data;
  const busy = Boolean(engine.busy);
  const notices = [];

  if (!data.supported) {
    notices.push(html`<${Flash} kind="attention" icon="alert">${t('notice.unsupported')}<//>`);
  } else if (!data.engineReady) {
    notices.push(html`<${Flash} kind="danger" icon="alert">${t('error.engine-missing')}<//>`);
  }

  if (engine.error) {
    // Motorun çıkış kodu bilinen bir nedense onu söyle; değilse ayrıntıyı olduğu gibi göster.
    const detail = engine.error.detail;
    const hintKey = `engineCode.${detail}`;
    const hint = /^[0-9]+$/.test(detail || '') && t(hintKey) !== hintKey ? t(hintKey) : null;
    notices.push(html`<${Flash}
      kind="danger"
      icon="alert"
      detail=${hint || (detail && t('error.detail', { detail }))}
      actions=${html`<button class="btn btn-sm" onClick=${actions.dismissError}>${t('action.dismiss')}</button>`}
    >
      ${actions.errorText(engine.error)}
    <//>`);
  }

  if (conflicts.length) {
    const fixable = conflicts.some((c) => c.name === 'goodbyedpi.exe');
    const names = list([...new Set(conflicts.map((c) => c.name))]);
    notices.push(html`<${Flash}
      kind="attention"
      icon="alert"
      actions=${fixable &&
      html`<button class="btn btn-sm" disabled=${busy} onClick=${actions.stopConflicts}>${t('notice.conflict.fix')}</button>`}
    >
      ${t('notice.conflict', { name: names })} ${!fixable && t('notice.conflict.manual')}
    <//>`);
  }

  if (engine.problem) {
    notices.push(html`<${Flash} kind="attention" icon="info">${t(`problem.${engine.problem}`)}<//>`);
  } else if (engine.pending) {
    notices.push(html`<${Flash}
      kind="accent"
      icon="info"
      actions=${html`<button class="btn btn-sm btn-primary" disabled=${busy} onClick=${actions.apply}>
        <${Icon} name="shield" size=${14} />${t('action.apply')}
      </button>`}
    >
      ${t('notice.pending')}
    <//>`);
  }

  return notices;
}

function AlwaysOn() {
  const { data } = useStore();
  return html`<div class="box" style=${{ marginTop: '12px' }}>
    <div class="row">
      <div class="row-text">
        <div class="row-title">${t('always.title')}</div>
        <p class="muted">${data.packaged ? t('always.desc') : t('always.dev')}</p>
      </div>
      <div class="row-control">
        <${Toggle}
          checked=${data.settings.launchAtLogin}
          label=${t('always.title')}
          disabled=${!data.packaged}
          onChange=${(value) => actions.set('launchAtLogin', value)}
        />
      </div>
    </div>
  </div>`;
}

function CustomDomains() {
  const { data } = useStore();
  const [text, setText] = useState('');
  const domains = data.settings.customDomains;

  const submit = async (event) => {
    event.preventDefault();
    if (await actions.addDomains(text)) setText('');
  };

  return html`<div class="box">
    <div class="row row-stack">
      <div class="row-text">
        <div class="row-title">${t('custom.title')}</div>
        <p class="muted">${t('custom.desc')}</p>
      </div>
      <form class="domain-form" onSubmit=${submit}>
        <input
          class="input"
          type="text"
          spellcheck=${false}
          autocomplete="off"
          aria-label=${t('custom.title')}
          placeholder=${t('custom.placeholder')}
          value=${text}
          onInput=${(event) => setText(event.target.value)}
        />
        <button class="btn" type="submit" disabled=${!text.trim()}><${Icon} name="plus" />${t('custom.add')}</button>
      </form>
      ${domains.length > 0 &&
      html`<ul class="chips">
        ${domains.map(
          (domain) => html`<li class="chip" key=${domain}>
            <span>${domain}</span>
            <button aria-label=${t('custom.remove', { domain })} title=${t('custom.remove', { domain })} onClick=${() => actions.removeDomain(domain)}>
              <${Icon} name="x" size=${12} />
            </button>
          </li>`,
        )}
      </ul>`}
    </div>
  </div>`;
}

function Scope() {
  const { data } = useStore();
  const { settings, catalog } = data;

  return html`<section class="section">
    <div class="section-head"><h2 class="section-title">${t('scope.title')}</h2></div>
    <${Segmented}
      label=${t('scope.title')}
      value=${settings.scope}
      onChange=${(value) => actions.set('scope', value)}
      options=${[
        { value: 'selected', label: t('scope.selected') },
        { value: 'all', label: t('scope.all') },
      ]}
    />
    ${settings.scope === 'all'
      ? html`<${Flash} kind="attention" icon="alert">${t('scope.all.desc')}<//>`
      : html`<p class="muted section-note" style=${{ margin: '10px 0 12px' }}>${t('scope.selected.desc')}</p>
          <div class="box">
            ${catalog.services.map(
              (service) => html`<div class="row" key=${service.id}>
                <${ServiceBadge} service=${service} />
                <div class="row-text">
                  <div class="row-title">${service.name}</div>
                  <p class="muted">${t('service.domains', { n: service.domains.length })}</p>
                </div>
                <div class="row-control">
                  <${Toggle}
                    checked=${settings.services.includes(service.id)}
                    label=${service.name}
                    onChange=${(value) => actions.setService(service.id, value)}
                  />
                </div>
              </div>`,
            )}
          </div>
          <${CustomDomains} />`}
  </section>`;
}

function Probe() {
  const { data } = useStore();
  const { probe, engine } = data;
  const failed = probe.results.some((r) => !r.ok);

  return html`<section class="section">
    <div class="section-head">
      <h2 class="section-title">${t('test.title')}</h2>
      <button class="btn btn-sm" disabled=${probe.running} onClick=${actions.test}>
        ${probe.running ? html`<${Spinner} size=${14} />` : html`<${Icon} name="pulse" size=${14} />`}
        ${probe.running ? t('test.running') : t('test.run')}
      </button>
    </div>
    ${probe.results.length === 0
      ? html`<p class="muted section-note">${t('test.hint')}</p>`
      : html`<div class="box">
            ${probe.results.map(
              (result) => html`<div class="probe" key=${result.id}>
                <span class="probe-name">${result.label}</span>
                ${result.ok
                  ? html`<span class="probe-result probe-ok">
                      <${Icon} name="check" size=${14} />${t('test.ok')}
                      <span class="probe-ms mono">${t('test.ms', { ms: result.ms })}</span>
                    </span>`
                  : html`<span class="probe-result probe-fail"><${Icon} name="x" size=${14} />${t('test.fail')}</span>`}
              </div>`,
            )}
          </div>
          ${failed &&
          html`<p class="muted section-note" style=${{ marginTop: '10px' }}>
            ${engine.state === 'running' ? t('test.adviceOn') : t('test.adviceOff')}${' '}
            ${engine.state === 'running' && html`<a href="#" onClick=${() => showTab('settings')}>${t('test.openSettings')}</a>`}
          </p>`}`}
  </section>`;
}

export function Home() {
  return html`<div class="page">
    <${PowerCard} />
    <${Notices} />
    <${AlwaysOn} />
    <${Scope} />
    <${Probe} />
  </div>`;
}
