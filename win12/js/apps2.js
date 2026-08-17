/* ==========================================================================
   Поведение приложений как в настоящей системе:
   файловые ассоциации, «Свойства» отдельным окном, меню Файл/Правка/Вид,
   диспетчер задач с настоящими метриками
   ========================================================================== */
'use strict';

/* ==========================================================================
   1. Файловые ассоциации
   ========================================================================== */
const Assoc = {
  KEY:'assoc',
  DEFAULT:{
    txt:'notepad', md:'notepad', log:'notepad', ini:'notepad', csv:'notepad',
    js:'notepad', json:'notepad', css:'notepad', py:'notepad',
    html:'browser', htm:'browser',
    png:'photos', jpg:'photos', jpeg:'photos', gif:'photos', webp:'photos', bmp:'photos', svg:'photos',
    mp3:'music', wav:'music', ogg:'music', m4a:'music', flac:'music'
  },
  map(){ return { ...this.DEFAULT, ...KV.get(this.KEY, {}) }; },
  ext(name){ const m = String(name).match(/\.([a-z0-9]+)$/i); return m ? m[1].toLowerCase() : ''; },

  appFor(node){
    const byExt = this.map()[this.ext(node.name)];
    if (byExt && APPS[byExt]) return byExt;
    if (node.img) return 'photos';
    return 'notepad';
  },
  set(ext, appId){
    const m = KV.get(this.KEY, {});
    m[ext] = appId; KV.set(this.KEY, m);
    Shell.toast('Приложения', `.${ext} теперь открывается в «${APPS[appId].name}»`, APPS[appId].glyph);
  },

  /* чем вообще можно открыть файл */
  openers(){
    return ['notepad','photos','browser','music','paint','term']
      .filter(id => APPS[id])
      .map(id => ({ id, name:APPS[id].name, glyph:APPS[id].glyph }));
  },

  open(node, path, appId){
    if (!node || node.type === 'dir') return;
    const id = appId || this.appFor(node);
    const file = { name:node.name, path:(path || []).slice(), body:node.body, img:node.img };
    if (id === 'photos' && node.img) return WM.open('photos', { img:node.img, name:node.name });
    if (id === 'music')  return WM.open('music');
    if (id === 'paint')  return WM.open('paint');
    if (id === 'browser') return WM.open('browser');
    if (id === 'term')   return WM.open('term');
    return WM.open('notepad', { file });
  },

  /* пункт меню «Открыть с помощью» */
  menu(node, path){
    const cur = this.appFor(node);
    const ext = this.ext(node.name);
    return this.openers().map(o => ({
      i:o.glyph, t:o.name + (o.id === cur ? ' · по умолчанию' : ''),
      f: async () => {
        this.open(node, path, o.id);
        if (o.id !== cur && ext){
          const always = await Dlg.confirm('Открыть с помощью',
            `Всегда открывать файлы .${ext} в «${o.name}»?`, { icon:o.glyph, okText:'Всегда', cancelText:'Только сейчас' });
          if (always) this.set(ext, o.id);
        }
      }
    }));
  }
};
window.Assoc = Assoc;

/* ==========================================================================
   2. Свойства файла — отдельное окно, а не текстовый блок
   ========================================================================== */
