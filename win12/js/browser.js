/* ==========================================================================
   Браузер GlowerOS

   Страницы рисует настоящий движок — второй Chromium, поднятый агентом без
   собственного интерфейса. Оболочка забирает у него кадры по отладочному
   протоколу и отправляет обратно мышь, колесо и клавиатуру. Всё вокруг
   страницы — вкладки, адресная строка, закладки, история, загрузки — наше.

   Когда система работает прототипом в браузере, движка нет и взяться ему
   неоткуда: в этом случае открывается стартовая страница с честным
   объяснением, а не подделка сайта.
   ========================================================================== */
'use strict';

/* ---------- разговор с движком ---------- */
class CDP {
  constructor(url){ this.url = url; this.n = 0; this.wait = new Map(); this.subs = new Map(); }

  connect(){
    return new Promise((res, rej) => {
      const s = this.sock = new WebSocket(this.url);
      s.onopen = () => res(this);
      s.onerror = () => rej(new Error('движок не отвечает по отладочному протоколу'));
      s.onclose = () => { this.closed = true; if (this.onClose) this.onClose(); };
      s.onmessage = e => this.take(JSON.parse(e.data));
    });
  }

  send(method, params = {}, sessionId){
    if (this.closed) return Promise.reject(new Error('движок закрыт'));
    const id = ++this.n;
    const msg = { id, method, params };
    if (sessionId) msg.sessionId = sessionId;
    return new Promise((res, rej) => {
      this.wait.set(id, { res, rej });
      this.sock.send(JSON.stringify(msg));
    });
  }

  /* sid не задан — слушаем событие от любой вкладки */
  on(method, fn, sid){
    const k = (sid || '*') + '|' + method;
    if (!this.subs.has(k)) this.subs.set(k, []);
    this.subs.get(k).push(fn);
  }
  off(sid){ [...this.subs.keys()].filter(k => k.startsWith(sid + '|')).forEach(k => this.subs.delete(k)); }

  take(m){
    if (m.id){
      const w = this.wait.get(m.id);
      if (!w) return;
      this.wait.delete(m.id);
      return m.error ? w.rej(new Error(m.error.message)) : w.res(m.result);
    }
    (this.subs.get((m.sessionId || '') + '|' + m.method) || []).forEach(f => f(m.params, m.sessionId));
    (this.subs.get('*|' + m.method) || []).forEach(f => f(m.params, m.sessionId));
  }
}

