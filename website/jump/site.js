// JumpNDodge – mert.win/jump: Sprache, passender Download fürs System, aktuelle Version.
(() => {
  const REPO = 'erqf1/JumpNDodge';
  const DL = (f) => `https://github.com/${REPO}/releases/latest/download/${f}`;

  // ---- Sprache (Deutsch / English), gemerkt im Browser
  const html = document.documentElement;
  let lang = 'de';
  try { lang = localStorage.getItem('jnd-lang') || ((navigator.language || 'de').startsWith('de') ? 'de' : 'en'); } catch { /* privat */ }
  const setLang = (l) => {
    lang = l; html.lang = l;
    document.querySelectorAll('.lang').forEach((b) => { b.textContent = l === 'de' ? 'English' : 'Deutsch'; });
    try { localStorage.setItem('jnd-lang', l); } catch { /* egal */ }
    pickDownload();
  };
  document.querySelectorAll('.lang').forEach((b) => b.addEventListener('click', () => setLang(lang === 'de' ? 'en' : 'de')));

  // ---- System erkennen -> Hauptknopf
  function detect() {
    const ua = navigator.userAgent || '';
    const p = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';
    if (/Win/i.test(p) || /Windows/i.test(ua)) return 'win';
    if (/Mac/i.test(p) || /Mac OS X/i.test(ua)) return /arm|aarch/i.test(ua) ? 'mac-arm' : 'mac';
    if (/Linux|X11/i.test(p + ua) && !/Android/i.test(ua)) return 'linux';
    return 'other';
  }
  const OPTIONS = {
    win: { file: 'JumpNDodge-windows-x64-setup.exe', de: 'Für Windows laden', en: 'Download for Windows', sub: { de: 'Installer · 64 Bit', en: 'Installer · 64-bit' } },
    mac: { file: 'JumpNDodge-macos-apple-silicon.dmg', de: 'Für macOS laden', en: 'Download for macOS', sub: { de: 'Apple Silicon · Intel unten', en: 'Apple Silicon · Intel below' } },
    'mac-arm': { file: 'JumpNDodge-macos-apple-silicon.dmg', de: 'Für macOS laden', en: 'Download for macOS', sub: { de: 'Apple Silicon', en: 'Apple Silicon' } },
    linux: { file: 'jumpndodge_amd64.deb', de: 'Für Linux laden', en: 'Download for Linux', sub: { de: '.deb · andere Pakete unten', en: '.deb · other packages below' } },
  };
  function pickDownload() {
    const btn = document.getElementById('dl-main');
    if (!btn) return;
    const o = OPTIONS[detect()];
    if (!o) { btn.href = '#download'; btn.querySelector('b').textContent = lang === 'de' ? 'Downloads ansehen' : 'See downloads'; return; }
    btn.href = DL(o.file);
    btn.querySelector('b').textContent = o[lang];
    btn.querySelector('small').textContent = o.sub[lang];
  }

  // ---- direkte Links in der Tabelle
  document.querySelectorAll('[data-file]').forEach((a) => { a.href = DL(a.dataset.file); });

  // ---- aktuelle Version (nur Anzeige - die Links funktionieren auch ohne)
  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: 'application/vnd.github+json' } })
    .then((r) => (r.ok ? r.json() : null))
    .then((rel) => {
      if (!rel || !/^v?\d/.test(rel.tag_name || '')) return;
      const v = rel.tag_name.replace(/^v/, '');
      document.querySelectorAll('[data-version]').forEach((e) => { e.textContent = v; });
      document.querySelectorAll('[data-version-row]').forEach((e) => { e.hidden = false; });
    })
    .catch(() => {});

  setLang(lang);
})();