APPS.props = {
  name:'Свойства', glyph:'ℹ️', bg:'linear-gradient(140deg,#a5b4fc,#4f46e5)', w:420, h:480,
  render(win, opts){
    const node = opts && opts.node;
    const path = (opts && opts.path) || [];
    if (!node){ win.body.innerHTML = '<div class="pad muted">Файл не выбран</div>'; return; }

    const size = n => n.type === 'dir'
      ? Object.values(n.children || {}).reduce((s, c) => s + size(c), 0)
      : (n.img ? Math.round(n.img.length * .75) : new Blob([n.body || '']).size);
    const fmt = b => b < 1024 ? b + ' Б' : b < 1048576 ? (b / 1024).toFixed(1) + ' КБ' : (b / 1048576).toFixed(2) + ' МБ';
    const date = t => t ? new Date(t).toLocaleString('ru-RU') : '—';
    const ext = Assoc.ext(node.name);
    const opener = APPS[Assoc.appFor(node)];

    win.setTitle(node.name + ' — Свойства');
    const wrap = el('div', 'app col scroll pad');
    win.body.appendChild(wrap);

    const head = el('div', 'props-head');
    head.innerHTML = `<div class="props-ico">${node.type === 'dir' ? '📁' : node.img ? '🖼️' : '📄'}</div>
      <div><b>${esc(node.name)}</b><div class="tiny muted">${node.type === 'dir' ? 'Папка' : (ext ? ext.toUpperCase() + ' файл' : 'Файл')}</div></div>`;
    wrap.appendChild(head);

    const rows = [
      ['Расположение', '/' + path.join('/')],
      ['Размер', fmt(size(node))],
      ['Создан', date(node.ctime)],
      ['Изменён', date(node.mtime)]
    ];
    if (node.type === 'dir'){
      const dirs = Object.values(node.children || {}).filter(c => c.type === 'dir').length;
      const files = Object.values(node.children || {}).length - dirs;
      rows.push(['Содержит', `${files} файлов, ${dirs} папок`]);
    } else {
      rows.push(['Символов', String((node.body || '').length)]);
      if ((node.body || '').length) rows.push(['Строк', String((node.body || '').split('\n').length)]);
    }
    const card2 = el('div', 'card');
    rows.forEach(([k, v]) => {
      const r = el('div', 'set-row');
      r.innerHTML = `<div class="l"><b>${k}</b></div><div class="ctl muted tiny">${esc(v)}</div>`;
      card2.appendChild(r);
    });
    wrap.appendChild(card2);

    if (node.type === 'file'){
      const c = card('Открывается с помощью');
      const sel2 = el('select', 'inp');
      Assoc.openers().forEach(o => {
        const op = el('option', '', o.glyph + ' ' + o.name);
        op.value = o.id; sel2.appendChild(op);
      });
      sel2.value = Assoc.appFor(node);
      sel2.onchange = () => { if (ext) Assoc.set(ext, sel2.value); };
      c.appendChild(row('📂', 'Приложение', ext ? `Для всех файлов .${ext}` : 'Только для этого файла', sel2));
      const openBtn = el('button', 'btn pri', 'Открыть');
      openBtn.onclick = () => Assoc.open(node, path);
      c.appendChild(row(opener.glyph, 'Открыть сейчас', opener.name, openBtn));
      wrap.appendChild(c);
    }
  }
};

/* «Свойства» в контекстных меню Проводника и стола */
(function patchProps(){
  const ctxOrig = Shell.ctx.bind(Shell);
  Shell.ctx = function(x, y, items){
    const win = WM.wins.find(w => w.appId === 'files' && w.node.classList.contains('focus'));
    const patched = [];
    items.forEach(it => {
      if (it !== 'hr' && it.t === 'Открыть' && win && win.data.path){
        const sel = $$('.fe-it.sel, .fe-tr.sel', win.body).map(n => n.dataset.name);
        const node = sel.length === 1 && FS.node([...win.data.path, sel[0]]);
        if (node && node.type === 'file'){
          patched.push(it);
          patched.push({ i:'📂', t:'Открыть с помощью', sub:Assoc.menu(node, win.data.path) });
          return;
        }
      }
      if (it !== 'hr' && it.t === 'Свойства' && win && win.data.path){
        const sel = $$('.fe-it.sel, .fe-tr.sel', win.body).map(n => n.dataset.name);
        const node = sel.length === 1 && FS.node([...win.data.path, sel[0]]);
        if (node){
          patched.push({ i:'ℹ️', t:'Свойства', f:() => WM.open('props', { node, path:win.data.path }) });
          return;
        }
      }
      patched.push(it);
    });
    return ctxOrig(x, y, patched);
  };

  /* вложенные меню в Shell.ctx */
  const ctxOrig2 = Shell.ctx.bind(Shell);
  Shell.ctx = function(x, y, items){
    const flat = items.map(it => (it !== 'hr' && it.sub)
      ? { ...it, t:it.t + ' ▸', f:() => setTimeout(() => Shell.ctx(x + 30, y + 30, it.sub), 10) }
      : it);
    return ctxOrig2(x, y, flat);
  };
})();

/* ассоциации применяются при открытии файлов */
(function patchOpening(){
  const iconsOrig = Shell.renderIcons.bind(Shell);
  Shell.renderIcons = function(){
    iconsOrig();
    $$('.di').forEach(n => {
      const it = n._it;
      if (!it || !it.file || it.file.type === 'dir') return;
      const node = it.file, path = ['Рабочий стол'];
      n.ondblclick = () => Assoc.open(node, path);
      const prevCtx = n.oncontextmenu;
      n.oncontextmenu = e => {
        e.preventDefault(); e.stopPropagation();
        $$('.di').forEach(x => x.classList.remove('sel')); n.classList.add('sel');
        Shell.ctx(e.clientX, e.clientY, [
          { i:'📂', t:'Открыть', f:() => Assoc.open(node, path) },
          { i:'🧩', t:'Открыть с помощью', sub:Assoc.menu(node, path) },
          'hr',
          { i:'✏️', t:'Переименовать', k:'F2', f:async () => {
              const nn = await Dlg.prompt('Переименовать', 'Новое имя', node.name);
              if (nn && nn !== node.name){ FS.rename(path, node.name, nn); Shell.renderIcons(); } } },
          { i:'🗑️', t:'Удалить', k:'Del', f:async () => {
              if (await Dlg.confirm('Удалить в корзину', `«${node.name}» будет перемещён в корзину.`,
                  { icon:'🗑️', okText:'Удалить', danger:true })){
                FS.rm(path, node.name); Shell.renderIcons(); } } },
          'hr',
          { i:'ℹ️', t:'Свойства', f:() => WM.open('props', { node, path }) }
        ]);
      };
    });
  };
  Shell.renderIcons();
})();