/* ---------- сам движок: один на всю систему ---------- */
const Web = {
  cdp:null, starting:null,
  SEARCH:{ ddg:['DuckDuckGo', 'https://duckduckgo.com/?q='],
           google:['Google', 'https://www.google.com/search?q='],
           yandex:['Яндекс', 'https://yandex.ru/search/?text='] },

  available(){ return !!(window.OS && OS.on()); },

  async engine(){
    if (this.cdp && !this.cdp.closed) return this.cdp;
    if (this.starting) return this.starting;
    this.starting = (async () => {
      if (!this.available())
        throw new Error('Движок браузера живёт на машине. Он доступен, когда система управляет ей сама, а не открыта страницей в чужом браузере.');
      const st = await Platform.rpc('browser.start');
      const c = new CDP(st.ws);
      await c.connect();
      await c.send('Target.setDiscoverTargets', { discover:true });
      try {
        await c.send('Browser.setDownloadBehavior',
          { behavior:'allow', downloadPath:Platform.info.root || '/tmp', eventsEnabled:true });
        c.on('Browser.downloadWillBegin', p =>
          Shell.toast('Загрузка', 'Началась: ' + (p.suggestedFilename || p.url), '⬇️'));
        c.on('Browser.downloadProgress', p => {
          if (p.state !== 'completed') return;
          Shell.toast('Загрузка', 'Файл сохранён в рабочую папку', '✅');
          if (Platform.mount) Platform.mount().catch(() => {});
        });
      } catch(e){}
      c.onClose = () => { this.cdp = null; };
      this.cdp = c; this.starting = null;
      return c;
    })();
    try { return await this.starting; }
    finally { this.starting = null; }
  },

  /* строка из адресной строки → настоящий адрес */
  resolve(s){
    s = String(s || '').trim();
    if (!s) return '';
    if (/^[a-z]+:\/\//i.test(s) || /^glower:/.test(s)) return s;
    const looksHost = /^[\w-]+(\.[\w-]+)+(\/|$|:\d)/.test(s) || s.startsWith('localhost');
    if (looksHost) return 'https://' + s;
    const eng = this.SEARCH[KV.get('web.search', 'ddg')] || this.SEARCH.ddg;
    return eng[1] + encodeURIComponent(s);
  },

  bookmarks(){ return KV.get('web.bookmarks', [
    { t:'DuckDuckGo', u:'https://duckduckgo.com' },
    { t:'Википедия', u:'https://ru.wikipedia.org' },
    { t:'GitHub', u:'https://github.com' }
  ]); },
  saveBookmarks(v){ KV.set('web.bookmarks', v); },

  history(){ return KV.get('web.history', []); },
  remember(url, title){
    if (!url || url.startsWith('glower:') || url === 'about:blank') return;
    const h = this.history().filter(x => x.u !== url);
    h.unshift({ u:url, t:title || url, ts:Date.now() });
    KV.set('web.history', h.slice(0, 300));
  }
};
window.Web = Web;

/* ==========================================================================
   Вкладка: кусок экрана движка внутри нашего окна
   ========================================================================== */
class WebTab {
  constructor(host){
    this.host = host;                       // элемент, в котором живёт картинка
    this.node = el('div', 'br-view');
    this.node.tabIndex = 0;
    this.img = el('img', 'br-frame');
    this.img.draggable = false;
    this.node.appendChild(this.img);
    this.title = 'Новая вкладка';
    this.url = '';
    this.loading = false;
    this.scale = 1;
    this.bindInput();
  }

  async attach(cdp, url, existingTarget){
    this.cdp = cdp;
    const targetId = existingTarget ||
      (await cdp.send('Target.createTarget', { url:'about:blank' })).targetId;
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten:true });
    this.targetId = targetId;
    this.sid = sessionId;

    await cdp.send('Page.enable', {}, this.sid);
    await cdp.send('Page.setLifecycleEventsEnabled', { enabled:true }, this.sid).catch(() => {});

    cdp.on('Page.screencastFrame', p => {
      this.img.src = 'data:image/jpeg;base64,' + p.data;
      cdp.send('Page.screencastFrameAck', { sessionId:p.sessionId }, this.sid).catch(() => {});
    }, this.sid);

    cdp.on('Page.frameNavigated', p => {
      if (p.frame.parentId) return;                 // вложенные кадры нас не касаются
      this.url = p.frame.url;
      this.loading = true;
      this.changed();
    }, this.sid);

    cdp.on('Page.loadEventFired', async () => {
      this.loading = false;
      try {
        const r = await cdp.send('Runtime.evaluate',
          { expression:'document.title', returnByValue:true }, this.sid);
        this.title = (r.result && r.result.value) || this.url;
      } catch(e){}
      Web.remember(this.url, this.title);
      this.changed();
    }, this.sid);

    await this.fit();
    if (url) await this.navigate(url);
  }

  changed(){ if (this.onChange) this.onChange(this); }

  /* заставка «готовлю страницу» поверх пустого полотна */
  waiting(on){
    if (!on){ if (this.wait){ this.wait.remove(); this.wait = null; } return; }
    if (this.wait) return;
    this.wait = el('div', 'br-wait',
      `<div class="br-spin"></div><div class="t">Открываю страницу…</div>
       <div class="s">Первый запуск на слабой машине занимает до минуты</div>
       <button class="btn">Что происходит</button>`);
    this.node.appendChild(this.wait);
    $('button', this.wait).onclick = async () => {
      let st = {};
      try { st = await Platform.rpc('browser.state'); } catch(e){}
      const why = [
        'движок: ' + (st.running ? 'работает' : 'не работает'),
        st.exitCode != null ? 'код выхода ' + st.exitCode : null,
        st.stderr || null
      ].filter(Boolean).join('\n');
      Dlg.alert('Страница ещё не пришла', why || 'Движок молчит.', 'ℹ️');
    };
  }

  async navigate(url){
    if (!this.sid) return;
    this.loading = true; this.changed();
    await this.cdp.send('Page.navigate', { url }, this.sid);
  }
  async reload(){ if (this.sid) await this.cdp.send('Page.reload', {}, this.sid); }
  async stop(){ if (this.sid) await this.cdp.send('Page.stopLoading', {}, this.sid).catch(() => {}); }

  async step(delta){
    if (!this.sid) return;
    const h = await this.cdp.send('Page.getNavigationHistory', {}, this.sid);
    const i = h.currentIndex + delta;
    if (i < 0 || i >= h.entries.length) return;
    await this.cdp.send('Page.navigateToHistoryEntry', { entryId:h.entries[i].id }, this.sid);
  }

  /* размер картинки движка = размер места под неё, чтобы координаты совпадали */
  async fit(){
    if (!this.sid) return;
    const r = this.node.getBoundingClientRect();
    const w = Math.max(320, Math.round(r.width)), h = Math.max(240, Math.round(r.height));
    if (this.w === w && this.h === h) return;
    this.w = w; this.h = h;
    await this.cdp.send('Emulation.setDeviceMetricsOverride',
      { width:w, height:h, deviceScaleFactor:1, mobile:false }, this.sid).catch(() => {});
    if (this.casting) await this.startCast();
  }

  async startCast(){
    if (!this.sid) return;
    const lite = KV.get('perf.lite', false);
    await this.cdp.send('Page.startScreencast', {
      format:'jpeg', quality:lite ? 45 : 72,
      maxWidth:this.w || 1280, maxHeight:this.h || 800,
      everyNthFrame:lite ? 2 : 1
    }, this.sid).catch(() => {});
    this.casting = true;

    this.ensureFrame();
  }

  /* Движок присылает кадр, только когда на странице что-то перерисовалось:
     пустая или замершая страница иначе осталась бы белым пятном. Тогда просим
     снимок отдельно — но не ждём его в общей цепочке: пока страница не
     отрисовалась, этот запрос может висеть сколько угодно. */
  ensureFrame(){
    [400, 1500, 4000].forEach(ms => setTimeout(async () => {
      if (!this.sid || this.img.src) return;
      try {
        const shot = await this.cdp.send('Page.captureScreenshot', { format:'jpeg', quality:70 }, this.sid);
        if (shot && shot.data && !this.img.src) this.img.src = 'data:image/jpeg;base64,' + shot.data;
      } catch(e){}
    }, ms));

    /* Пока кадра нет, окно не должно быть просто белым: на слабой машине
       первый запуск движка занимает время, и человек должен видеть, что
       система работает, а не сломалась. */
    this.waiting(true);
    const stop = setInterval(() => { if (this.img.src){ clearInterval(stop); this.waiting(false); } }, 200);
    setTimeout(() => clearInterval(stop), 120000);
  }
  async stopCast(){
    if (!this.sid || !this.casting) return;
    this.casting = false;
    await this.cdp.send('Page.stopScreencast', {}, this.sid).catch(() => {});
  }

  async close(){
    if (!this.sid) return;
    this.cdp.off(this.sid);
    await this.cdp.send('Target.closeTarget', { targetId:this.targetId }).catch(() => {});
    this.sid = null;
  }

  /* ---------- ввод ---------- */
  mods(e){
    return (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
  }
  point(e){
    const r = this.img.getBoundingClientRect();
    return { x:Math.round(e.clientX - r.left), y:Math.round(e.clientY - r.top) };
  }
  mouse(type, e, extra = {}){
    if (!this.sid) return;
    const p = this.point(e);
    const btn = ['left', 'middle', 'right'][e.button] || 'left';
    this.cdp.send('Input.dispatchMouseEvent', Object.assign({
      type, x:p.x, y:p.y, modifiers:this.mods(e),
      button:type === 'mouseMoved' && !e.buttons ? 'none' : btn,
      buttons:e.buttons || 0, clickCount:type === 'mouseMoved' ? 0 : (e.detail || 1)
    }, extra), this.sid).catch(() => {});
  }

  bindInput(){
    const n = this.node;
    n.addEventListener('mousedown', e => { e.preventDefault(); n.focus(); this.mouse('mousePressed', e); });
    n.addEventListener('mouseup', e => this.mouse('mouseReleased', e));
    n.addEventListener('mousemove', e => this.mouse('mouseMoved', e));
    n.addEventListener('contextmenu', e => e.preventDefault());
    n.addEventListener('wheel', e => {
      e.preventDefault();
      /* колесо: знак тот же, что у события, кнопка при этом не нажата */
      this.mouse('mouseWheel', e, { deltaX:e.deltaX, deltaY:e.deltaY, button:'none', clickCount:0, buttons:0 });
    }, { passive:false });

    const key = (type, e) => {
      if (!this.sid) return;
      /* сочетания оболочки не перехватываем: пусть система остаётся системой */
      if ((e.ctrlKey || e.metaKey) && ['t', 'w', 'l', 'r'].includes(e.key.toLowerCase())) return;
      e.preventDefault();
      const printable = e.key.length === 1 && !e.ctrlKey && !e.metaKey;
      const p = {
        type: type === 'down' ? (printable ? 'keyDown' : 'rawKeyDown') : 'keyUp',
        key:e.key, code:e.code, modifiers:this.mods(e), autoRepeat:e.repeat,
        windowsVirtualKeyCode:e.keyCode, nativeVirtualKeyCode:e.keyCode
      };
      if (printable && type === 'down'){ p.text = e.key; p.unmodifiedText = e.key.toLowerCase(); }
      this.cdp.send('Input.dispatchKeyEvent', p, this.sid).catch(() => {});
    };
    n.addEventListener('keydown', e => key('down', e));
    n.addEventListener('keyup', e => key('up', e));
  }
}

