// Arayüzün girişi: Rust çekirdeğinden durumu alır, kabuğu (üst çubuk, sekmeler, sayfa) çizer.
import './bridge.js';
import { html, render, useEffect, useRef } from '../vendor/preact-htm.js';
import { Icon } from './icons.js';
import { lang, t } from './i18n.js';
import { closeDialog, dismissToast, showTab, update, useStore } from './store.js';
import * as actions from './actions.js';
import { LanguageSelect, Spinner } from './components.js';
import { Home } from './views/home.js';
import { Settings } from './views/settings.js';

function Topbar() {
  const { data } = useStore();
  return html`<header class="topbar">
    <div class="brand">
      <img src="assets/icon.png" width="24" height="24" alt="" />
      <span>Robz DPI</span>
    </div>
    <div class="topbar-actions">
      <${LanguageSelect}
        compact
        label=${t('settings.language')}
        value=${lang()}
        onChange=${(value) => actions.set('language', value)}
        options=${[
          { value: 'tr', label: t('lang.tr'), flag: 'tr' },
          { value: 'en', label: t('lang.en'), flag: 'en' },
        ]}
      />
      <a class="icon-link" href=${data.links.github} title=${t('link.github')} aria-label=${t('link.github')}>
        <${Icon} name="github" />
      </a>
    </div>
  </header>`;
}

function Tabs() {
  const { tab } = useStore();
  const item = (id, icon) => html`<button class="tab" role="tab" aria-selected=${tab === id} onClick=${() => showTab(id)}>
    <${Icon} name=${icon} /><span>${t(`tab.${id}`)}</span>
  </button>`;
  return html`<nav class="tabs" role="tablist">${item('home', 'home')}${item('settings', 'sliders')}</nav>`;
}

function ConfirmDialog() {
  const { dialog } = useStore();
  const ref = useRef(null);
  useEffect(() => {
    if (dialog && ref.current && !ref.current.open) ref.current.showModal();
  }, [dialog]);
  if (!dialog) return null;
  const confirm = () => {
    closeDialog();
    dialog.onConfirm();
  };
  return html`<dialog class="dialog" ref=${ref} onClose=${closeDialog} onCancel=${closeDialog}>
    <h2>${dialog.title}</h2>
    <p class="muted">${dialog.body}</p>
    <div class="dialog-actions">
      <button class="btn" onClick=${closeDialog}>${t('dialog.cancel')}</button>
      <button class=${`btn ${dialog.danger ? 'btn-danger-solid' : 'btn-primary'}`} onClick=${confirm}>${dialog.confirm}</button>
    </div>
  </dialog>`;
}

function Toasts() {
  const { toasts } = useStore();
  return html`<div class="toasts" aria-live="polite">
    ${toasts.map(
      (item) => html`<div class=${`toast toast-${item.kind}`} key=${item.id}>
        <${Icon} name=${item.kind === 'error' ? 'alert' : item.kind === 'success' ? 'check' : 'info'} />
        <span class="toast-text">${item.message}</span>
        <button class="btn btn-icon btn-invisible btn-sm" aria-label=${t('action.dismiss')} onClick=${() => dismissToast(item.id)}>
          <${Icon} name="x" size=${14} />
        </button>
      </div>`,
    )}
  </div>`;
}

function App() {
  const { data, tab } = useStore();
  useEffect(() => {
    if (data) document.documentElement.lang = lang();
  });
  if (!data) return html`<div class="loading"><${Spinner} size=${20} /></div>`;
  return html`<div class="app">
    <${Topbar} />
    <${Tabs} />
    <main class="content">${tab === 'settings' ? html`<${Settings} />` : html`<${Home} />`}</main>
    <${Toasts} />
    <${ConfirmDialog} />
  </div>`;
}

window.dpi.on('state', (data) => update({ data }));

// Dış bağlantılar varsayılan tarayıcıda açılır; "#" bağlantıları sayfa içi eylemdir.
document.addEventListener('click', (event) => {
  const link = event.target.closest?.('a[href]');
  if (!link) return;
  event.preventDefault();
  const href = link.getAttribute('href');
  if (/^https:\/\//i.test(href)) actions.openExternal(href);
});

// Tarayıcının sağ tık menüsü (Geri, Yenile, İncele) yalnız seçilebilir metinde ve yazı kutularında açılır.
document.addEventListener('contextmenu', (event) => {
  if (!event.target.closest?.('.selectable, input, textarea')) event.preventDefault();
});

render(html`<${App} />`, document.getElementById('root'));
window.dpi.call('getState').then((result) => {
  if (result.ok) update({ data: result.value });
});