/* ==========================================================================
   3. Меню Файл / Правка / Вид в приложениях
   ========================================================================== */
const Menu = {
  bar(win, groups){
    const bar = el('div', 'menubar');
    groups.forEach(g => {
      const b = el('button', 'menu-b', g.t);
      b.onclick = e => {
        e.stopPropagation();
        const r = b.getBoundingClientRect();
        Shell.ctx(r.left, r.bottom + 4, typeof g.items === 'function' ? g.items() : g.items);
      };
      bar.appendChild(b);
    });
    win.body.style.flexDirection = 'column';   // тело окна — flex-строка, разворачиваем в колонку
    win.body.prepend(bar);                     // меню всегда самое верхнее в окне
    return bar;
  }
};
window.Menu = Menu;

(function appMenus(){
  /* --- Блокнот --- */
  const np = APPS.notepad.render;
  APPS.notepad.render = function(win, opts){
    np.call(this, win, opts);
    const area = $('.np-area', win.body);
    const bar = $('.toolbar', win.body);
    if (!area || !bar) return;
    const click = t => { const b = $$('.btn', bar).find(x => x.textContent.includes(t)); if (b) b.click(); };

    Menu.bar(win, [
      { t:'Файл', items:() => [
        { i:'＋', t:'Новый', k:'Ctrl+N', f:() => click('Новый') },
        { i:'📂', t:'Открыть…', k:'Ctrl+O', f:() => click('Открыть') },
        { i:'💾', t:'Сохранить', k:'Ctrl+S', f:() => click('Сохранить') },
        { i:'⬇️', t:'Скачать на компьютер', f:() => click('Скачать') },
        'hr',
        { i:'✖', t:'Закрыть', k:'Alt+F4', f:() => win.close() }
      ]},
      { t:'Правка', items:() => [
        { i:'🔎', t:'Найти и заменить', k:'Ctrl+F', f:() => click('Найти') },
        { i:'🔠', t:'Выделить всё', k:'Ctrl+A', f:() => { area.focus(); area.select(); } },
        'hr',
        { i:'🕐', t:'Вставить дату и время', k:'F5', f:() => {
            const s = area.selectionStart;
            area.setRangeText(new Date().toLocaleString('ru-RU'), s, area.selectionEnd, 'end');
            area.dispatchEvent(new Event('input')); } },
        { i:'🧹', t:'Очистить всё', f:async () => {
            if (await Dlg.confirm('Очистить', 'Весь текст будет удалён.', { okText:'Очистить', danger:true })){
              area.value = ''; area.dispatchEvent(new Event('input')); } } }
      ]},
      { t:'Вид', items:() => [
        { i:'🔡', t:'Крупнее', f:() => { const v = parseInt(area.style.fontSize || 14) + 2;
            area.style.fontSize = v + 'px'; KV.set('notepad.size', v); } },
        { i:'🔠', t:'Мельче', f:() => { const v = Math.max(9, parseInt(area.style.fontSize || 14) - 2);
            area.style.fontSize = v + 'px'; KV.set('notepad.size', v); } },
        'hr',
        { i:'↩️', t:'Перенос строк', f:() => {
            const on = area.style.whiteSpace !== 'pre';
            area.style.whiteSpace = on ? 'pre' : 'pre-wrap'; KV.set('notepad.wrap', !on); } }
      ]}
    ]);
  };

  /* --- Paint --- */
  const pt = APPS.paint.render;
  APPS.paint.render = function(win, opts){
    pt.call(this, win, opts);
    const bar = $('.toolbar', win.body);
    const cv = $('#paint-canvas', win.body) || $('canvas', win.body);
    if (!bar || !cv) return;
    const click = t => { const b = $$('.btn', bar).find(x => x.textContent.includes(t)); if (b) b.click(); };

    Menu.bar(win, [
      { t:'Файл', items:() => [
        { i:'💾', t:'Сохранить в Изображения', f:() => click('В файлы') },
        { i:'⬇️', t:'Скачать PNG', f:() => {
            const a = el('a'); a.setAttribute('download', 'Рисунок.png');
            a.href = cv.toDataURL(); document.body.appendChild(a); a.click(); a.remove();
            Shell.toast('Paint', 'Изображение сохранено на компьютер', '⬇️'); } },
        'hr',
        { i:'✖', t:'Закрыть', f:() => win.close() }
      ]},
      { t:'Правка', items:() => [
        { i:'↶', t:'Отменить', f:() => click('Отмена') },
        { i:'🗑', t:'Очистить холст', f:() => click('Очистить') }
      ]},
      { t:'Вид', items:() => [
        { i:'🖼', t:'Размер холста', f:() => Dlg.alert('Холст',
            `${cv.width} × ${cv.height} пикселей`, '🖼') }
      ]}
    ]);
  };
})();

