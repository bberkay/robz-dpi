// Sayfadaki canlı bilgiler (son sürüm, kurulum dosyasının boyutu, yıldız sayısı) GitHub'dan güncellenir; diff
// görününce oynatılır. GitHub'a ulaşılamazsa sayfa derleme anındaki bilgilerle kalır. Başka hiçbir yere bağlanmaz.
(() => {
  const REPO = 'kutsalkiremitci/robz-dpi';
  const SETUP = 'RobzDPI-Setup.exe';
  const locale = { tr: 'tr-TR', en: 'en-US', ru: 'ru-RU' }[document.documentElement.lang] || 'en-US';
  document.documentElement.classList.add('js');

  // ---- diff: görünür olunca bir kez oynar ----
  const diff = document.querySelector('.diff');
  if (diff && 'IntersectionObserver' in window) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          diff.classList.add('play');
          observer.disconnect();
        }
      },
      { threshold: 0.35 },
    );
    observer.observe(diff);
  } else if (diff) {
    diff.classList.add('play');
  }

  // ---- dil menüsü: dışarı tıklayınca ya da Esc ile kapanır ----
  const langMenu = document.querySelector('[data-lang-menu]');
  if (langMenu) {
    document.addEventListener('click', (event) => {
      if (langMenu.open && !langMenu.contains(event.target)) langMenu.open = false;
    });
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && langMenu.open) {
        langMenu.open = false;
        langMenu.querySelector('summary').focus();
      }
    });
  }

  // ---- GitHub ----
  const cacheKey = 'robzdpi:gh:v1';
  async function api(path) {
    let cache = {};
    try {
      cache = JSON.parse(sessionStorage.getItem(cacheKey) || '{}');
    } catch {}
    if (cache[path] && Date.now() - cache[path].t < 10 * 60 * 1000) return cache[path].v;
    const res = await fetch(`https://api.github.com/${path}`, { headers: { Accept: 'application/vnd.github+json' } });
    if (!res.ok) throw new Error(String(res.status));
    const value = await res.json();
    try {
      cache[path] = { t: Date.now(), v: value };
      sessionStorage.setItem(cacheKey, JSON.stringify(cache));
    } catch {}
    return value;
  }
  const number = (n) => new Intl.NumberFormat(locale).format(n);
  const megabytes = (bytes) => `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / 1048576)} MB`;
  const set = (selector, text) =>
    document.querySelectorAll(selector).forEach((node) => {
      node.textContent = text;
      node.hidden = false;
    });

  api(`repos/${REPO}`)
    .then((repo) => set('[data-stars]', number(repo.stargazers_count)))
    .catch(() => {});

  api(`repos/${REPO}/releases/latest`)
    .then((release) => {
      set('[data-version]', release.tag_name.startsWith('v') ? release.tag_name : `v${release.tag_name}`);
      document.querySelectorAll('[data-release-link]').forEach((link) => (link.href = release.html_url));
      const setup = release.assets.find((asset) => asset.name === SETUP);
      if (setup) set('[data-size]', megabytes(setup.size));
    })
    .catch(() => {});
})();
