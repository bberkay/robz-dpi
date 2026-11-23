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
// Hazır uygulamaların logoları (Simple Icons, CC0). Discord ve Roblox kendi sahiplerinin ticari markalarıdır; logolar
// yalnız hangi hizmetin seçildiğini göstermek için kullanılır, proje onlarla bağlantılı değildir (DISCLAIMER.md).
const LOGOS = {
  discord: 'M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z',
  roblox: 'M18.926 23.998 0 18.892 5.075.002 24 5.108ZM15.348 10.09l-5.282-1.453-1.414 5.273 5.282 1.453z',
};

export function ServiceBadge({ service }) {
  const logo = LOGOS[service.id];
  return html`<span class=${`svc svc-${service.id}`} aria-hidden="true">
    ${logo
      ? html`<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d=${logo} /></svg>`
      : service.name[0]}
  </span>`;
}

// Kullanıcının eklediği sitenin rozeti: adının ilk iki harfi ("violabot.com" -> "VI"). Renk alan adından türetilir;
// aynı site her zaman aynı renkte görünür.
const DOMAIN_COLORS = ['#8957e5', '#1f6feb', '#238636', '#bf8700', '#bd561d', '#da3633', '#bf4b8a', '#0d9488'];

export function DomainBadge({ domain, muted = false }) {
  const name = domain.split('.')[0] || domain;
  const letters = name.replace(/[^a-z0-9]/gi, '').slice(0, 2).toUpperCase() || '?';
  let hash = 0;
  for (const char of domain) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  const color = DOMAIN_COLORS[hash % DOMAIN_COLORS.length];
  return html`<span class=${`svc svc-domain ${muted ? 'is-muted' : ''}`} style=${{ background: color }} aria-hidden="true">${letters}</span>`;
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
  if (code === 'ru') {
    return html`<svg class="flag" width=${size} height=${height} viewBox="0 0 30 20" aria-hidden="true">
      <rect width="30" height="20" fill="#fff" />
      <rect y="6.67" width="30" height="6.67" fill="#0039A6" />
      <rect y="13.33" width="30" height="6.67" fill="#D52B1E" />
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
