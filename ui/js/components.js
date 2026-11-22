// Sayfaların paylaştığı parçalar: anahtar, iki seçenekli düğme, dil seçimi, bekleme halkası, uyarı kutusu.
import { html, useEffect, useRef, useState } from '../vendor/preact-htm.js';
import { Icon } from './icons.js';

export function Toggle({ checked, onChange, label, disabled = false }) {
  return html`<button
    class="toggle"
    role="switch"
    aria-checked=${checked}
    aria-label=${label}
    disabled=${disabled}
    onClick=${() => onChange(!checked)}
  >
    <span class="toggle-knob"></span>
  </button>`;
}

export function Spinner({ size = 16 }) {
  return html`<span class="spinner" style=${{ width: size, height: size }} aria-hidden="true"></span>`;
}

// options: [{ value, label }]
export function Segmented({ value, options, onChange, label }) {
  return html`<div class="segmented" role="radiogroup" aria-label=${label}>
    ${options.map(
      (option) => html`<button
        class="segmented-item"
        role="radio"
        aria-checked=${option.value === value}
        onClick=${() => option.value !== value && onChange(option.value)}
      >
        ${option.label}
      </button>`,
    )}
  </div>`;
}

// Sayı kutusu: kutudan çıkınca ya da Enter'a basınca, değer [min, max] aralığındaysa kaydedilir; değilse eski
// değere döner.
export function NumberField({ value, min, max, onCommit, label }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const number = Number(text);
    if (text === '' || !Number.isInteger(number) || number < min || number > max) {
      setText(String(value));
      return;
    }
    if (number !== value) onCommit(number);
  };
  return html`<input
    class="input mono field-number"
    type="text"
    inputmode="numeric"
    aria-label=${label}
    title=${`${min} – ${max}`}
    value=${text}
    onInput=${(event) => setText(event.target.value.replace(/[^0-9]/g, '').slice(0, 5))}
    onBlur=${commit}
    onKeyDown=${(event) => event.key === 'Enter' && event.target.blur()}
  />`;
}

// kind: 'danger' | 'attention' | 'accent' | ''. actions: düğmeler.
export function Flash({ kind = '', icon = 'info', children, detail, actions }) {
  return html`<div class=${`flash ${kind ? `flash-${kind}` : ''}`} role=${kind === 'danger' ? 'alert' : 'status'}>
    <${Icon} name=${icon} />
    <div class="flash-body">
      <div>${children}</div>
      ${detail && html`<div class="flash-detail mono selectable">${detail}</div>`}
    </div>
    ${actions && html`<div class="flash-actions">${actions}</div>`}
  </div>`;
}

// Hazır uygulamanın rozeti: adının baş harfi, uygulamanın kendi renginde.
export function ServiceBadge({ service }) {
  return html`<span class=${`svc svc-${service.id}`} aria-hidden="true">${service.name[0]}</span>`;
}

// ---- dil seçimi: bayraklı açılır kutu -----------------------------------------------------------------------------

// Türk bayrağının yıldızı: merkezi (16.1, 10), bir ucu sola (hilale) bakar.
const STAR = Array.from({ length: 10 }, (_, k) => {
  const angle = Math.PI + (k * Math.PI) / 5;
  const radius = k % 2 ? 0.96 : 2.5;
  return `${(16.1 + radius * Math.cos(angle)).toFixed(2)},${(10 + radius * Math.sin(angle)).toFixed(2)}`;
}).join(' ');

// Windows bayrak emojilerini bayrak olarak çizmez ("TR", "GB" harfleri görünür); bayraklar bu yüzden SVG.
export function Flag({ code, size = 20 }) {
  const height = Math.round((size * 2) / 3);
  if (code === 'tr') {
    return html`<svg class="flag" width=${size} height=${height} viewBox="0 0 30 20" aria-hidden="true">
      <rect width="30" height="20" fill="#E30A17" />
      <circle cx="10.5" cy="10" r="5" fill="#fff" />
      <circle cx="11.75" cy="10" r="4" fill="#E30A17" />
      <polygon points=${STAR} fill="#fff" />
    </svg>`;
  }
  if (code === 'en') {
    return html`<svg class="flag" width=${size} height=${height} viewBox="0 0 45 30" aria-hidden="true">
      <rect width="45" height="30" fill="#012169" />
      <path d="M0,0 L45,30 M45,0 L0,30" stroke="#fff" stroke-width="6" />
      <path d="M0,0 L45,30 M45,0 L0,30" stroke="#C8102E" stroke-width="2.2" />
      <path d="M22.5,0 V30 M0,15 H45" stroke="#fff" stroke-width="9" />
      <path d="M22.5,0 V30 M0,15 H45" stroke="#C8102E" stroke-width="5.4" />
    </svg>`;
  }
  return html`<span class="flag flag-auto" style=${{ width: size, height }}><${Icon} name="globe" size=${height} /></span>`;
}

// Seçim kutusu: düğmede seçili dilin bayrağı ve adı; açılan listede bir seçeneğe tıklamak dili hemen değiştirir.
// options: [{ value, label, flag }]. compact: düğmede yalnız bayrak görünür (üst çubuk).
export function LanguageSelect({ value, options, onChange, label, compact = false }) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const ref = useRef(null);
  const current = options.find((o) => o.value === value) || options[0];

  useEffect(() => {
    if (!open) return undefined;
    setActive(Math.max(0, options.findIndex((o) => o.value === value)));
    const onDown = (event) => {
      if (!ref.current?.contains(event.target)) setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [open]);

  const choose = (option) => {
    setOpen(false);
    if (option.value !== value) onChange(option.value);
  };
  const onKeyDown = (event) => {
    if (!open && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
      event.preventDefault();
      setOpen(true);
    } else if (open && event.key === 'ArrowDown') {
      event.preventDefault();
      setActive((i) => (i + 1) % options.length);
    } else if (open && event.key === 'ArrowUp') {
      event.preventDefault();
      setActive((i) => (i - 1 + options.length) % options.length);
    } else if (open && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      choose(options[active]);
    } else if (event.key === 'Escape') {
      setOpen(false);
    }
  };

  return html`<div class="lang-select" ref=${ref}>
    <button
      class="lang-select-button"
      aria-haspopup="listbox"
      aria-expanded=${open}
      aria-label=${`${label}: ${current.label}`}
      onClick=${() => setOpen(!open)}
      onKeyDown=${onKeyDown}
    >
      <${Flag} code=${current.flag} />
      ${!compact && html`<span>${current.label}</span>`}
      <${Icon} name="chevron-down" size=${14} class="lang-select-chevron" />
    </button>
    ${open &&
    html`<ul class="lang-select-list" role="listbox" aria-label=${label}>
      ${options.map(
        (option, i) => html`<li
          role="option"
          aria-selected=${option.value === value}
          class=${`lang-select-option ${i === active ? 'is-active' : ''}`}
          onMouseEnter=${() => setActive(i)}
          onClick=${() => choose(option)}
        >
          <${Flag} code=${option.flag} />
          <span>${option.label}</span>
          ${option.value === value && html`<${Icon} name="check" size=${14} class="lang-select-check" />`}
        </li>`,
      )}
    </ul>`}
  </div>`;
}
