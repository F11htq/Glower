/* ==========================================================================
   Ещё немного настоящей системы:
   недавние документы и списки переходов, ярлыки, «Свернуть всё»,
   печать, привычные горячие клавиши
   ========================================================================== */
'use strict';

/* ==========================================================================
   1. Недавние документы
   ========================================================================== */
const Recent = {
  KEY:'recent.files',
  list(){ return KV.get(this.KEY, []); },
  add(name, path, app){
    if (!name) return;
    const l = this.list().filter(f => !(f.name === name && String(f.path) === String(path)));
    l.unshift({ name, path:(path || []).slice(), app, ts:Date.now() });
    KV.set(this.KEY, l.slice(0, 20));
    Shell.renderStart();
  },
  forApp(app){ return this.list().filter(f => f.app === app); },
  clear(){ KV.set(this.KEY, []); Shell.renderStart(); }
};
window.Recent = Recent;

/* запоминаем всё, что открывают */
(function trackRecent(){
  const openOrig = Assoc.open.bind(Assoc);
  Assoc.open = function(node, path, appId){
    const id = appId || this.appFor(node);
    Recent.add(node.name, path, id);
    return openOrig(node, path, appId);
  };
})();

/* ==========================================================================
   2. «Рекомендуем» в Пуске — настоящие недавние файлы
   ========================================================================== */
(function startRecent(){
  const orig = Shell.renderStart.bind(Shell);
  Shell.renderStart = function(){
    orig();
    const box = $('#start-reco');
    const files = Recent.list();
    if (!box || !files.length) return;

    const head = $('#start-reco-sec .start-head span');
    if (head) head.textContent = 'Недавние';
    box.innerHTML = '';
    files.slice(0, 6).forEach(f => {
      const app = APPS[f.app] || APPS.notepad;
      const b = el('button', 'rc');
      b.appendChild(appIcon(app));
      const when = new Date(f.ts);
      const ago = Date.now() - f.ts < 86400000
        ? when.toLocaleTimeString('ru-RU', { hour:'2-digit', minute:'2-digit' })
        : when.toLocaleDateString('ru-RU', { day:'numeric', month:'short' });
      b.appendChild(el('div', 't', `${esc(f.name)}<small>/${esc(f.path.join('/'))} · ${ago}</small>`));
      b.onclick = () => {
        const node = FS.node([...f.path, f.name]);
        if (!node) return Shell.toast('Недавние', 'Файл больше не существует', '⚠️');
        Assoc.open(node, f.path); Shell.closePanels();
      };
      b.oncontextmenu = e => { e.preventDefault(); Shell.ctx(e.clientX, e.clientY, [
        { i:'📂', t:'Показать в Проводнике', f:() => { WM.open('files', { path:f.path }); Shell.closePanels(); } },
        { i:'🧹', t:'Очистить список', f:() => Recent.clear() }
      ]); };
      box.appendChild(b);
    });
  };
  Shell.renderStart();
})();

/* ==========================================================================
   3. Списки переходов: правый клик по значку в доке
   ========================================================================== */
(function jumpLists(){
  const orig = Shell.renderDock.bind(Shell);
  Shell.renderDock = function(){
    orig();
    $$('#dock-items .dock-item').forEach(b => {
      const id = b.dataset.app;
      if (!id || !APPS[id]) return;
      b.oncontextmenu = e => {
        e.preventDefault();
        const a = APPS[id];
        const open = WM.wins.filter(w => w.appId === id);
        const recent = Recent.forApp(id).slice(0, 5);
        Shell.ctx(e.clientX, e.clientY, [
          ...(recent.length ? [{ i:'🕐', t:'Недавние', head:true }] : []),
          ...recent.map(f => ({ i:'📄', t:f.name, f:() => {
            const node = FS.node([...f.path, f.name]);
            node ? Assoc.open(node, f.path) : Shell.toast('Недавние', 'Файл не найден', '⚠️');
          }})),
          ...(recent.length ? ['hr'] : []),
          { i:a.glyph, t:'Открыть', f:() => Shell.launch(id) },
          ...(open.length ? [{ i:'✖', t:'Закрыть все окна', f:() => open.forEach(w => WM.close(w)) }] : []),
          'hr',
          { i:'🖥️', t:'Ярлык на рабочий стол', f:() => Link.toDesktop({ app:id }, a.name) },
          { i:'📌', t:'Убрать из дока', f:() => {
              S.dockApps = S.dockApps.filter(x => x !== id); Store.save(); Shell.renderDock(); } }
        ]);
      };
    });
  };
  Shell.renderDock();
})();

/* ==========================================================================
   4. Ярлыки
   ========================================================================== */
const Link = {
  toDesktop(target, title){
    const name = (title || 'Ярлык') + ' — ярлык';
    const d = FS.node(['Рабочий стол']);
    if (!d) return;
    d.children[FS.uniqueName(['Рабочий стол'], name)] = {
      type:'file', name, link:target, body:'', ctime:Date.now(), mtime:Date.now()
    };
    FS.save();
    Shell.renderIcons();
    Shell.toast('Ярлык создан', name, '🔗');
  },
  open(node){
    const l = node.link;
    if (!l) return false;
    if (l.app){ WM.open(l.app); return true; }
    const t = FS.node([...l.path, l.name]);
    if (!t) { Shell.toast('Ярлык', 'Целевой файл не найден', '⚠️'); return true; }
    if (t.type === 'dir') WM.open('files', { path:[...l.path, l.name] });
    else Assoc.open(t, l.path);
    return true;
  }
};
window.Link = Link;