/* ==========================================================================
   4. Диспетчер задач: только настоящие метрики
   ========================================================================== */
APPS.taskmgr = {
  name:'Диспетчер задач', glyph:'📊', bg:'linear-gradient(140deg,#cbd5e1,#475569)',
  w:720, h:560, single:true,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const top = el('div', 'tm-cards');
    const list = el('div', 'scroll');
    const foot = el('div', 'statusbar');
    wrap.append(top, list, foot);

    /* реальный FPS */
    let fps = 0, frames = 0, t0 = performance.now();
    const tick = () => {
      frames++;
      const now = performance.now();
      if (now - t0 >= 1000){ fps = Math.round(frames * 1000 / (now - t0)); frames = 0; t0 = now; }
      if (win.node.isConnected) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);

    const opened = new Map();
    const mb = b => b < 1048576 ? (b / 1024).toFixed(0) + ' КБ' : (b / 1048576).toFixed(1) + ' МБ';

    const draw = async () => {
      const mem = performance.memory;
      const st = await Real.storage();
      const up = Math.round(performance.now() / 1000);

      top.innerHTML = '';
      const cards = [
        ['Кадров в секунду', fps + ' FPS', 'замер по requestAnimationFrame'],
        ['Память JavaScript', mem ? mb(mem.usedJSHeapSize) : 'н/д',
          mem ? 'из ' + mb(mem.jsHeapSizeLimit) + ' лимита' : 'браузер не отдаёт performance.memory'],
        ['Хранилище', st.usage != null ? mb(st.usage) : (st.ls / 1024).toFixed(0) + ' КБ',
          st.quota ? 'из ' + (st.quota / 1048576).toFixed(0) + ' МБ квоты' : 'localStorage'],
        ['Время работы', `${Math.floor(up / 3600)} ч ${Math.floor(up / 60) % 60} мин`, 'с момента загрузки страницы']
      ];
      cards.forEach(([t, v, s]) => {
        const c = el('div', 'tm-card');
        c.innerHTML = `<div class="tm-k">${t}</div><div class="tm-v">${v}</div><div class="tiny muted">${s}</div>`;
        top.appendChild(c);
      });

      list.innerHTML = `<div class="tm-row head"><div>Приложение</div><div>Открыто</div><div>Элементов DOM</div><div></div></div>`;
      WM.wins.forEach(w => {
        if (!opened.has(w.id)) opened.set(w.id, Date.now());
        const secs = Math.round((Date.now() - opened.get(w.id)) / 1000);
        const nodes = w.node.querySelectorAll('*').length;
        const r = el('div', 'tm-row');
        r.innerHTML = `<div>${w.app.glyph} ${esc(w.app.name)}
            <div class="tiny muted">${esc(w.titleEl.textContent)}</div></div>
          <div>${secs < 60 ? secs + ' с' : Math.floor(secs / 60) + ' мин'}</div>
          <div>${nodes}</div><div><button class="btn">Снять</button></div>`;
        $('.btn', r).onclick = () => { WM.close(w); setTimeout(draw, 300); };
        list.appendChild(r);
      });
      if (!WM.wins.length) list.appendChild(el('div', 'empty', 'Нет запущенных приложений'));

      const totalNodes = document.querySelectorAll('*').length;
      foot.innerHTML = `<span>Окон: ${WM.wins.length}</span><span>Элементов на странице: ${totalNodes}</span>
        <span class="grow"></span>
        <span>${performance.memory ? 'метрики памяти реальные' : 'память доступна только в Chrome'}</span>`;
      win.setSub(`${fps} FPS · окон ${WM.wins.length}`);
    };

    const iv = setInterval(draw, 1500);
    draw();
    win.onClose = () => clearInterval(iv);
  }
};
