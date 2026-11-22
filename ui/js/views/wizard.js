// İlk açılış: üç kısa, atlanamayan adım. Dil, sorumluluk reddinin kabulü ve nerede çalışacağı.
// Bitince ayarlardaki `onboarded` işaretlenir ve bir daha gösterilmez; bitmeden motor açılamaz.
import { html, useState } from '../../vendor/preact-htm.js';
import { Icon } from '../icons.js';
import { lang, languageOptions, list, t } from '../i18n.js';
import { useStore } from '../store.js';
import * as actions from '../actions.js';
import { Flag, Flash, Segmented, ServiceBadge, Toggle } from '../components.js';

const STEPS = 3;

function Points({ keys, icon = 'check' }) {
  return html`<ul class=${`wizard-points wizard-points-${icon}`}>
    ${keys.map((key) => html`<li key=${key}><${Icon} name=${icon} /><span>${t(key)}</span></li>`)}
  </ul>`;
}

function Welcome() {
  const current = lang();
  return html`<h1>${t('wizard.welcome.title')}</h1>
    <p class="wizard-lead">${t('wizard.welcome.body')}</p>
    <${Points} keys=${['wizard.welcome.point1', 'wizard.welcome.point2', 'wizard.welcome.point3']} />
    <h2 class="wizard-label">${t('wizard.welcome.language')}</h2>
    <div class="box" role="radiogroup" aria-label=${t('wizard.welcome.language')}>
      ${languageOptions().map(
        (option) => html`<button
          class="choice choice-center"
          role="radio"
          key=${option.value}
          aria-checked=${option.value === current}
          onClick=${() => option.value !== current && actions.set('language', option.value)}
        >
          <span class="choice-mark"></span>
          <${Flag} code=${option.flag} size=${24} />
          <span class="row-title">${option.label}</span>
        </button>`,
      )}
    </div>`;
}

function Terms({ accepted, onAccept }) {
  const { data } = useStore();
  return html`<h1>${t('wizard.terms.title')}</h1>
    <p class="wizard-lead">${t('wizard.terms.body')}</p>
    <${Points}
      icon="info"
      keys=${['wizard.terms.point1', 'wizard.terms.point2', 'wizard.terms.point3', 'wizard.terms.point4', 'wizard.terms.point5']}
    />
    <p class="wizard-link"><a href=${data.links.disclaimer}><${Icon} name="shield" size=${14} />${t('wizard.terms.full')}</a></p>
    <label class="check">
      <input type="checkbox" checked=${accepted} onChange=${(event) => onAccept(event.target.checked)} />
      <span>${t('wizard.terms.accept')}</span>
    </label>`;
}

function Scope() {
  const { data } = useStore();
  const { settings, catalog, conflicts } = data;
  return html`<h1>${t('wizard.scope.title')}</h1>
    <p class="wizard-lead">${t('wizard.scope.body')}</p>
    <div class="wizard-gap">
      <${Segmented}
        label=${t('wizard.scope.title')}
        value=${settings.scope}
        onChange=${(value) => actions.set('scope', value)}
        options=${[
          { value: 'selected', label: t('scope.selected') },
          { value: 'all', label: t('scope.all') },
        ]}
      />
    </div>
    ${settings.scope === 'all'
      ? html`<${Flash} kind="attention" icon="alert">${t('scope.all.desc')}<//>`
      : html`<div class="box wizard-gap">
          ${catalog.services.map(
            (service) => html`<div class="row" key=${service.id}>
              <${ServiceBadge} service=${service} />
              <div class="row-text"><div class="row-title">${service.name}</div></div>
              <div class="row-control">
                <${Toggle}
                  checked=${settings.services.includes(service.id)}
                  label=${service.name}
                  onChange=${(value) => actions.setService(service.id, value)}
                />
              </div>
            </div>`,
          )}
        </div>`}
    <${Flash} icon="shield">${t('wizard.scope.admin')}<//>
    ${conflicts.length > 0 &&
    html`<${Flash} kind="attention" icon="alert">
      ${t('wizard.scope.conflict', { name: list([...new Set(conflicts.map((c) => c.name))]) })}
    <//>`}`;
}

export function Wizard() {
  const { data } = useStore();
  // Önizleme ve ekran görüntüleri için başlangıç adımı adresle seçilebilir (?step=2); uygulamada hep 1'dir.
  const initial = Number(new URLSearchParams(location.search).get('step')) || 1;
  const [step, setStep] = useState(Math.min(Math.max(initial, 1), STEPS));
  const [accepted, setAccepted] = useState(false);

  // Son adımda hiçbir şey seçili değilse bitirilemez: uygulama boş bir listeyle açılmasın.
  const nothing = data.settings.scope === 'selected' && data.settings.services.length === 0 && data.settings.customDomains.length === 0;
  const blocked = (step === 2 && !accepted) || (step === STEPS && (nothing || !accepted));
  const next = () => (step < STEPS ? setStep(step + 1) : actions.set('onboarded', true));

  return html`<div class="wizard">
    <header class="wizard-head">
      <div class="brand">
        <img src="assets/icon.png" width="24" height="24" alt="" />
        <span>Robz DPI</span>
      </div>
      <div class="wizard-steps" role="img" aria-label=${t('wizard.step', { n: step, total: STEPS })}>
        ${Array.from({ length: STEPS }, (_, i) => html`<span class=${`wizard-dot ${i < step ? 'is-done' : ''}`}></span>`)}
      </div>
    </header>
    <main class="wizard-body">
      <div class="wizard-inner">
        ${step === 1 && html`<${Welcome} />`}
        ${step === 2 && html`<${Terms} accepted=${accepted} onAccept=${setAccepted} />`}
        ${step === 3 && html`<${Scope} />`}
      </div>
    </main>
    <footer class="wizard-foot">
      <span class="muted wizard-count">${t('wizard.step', { n: step, total: STEPS })}</span>
      <div class="wizard-actions">
        ${step > 1 && html`<button class="btn" onClick=${() => setStep(step - 1)}>${t('wizard.back')}</button>`}
        <button class="btn btn-primary" disabled=${blocked} onClick=${next}>
          ${step === STEPS ? t('wizard.finish') : step === 2 ? t('wizard.agree') : t('wizard.next')}
        </button>
      </div>
    </footer>
  </div>`;
}