/* ярлыки открываются, а не читаются как текст */
(function linkOpening(){
  const orig = Assoc.open.bind(Assoc);
  Assoc.open = function(node, path, appId){
    if (node && node.link && !appId) return Link.open(node);
    return orig(node, path, appId);
  };

  const icons = Shell.renderIcons.bind(Shell);
  Shell.renderIcons = function(){
    icons();
    $$('.di').forEach(n => {
      const it = n._it;
      if (it && it.file && it.file.link){
        n.classList.add('is-link');
        n.ondblclick = () => Link.open(it.file);
      }
    });
  };
  Shell.renderIcons();

  /* «Создать ярлык» в Проводнике */
  const ctxOrig = Shell.ctx.bind(Shell);
  Shell.ctx = function(x, y, items){
    const win = WM.wins.find(w => w.appId === 'files' && w.node.classList.contains('focus'));
    if (!win || !win.data.path) return ctxOrig(x, y, items);
    const sel = $$('.fe-it.sel, .fe-tr.sel', win.body).map(n => n.dataset.name);
    if (sel.length !== 1) return ctxOrig(x, y, items);
    const node = FS.node([...win.data.path, sel[0]]);
    if (!node) return ctxOrig(x, y, items);
    const out = [];
    items.forEach(it => {
      out.push(it);
      if (it !== 'hr' && it.t === 'Скачать на компьютер')
        out.push({ i:'🔗', t:'Создать ярлык на столе',
          f:() => Link.toDesktop({ path:win.data.path, name:node.name }, node.name.replace(/\.[^.]+$/, '')) });
    });
    return ctxOrig(x, y, out);
  };
})();

/* ==========================================================================
   5. «Свернуть всё» в углу панели задач
   ========================================================================== */
(function showDesktop(){
  const btn = el('button', 'show-desktop');
  btn.dataset.tip = 'Свернуть все окна';
  let stash = [];
  btn.onclick = () => {
    const open = WM.wins.filter(w => !w.minimized && w.desk === WM.desk);
    if (open.length){ stash = open; open.forEach(w => WM.minimize(w)); }
    else { stash.forEach(w => { if (WM.wins.includes(w)) WM.restore(w); }); stash = []; }
  };
  $('#dock').appendChild(btn);
  const orig = Shell.updateTaskbar.bind(Shell);
  Shell.updateTaskbar = function(){ orig(); $('#dock').appendChild(btn); };
})();

/* ==========================================================================
   6. Печать и привычные сочетания клавиш
   ========================================================================== */
(function printing(){
  const np = APPS.notepad.render;
  APPS.notepad.render = function(win, opts){
    np.call(this, win, opts);
    const area = $('.np-area', win.body);
    if (!area) return;

    const print = () => {
      const title = win.titleEl.textContent.split(' — ')[0];
      const f = el('iframe');
      f.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0';
      document.body.appendChild(f);
      const d = f.contentDocument;
      d.write(`<!doctype html><meta charset="utf-8"><title>${esc(title)}</title>
        <style>body{font:13px/1.6 ui-monospace,Consolas,monospace;padding:24px;white-space:pre-wrap}
        h1{font:600 15px sans-serif;margin:0 0 16px}</style>
        <h1>${esc(title)}</h1>${esc(area.value)}`);
      d.close();
      f.contentWindow.focus();
      f.contentWindow.print();
      setTimeout(() => f.remove(), 1000);
    };
    win.data.print = print;

    // пункт в меню «Файл»
    const fileBtn = $$('.menu-b', win.body)[0];
    if (fileBtn){
      const prev = fileBtn.onclick;
      fileBtn.onclick = e => {
        prev(e);
        setTimeout(() => {
          const c = $('#ctx');
          if (!c.classList.contains('on')) return;
          const b = el('button', '', `<span class="em">🖨</span><span>Печать…</span><span class="k">Ctrl+P</span>`);
          b.onclick = () => { c.classList.remove('on'); print(); };
          c.insertBefore(b, c.children[4] || null);
        }, 0);
      };
    }
    area.addEventListener('keydown', e => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p'){ e.preventDefault(); print(); }
    });
  };

  /* Ctrl+Shift+N — новая папка в Проводнике */
  addEventListener('keydown', async e => {
    if (!(e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'n')) return;
    const win = WM.wins.find(w => w.appId === 'files' && w.node.classList.contains('focus'));
    if (!win || !win.data.path) return;
    e.preventDefault();
    const n = await Dlg.prompt('Создать папку', 'Имя новой папки', 'Новая папка', '📁');
    if (n){ FS.mkdir(win.data.path, FS.uniqueName(win.data.path, n)); win.data.refresh && win.data.refresh(); }
  });
})();
