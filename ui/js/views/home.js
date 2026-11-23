// Genel: aç/kapa düğmesi, nerede çalışacağı (uygulamalar ve siteler) ve bağlantı testi.
import { html, useState } from '../../vendor/preact-htm.js';
import { Icon } from '../icons.js';
import { list, t } from '../i18n.js';
import { showTab, useStore } from '../store.js';
import * as actions from '../actions.js';
import { DomainBadge, Flash, Segmented, ServiceBadge, Spinner, Toggle } from '../components.js';

// Bir site listesinin kısa adı: tek siteyse adı, birden çoksa "3 site".
function sitesText(domains) {
  return domains.length === 1 ? domains[0] : t('scope.sites', { n: domains.length });
}

// "Discord, Roblox ve 2 site": seçili uygulamaların ve anahtarı açık eklenen sitelerin kısa özeti.
function scopeList(data) {
  const { settings, catalog } = data;
  const names = catalog.services.filter((s) => settings.services.includes(s.id)).map((s) => s.name);
  const custom = settings.customDomains.filter((d) => !settings.disabledDomains.includes(d));
  if (custom.length) names.push(sitesText(custom));
  return list(names);
}

// "Bütün sitelerde" kipinin özeti: hariç tutulan varsa onu da söyler.
function allText(data, on) {
  const excluded = data.settings.excludedDomains;
  if (!excluded.length) return on ? t('power.onScope.all') : t('power.offScope.all');
  return t(on ? 'power.onScope.allExcept' : 'power.offScope.allExcept', { list: sitesText(excluded) });
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
  if (on) subtitle = all ? allText(data, true) : t('power.onScope.selected', { list: scopeList(data) });
  else if (!engine.problem) subtitle = all ? allText(data, false) : t('power.offScope.selected', { list: scopeList(data) });

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

// Adres ekleme satırı: Discord ve Roblox'un altındaki sitelere ya da hariç tutulanlara.
function DomainForm({ title, desc, placeholder, onAdd }) {
  const [text, setText] = useState('');
  const submit = async (event) => {
    event.preventDefault();
    if (await onAdd(text)) setText('');
  };
  return html`<div class="row row-stack">
    <div class="row-text">
      <div class="row-title">${title}</div>
      <p class="muted">${desc}</p>
    </div>
    <form class="domain-form" onSubmit=${submit}>
      <input
        class="input"
        type="text"
        spellcheck=${false}
        autocomplete="off"
        aria-label=${title}
        placeholder=${placeholder}
        value=${text}
        onInput=${(event) => setText(event.target.value)}
      />
      <button class="btn" type="submit" disabled=${!text.trim()}><${Icon} name="plus" />${t('custom.add')}</button>
    </form>
  </div>`;
}

function RemoveButton({ label, onClick }) {
  return html`<button class="icon-btn" aria-label=${label} title=${label} onClick=${onClick}><${Icon} name="trash" size=${15} /></button>`;
}

// "Seçtiklerimde": hazır uygulamalar ve eklenen siteler aynı listede, her biri kendi anahtarıyla.
function SelectedList() {
  const { data } = useStore();
  const { settings, catalog } = data;
  return html`<div class="box">
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
    ${settings.customDomains.map((domain) => {
      const on = !settings.disabledDomains.includes(domain);
      return html`<div class="row" key=${domain}>
        <${DomainBadge} domain=${domain} muted=${!on} />
        <div class="row-text">
          <div class="row-title mono-title">${domain}</div>
          <p class="muted">${t('custom.row')}</p>
        </div>
        <div class="row-control">
          <${RemoveButton} label=${t('custom.remove', { domain })} onClick=${() => actions.removeDomain(domain)} />
          <${Toggle} checked=${on} label=${domain} onChange=${(value) => actions.setDomain(domain, value)} />
        </div>
      </div>`;
    })}
    <${DomainForm}
      title=${t('custom.title')}
      desc=${t('custom.desc')}
      placeholder=${t('custom.placeholder')}
      onAdd=${actions.addDomains}
    />
  </div>`;
}

// "Bütün sitelerde": dokunulmayacak siteler.
function ExcludedList() {
  const { data } = useStore();
  const excluded = data.settings.excludedDomains;
  return html`<div class="box">
    ${excluded.map(
      (domain) => html`<div class="row" key=${domain}>
        <${DomainBadge} domain=${domain} />
        <div class="row-text">
          <div class="row-title mono-title">${domain}</div>
          <p class="muted">${t('exclude.row')}</p>
        </div>
        <div class="row-control">
          <${RemoveButton} label=${t('exclude.remove', { domain })} onClick=${() => actions.removeExcluded(domain)} />
        </div>
      </div>`,
    )}
    <${DomainForm}
      title=${t('exclude.title')}
      desc=${t('exclude.desc')}
      placeholder=${t('custom.placeholder')}
      onAdd=${actions.addExcluded}
    />
  </div>`;
}

function Scope() {
  const { data } = useStore();
  const { settings } = data;

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
      ? html`<${Flash} kind="attention" icon="alert">${t('scope.all.desc')}<//>
          <${ExcludedList} />`
      : html`<p class="muted section-note" style=${{ margin: '10px 0 12px' }}>${t('scope.selected.desc')}</p>
          <${SelectedList} />`}
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