/* ==========================================================================
   Приложение «Браузер»
   ========================================================================== */
APPS.browser = {
  name:'Браузер', glyph:'🌐', bg:'linear-gradient(140deg,#38bdf8,#0369a1)', w:1060, h:700,

  render(win){
    const wrap = el('div', 'app col br-app'); win.body.appendChild(wrap);
    const tabsBar = el('div', 'br-tabs');
    const bar = el('div', 'br-bar');
    const stage = el('div', 'br-stage');
    wrap.append(tabsBar, bar, stage);

    const st = win._web = { tabs:[], cur:-1 };

    bar.innerHTML = `
      <button class="br-nav" data-a="back" title="Назад">‹</button>
      <button class="br-nav" data-a="fwd" title="Вперёд">›</button>
      <button class="br-nav" data-a="rl" title="Обновить">⟳</button>
      <button class="br-nav" data-a="home" title="Начальная страница">⌂</button>
      <div class="br-url"><span class="br-lock">🔎</span><input spellcheck="false" placeholder="Поиск или адрес"></div>
      <button class="br-nav" data-a="star" title="В закладки">☆</button>
      <button class="br-nav" data-a="menu" title="Меню">⋯</button>`;
    const inp = $('input', bar);
    const lock = $('.br-lock', bar);

    /* ---------- начальная страница: она наша, а не движка ---------- */
    const startPage = () => {
      const n = el('div', 'br-start');
      const eng = Web.SEARCH[KV.get('web.search', 'ddg')] || Web.SEARCH.ddg;
      n.innerHTML = `
        <img class="os-logo br-logo" src="assets/logo.png" alt="" draggable="false">
        <h1>${esc(Brand.name)}</h1>
        <div class="br-search"><input placeholder="Поиск в ${esc(eng[0])} или адрес" spellcheck="false"></div>
        <div class="br-tiles"></div>
        <div class="br-recent"></div>`;
      const si = $('input', n);
      si.onkeydown = e => { if (e.key === 'Enter') go(si.value); };
      setTimeout(() => si.focus(), 60);

      const tiles = $('.br-tiles', n);
      Web.bookmarks().forEach(b => {
        const t = el('div', 'br-tile');
        let host = b.u; try { host = new URL(b.u).hostname.replace(/^www\./, ''); } catch(e){}
        t.innerHTML = `<span class="e">${esc(host[0] ? host[0].toUpperCase() : '·')}</span><b>${esc(b.t)}</b><small>${esc(host)}</small>`;
        t.onclick = () => go(b.u);
        tiles.appendChild(t);
      });

      const rec = Web.history().slice(0, 6);
      if (rec.length){
        const r = $('.br-recent', n);
        r.innerHTML = '<div class="br-recent-t">Недавние</div>';
        rec.forEach(h => {
          const a = el('button', 'br-recent-i', `<b>${esc(h.t)}</b><small>${esc(h.u)}</small>`);
          a.onclick = () => go(h.u);
          r.appendChild(a);
        });
      }
      if (!Web.available()){
        const note = el('div', 'br-note',
          'Страницы рисует движок машины, поэтому здесь, в прототипе внутри чужого браузера, ' +
          'сайты не откроются. В собранной системе GlowerOS этот же браузер открывает настоящий интернет.');
        n.appendChild(note);
      }
      return n;
    };

    /* ---------- вкладки ---------- */
    const mkTab = () => {
      const t = { url:'glower://start', title:'Новая вкладка', view:null, start:startPage(), error:null };
      st.tabs.push(t);
      return t;
    };

    const drawTabs = () => {
      tabsBar.innerHTML = '';
      st.tabs.forEach((t, i) => {
        const b = el('div', 'br-tab' + (i === st.cur ? ' on' : ''));
        b.innerHTML = `<span class="t">${esc(t.loading ? 'Загрузка…' : t.title)}</span>`;
        const x = el('button', 'br-tab-x', '✕');
        x.onclick = e => { e.stopPropagation(); closeTab(i); };
        b.appendChild(x);
        b.onclick = () => show(i);
        tabsBar.appendChild(b);
      });
      const plus = el('button', 'br-tab-add', '+');
      plus.title = 'Новая вкладка';
      plus.onclick = () => { mkTab(); show(st.tabs.length - 1); };
      tabsBar.appendChild(plus);
    };

    const drawBar = () => {
      const t = st.tabs[st.cur];
      if (!t) return;
      if (document.activeElement !== inp) inp.value = t.url === 'glower://start' ? '' : t.url;
      lock.textContent = t.url.startsWith('https://') ? '🔒' : t.url.startsWith('http://') ? '⚠️' : '🔎';
      $('[data-a="rl"]', bar).textContent = t.loading ? '✕' : '⟳';
      const marked = Web.bookmarks().some(b => b.u === t.url);
      $('[data-a="star"]', bar).textContent = marked ? '★' : '☆';
      win.setSub(t.url === 'glower://start' ? 'Начальная страница' : t.url);
    };

    const show = i => {
      st.cur = i;
      stage.innerHTML = '';
      const t = st.tabs[i];
      st.tabs.forEach((x, k) => { if (k !== i && x.view) x.view.stopCast(); });
      if (t.error){
        stage.appendChild(t.error);
      } else if (t.url === 'glower://start' || !t.view){
        stage.appendChild(t.start);
      } else {
        stage.appendChild(t.view.node);
        t.view.fit().then(() => t.view.startCast());
        t.view.node.focus();
      }
      drawTabs(); drawBar();
    };

    const closeTab = i => {
      const t = st.tabs[i];
      if (t.view) t.view.close();
      st.tabs.splice(i, 1);
      if (!st.tabs.length){ mkTab(); return show(0); }
      show(Math.min(i, st.tabs.length - 1));
    };

    const fail = (t, e) => {
      const n = el('div', 'br-error');
      n.innerHTML = `<div class="e">🚫</div><h2>Страница не открылась</h2>
        <p class="muted">${esc(String(e.message || e))}</p>`;
      t.error = n;
      t.title = 'Ошибка';
      show(st.cur);
    };

    /* ---------- переход ---------- */
    const go = async (raw) => {
      const t = st.tabs[st.cur];
      const url = Web.resolve(raw);
      if (!url) return;
      t.error = null;
      if (url === 'glower://start'){ t.url = url; t.title = 'Новая вкладка'; return show(st.cur); }

      t.loading = true; t.title = 'Загрузка…'; t.url = url;
      drawTabs(); drawBar();
      try {
        const cdp = await Web.engine();
        if (!t.view){
          t.view = new WebTab(stage);
          t.view.onChange = v => {
            t.url = v.url || t.url; t.title = v.title; t.loading = v.loading;
            drawTabs(); drawBar();
          };
          stage.innerHTML = '';
          stage.appendChild(t.view.node);
          await t.view.attach(cdp, url);
          await t.view.fit();
          await t.view.startCast();
          t.view.node.focus();
        } else {
          show(st.cur);
          await t.view.navigate(url);
        }
      } catch(e){ t.loading = false; fail(t, e); }
    };

    /* ---------- кнопки ---------- */
    inp.onkeydown = e => { if (e.key === 'Enter'){ go(inp.value); inp.blur(); } };
    bar.onclick = async e => {
      const b = e.target.closest('[data-a]');
      if (!b) return;
      const t = st.tabs[st.cur];
      const a = b.dataset.a;
      if (a === 'back' && t.view) t.view.step(-1);
      if (a === 'fwd' && t.view) t.view.step(1);
      if (a === 'rl' && t.view) t.loading ? t.view.stop() : t.view.reload();
      if (a === 'home'){ t.url = 'glower://start'; t.title = 'Новая вкладка'; t.error = null; show(st.cur); }
      if (a === 'star'){
        const l = Web.bookmarks();
        const i = l.findIndex(x => x.u === t.url);
        if (i >= 0) l.splice(i, 1);
        else if (t.url && t.url !== 'glower://start') l.push({ t:t.title || t.url, u:t.url });
        Web.saveBookmarks(l); drawBar();
        Shell.toast('Браузер', i >= 0 ? 'Закладка убрана' : 'Добавлено в закладки', i >= 0 ? '☆' : '★');
      }
      if (a === 'menu'){
        const r = b.getBoundingClientRect();
        const eng = KV.get('web.search', 'ddg');
        Shell.ctx(r.left - 150, r.bottom + 6, [
          { i:'🕘', t:'История', f:() => openList('history') },
          { i:'★', t:'Закладки', f:() => openList('bookmarks') },
          'hr',
          ...Object.entries(Web.SEARCH).map(([k, v]) => ({
            i:eng === k ? '◉' : '○', t:'Поиск: ' + v[0],
            f:() => { KV.set('web.search', k); Shell.toast('Браузер', 'Поиск через ' + v[0], '🔎'); } })),
          'hr',
          { i:'🧹', t:'Очистить историю', f:() => { KV.set('web.history', []); Shell.toast('Браузер', 'История очищена', '🧹'); } }
        ]);
      }
    };

    const openList = kind => {
      const t = st.tabs[st.cur];
      t.error = null;
      const n = el('div', 'br-list');
      const items = kind === 'history' ? Web.history() : Web.bookmarks();
      n.innerHTML = `<h2>${kind === 'history' ? 'История' : 'Закладки'}</h2>`;
      if (!items.length) n.appendChild(el('div', 'empty', 'Пока пусто'));
      items.forEach(x => {
        const r2 = el('button', 'br-recent-i', `<b>${esc(x.t)}</b><small>${esc(x.u)}</small>`);
        r2.onclick = () => go(x.u);
        n.appendChild(r2);
      });
      t.start = n; t.url = 'glower://start'; t.title = kind === 'history' ? 'История' : 'Закладки';
      show(st.cur);
    };

    /* сочетания самого браузера: их вкладка наверх не пропускает */
    wrap.addEventListener('keydown', e => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const k = e.key.toLowerCase();
      if (k === 't'){ e.preventDefault(); mkTab(); show(st.tabs.length - 1); }
      if (k === 'w'){ e.preventDefault(); closeTab(st.cur); }
      if (k === 'l'){ e.preventDefault(); inp.focus(); inp.select(); }
      if (k === 'r'){ e.preventDefault(); const t = st.tabs[st.cur]; if (t.view) t.view.reload(); }
    });

    /* страница попросила открыть новое окно — открываем вкладкой, а не окном движка */
    const adopt = async (p) => {
      if (p.targetInfo.type !== 'page' || !p.targetInfo.openerId) return;
      if (!st.tabs.some(t => t.view && t.view.targetId === p.targetInfo.openerId)) return;
      const cdp = await Web.engine();
      const t = mkTab();
      t.url = p.targetInfo.url; t.title = p.targetInfo.title || 'Новая вкладка';
      t.view = new WebTab(stage);
      t.view.onChange = v => { t.url = v.url || t.url; t.title = v.title; t.loading = v.loading; drawTabs(); drawBar(); };
      await t.view.attach(cdp, null, p.targetInfo.targetId);
      show(st.tabs.length - 1);
    };
    Web.engine().then(c => c.on('Target.targetCreated', p => { if (!st.gone) adopt(p); })).catch(() => {});

    mkTab();
    show(0);

    win.onClose = () => { st.gone = true; st.tabs.forEach(t => t.view && t.view.close()); };
  },

  onResize(win){
    const st = win._web;
    if (!st) return;
    const t = st.tabs[st.cur];
    if (t && t.view) t.view.fit();
  }
};
