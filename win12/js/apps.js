/* ==========================================================================
   Приложения
   ========================================================================== */
'use strict';

const APPS = {};

/* ---------- Общие хелперы UI ---------- */
function row(icon, title, desc, ctl, cls = ''){
  const r = el('div', 'set-row ' + cls);
  r.innerHTML = `<div class="emo">${icon || ''}</div>
    <div class="l"><b>${title}</b>${desc ? `<small>${desc}</small>` : ''}</div>
    <div class="ctl"></div>`;
  if (ctl) $('.ctl', r).appendChild(ctl);
  return r;
}
function card(title){
  const c = el('div', 'card');
  if (title) c.appendChild(el('div', 'card-t', title));
  return c;
}
function toggle(get, set){
  const t = el('div', 'switch' + (get() ? ' on' : ''));
  t.onclick = () => { const v = !t.classList.contains('on'); t.classList.toggle('on', v); set(v); };
  return t;
}
function slider(get, set, min, max, step, fmt){
  const wrap = el('div', 'row');
  const i = el('input'); i.type = 'range'; i.min = min; i.max = max; i.step = step; i.value = get();
  i.style.width = '150px';
  const lab = el('span', 'tiny muted'); lab.style.width = '52px'; lab.style.textAlign = 'right';
  const upd = () => { lab.textContent = fmt ? fmt(+i.value) : i.value;
    i.style.setProperty('--p', ((i.value - min) / (max - min) * 100) + '%'); };
  i.oninput = () => { set(+i.value); upd(); };
  upd();
  wrap.append(i, lab);
  return wrap;
}
function seg(options, get, set){
  const s = el('div', 'seg');
  options.forEach(o => {
    const b = el('button', get() === o.v ? 'on' : '', o.n);
    b.onclick = () => { set(o.v); $$('button', s).forEach(x => x.classList.remove('on')); b.classList.add('on'); };
    s.appendChild(b);
  });
  return s;
}
function sel(options, get, set){
  const s = el('select', 'inp');
  options.forEach(o => { const op = el('option', '', o.n); op.value = o.v; s.appendChild(op); });
  s.value = get();
  s.onchange = () => set(s.value);
  return s;
}
function appIcon(app, size){
  const d = el('div', 'app-ico', app.glyph);
  d.style.background = app.bg;
  if (size){ d.style.width = d.style.height = size + 'px'; }
  return d;
}

/* ==========================================================================
   БЛОКНОТ
   ========================================================================== */
APPS.notepad = {
  name:'Блокнот', glyph:'📝', bg:'linear-gradient(140deg,#7dd3fc,#3b82f6)', w:760, h:560,
  render(win, opts){
    const tabsKey = 'notepad.tabs';
    let tabs = KV.get(tabsKey, [{ name:'Безымянный.txt', path:[], body:'' }]);
    let cur = 0;

    if (opts && opts.file){
      const i = tabs.findIndex(t => t.name === opts.file.name && String(t.path) === String(opts.file.path));
      if (i >= 0) cur = i;
      else { tabs.push({ name:opts.file.name, path:opts.file.path, body:opts.file.body || '' }); cur = tabs.length - 1; }
    }

    const wrap = el('div', 'app col'); wrap.style.flex = '1';
    const tabbar = el('div', 'np-tabs');
    const bar = el('div', 'toolbar');
    const area = el('textarea', 'np-area'); area.spellcheck = false;
    const status = el('div', 'statusbar');
    wrap.append(tabbar, bar, area, status);
    win.body.appendChild(wrap);

    bar.innerHTML = `
      <button class="btn" data-a="new">＋ Новый</button>
      <button class="btn" data-a="open">📂 Открыть</button>
      <button class="btn pri" data-a="save">💾 Сохранить</button>
      <div class="grow"></div>
      <span class="tiny muted">Размер</span>`;
    bar.appendChild(slider(() => KV.get('notepad.size', 14), v => { area.style.fontSize = v + 'px'; KV.set('notepad.size', v); }, 10, 28, 1, v => v + 'px'));
    const wrapT = toggle(() => KV.get('notepad.wrap', true), v => { KV.set('notepad.wrap', v); area.style.whiteSpace = v ? 'pre-wrap' : 'pre'; });
    bar.append(el('span', 'tiny muted', 'Перенос'), wrapT);

    area.style.fontSize = KV.get('notepad.size', 14) + 'px';
    area.style.whiteSpace = KV.get('notepad.wrap', true) ? 'pre-wrap' : 'pre';

    const save = () => KV.set(tabsKey, tabs);
    const renderTabs = () => {
      tabbar.innerHTML = '';
      tabs.forEach((t, i) => {
        const n = el('div', 'np-tab' + (i === cur ? ' on' : ''));
        n.innerHTML = `<span>${esc(t.name)}</span><span class="x">×</span>`;
        n.onclick = e => {
          if (e.target.classList.contains('x')){
            tabs.splice(i, 1); if (!tabs.length) tabs.push({ name:'Безымянный.txt', path:[], body:'' });
            cur = clamp(cur, 0, tabs.length - 1); save(); renderTabs(); load();
          } else { tabs[cur].body = area.value; cur = i; save(); renderTabs(); load(); }
        };
        tabbar.appendChild(n);
      });
    };
    const load = () => {
      area.value = tabs[cur].body || '';
      win.setTitle(tabs[cur].name + ' — Блокнот');
      win.setSub(tabs[cur].path && tabs[cur].path.length ? '/' + tabs[cur].path.join('/') : 'не сохранён');
      updStatus();
    };
    const updStatus = () => {
      const v = area.value;
      const ln = v.substr(0, area.selectionStart).split('\n').length;
      status.innerHTML = `<span>Стр ${ln}</span><span>Символов: ${v.length}</span>
        <span>Слов: ${v.trim() ? v.trim().split(/\s+/).length : 0}</span>
        <span class="grow"></span><span>UTF-8</span>`;
    };

    area.addEventListener('input', () => { tabs[cur].body = area.value; save(); updStatus(); });
    area.addEventListener('keyup', updStatus);
    area.addEventListener('click', updStatus);
    area.addEventListener('keydown', e => {
      if (e.key === 'Tab'){ e.preventDefault();
        const s = area.selectionStart; area.setRangeText('  ', s, area.selectionEnd, 'end'); }
      if ((e.ctrlKey || e.metaKey) && e.key === 's'){ e.preventDefault(); doSave(); }
    });

    const doSave = () => {
      const t = tabs[cur];
      if (!t.path || !t.path.length){
        const name = prompt('Имя файла:', t.name) || t.name;
        t.name = name; t.path = ['Документы'];
      }
      FS.write(t.path, t.name, area.value);
      save(); load();
      Shell.toast('Сохранено', '/' + t.path.join('/') + '/' + t.name, '💾');
    };

    bar.querySelector('[data-a="new"]').onclick = () => { tabs.push({ name:'Безымянный.txt', path:[], body:'' }); cur = tabs.length - 1; save(); renderTabs(); load(); };
    bar.querySelector('[data-a="save"]').onclick = doSave;
    bar.querySelector('[data-a="open"]').onclick = () => WM.open('files', { pick:f => {
      tabs.push({ name:f.name, path:f.path, body:f.body }); cur = tabs.length - 1; save(); renderTabs(); load(); WM.focus(win);
    }});

    renderTabs(); load();
    setTimeout(() => area.focus(), 100);
  }
};

/* ==========================================================================
   ПРОВОДНИК
   ========================================================================== */
APPS.files = {
  name:'Проводник', glyph:'📁', bg:'linear-gradient(140deg,#fcd34d,#f59e0b)', w:880, h:580,
  render(win, opts){
    let path = [], view = KV.get('files.view', 'grid'), sel = null;
    const wrap = el('div', 'app'); win.body.appendChild(wrap);
    const side = el('div', 'sidebar');
    const main = el('div', 'col grow');
    const bar = el('div', 'toolbar');
    const list = el('div', 'scroll');
    const status = el('div', 'statusbar');
    main.append(bar, list, status); wrap.append(side, main);

    const quick = [['🏠','Этот компьютер',[]],['🖥️','Рабочий стол',['Рабочий стол']],['📄','Документы',['Документы']],
                   ['🖼️','Изображения',['Изображения']],['🎵','Музыка',['Музыка']],['⬇️','Загрузки',['Загрузки']]];
    side.appendChild(el('div', 'sb-title', 'Быстрый доступ'));
    quick.forEach(([e, n, p]) => {
      const b = el('button', 'sb-item', `<span>${e}</span><span>${n}</span>`);
      b.onclick = () => go(p);
      side.appendChild(b);
    });

    bar.innerHTML = `<button class="btn" data-a="up">↑</button>
      <div class="fe-crumbs"></div>
      <button class="btn" data-a="nf">＋ Папка</button>
      <button class="btn" data-a="nt">📄 Файл</button>
      <button class="btn" data-a="view">▦</button>`;
    const crumbs = $('.fe-crumbs', bar);

    const go = p => { path = p.slice(); sel = null; draw(); };

    function draw(){
      const node = FS.node(path) || FS.root;
      win.setTitle((path.length ? path[path.length - 1] : 'Этот компьютер') + ' — Проводник');
      win.setSub('/' + path.join('/'));
      crumbs.innerHTML = '';
      const home = el('button', '', 'Этот компьютер'); home.onclick = () => go([]); crumbs.appendChild(home);
      path.forEach((p, i) => {
        crumbs.appendChild(el('span', 'muted', '›'));
        const b = el('button', '', esc(p)); b.onclick = () => go(path.slice(0, i + 1)); crumbs.appendChild(b);
      });

      const items = Object.values(node.children || {})
        .sort((a, b) => (a.type === b.type ? a.name.localeCompare(b.name) : a.type === 'dir' ? -1 : 1));
      list.className = 'scroll';
      list.innerHTML = '';
      const cont = el('div', view === 'grid' ? 'fe-grid' : 'fe-list');
      items.forEach(it => {
        const isDir = it.type === 'dir';
        const gl = isDir ? '📁' : it.img ? '🖼️' : /\.(md|txt)$/i.test(it.name) ? '📄' : '📄';
        const n = el('div', view === 'grid' ? 'fe-it' : 'fe-lr');
        n.innerHTML = view === 'grid'
          ? `<div class="gl">${gl}</div><div class="nm">${esc(it.name)}</div>`
          : `<div class="gl">${gl}</div><div>${esc(it.name)}</div><div class="sz">${isDir ? Object.keys(it.children).length + ' эл.' : ((it.body || '').length + ' Б')}</div>`;
        n.onclick = e => { e.stopPropagation(); $$('.sel', list).forEach(x => x.classList.remove('sel')); n.classList.add('sel'); sel = it; upd(); };
        n.ondblclick = () => openItem(it);
        n.oncontextmenu = e => {
          e.preventDefault(); e.stopPropagation();
          Shell.ctx(e.clientX, e.clientY, [
            { i:'📂', t:'Открыть', f:() => openItem(it) },
            { i:'✏️', t:'Переименовать', f:() => { const nn = prompt('Новое имя', it.name); if (nn) { FS.rename(path, it.name, nn); draw(); } } },
            { i:'📋', t:'Копировать имя', f:() => navigator.clipboard && navigator.clipboard.writeText(it.name) },
            'hr',
            { i:'🗑️', t:'Удалить', f:() => { if (confirm('Удалить «' + it.name + '»?')){ FS.rm(path, it.name); draw(); } } }
          ]);
        };
        cont.appendChild(n);
      });
      if (!items.length) cont.appendChild(el('div', 'empty', 'Папка пуста'));
      list.appendChild(cont);
      upd();
    }
    function upd(){
      const node = FS.node(path) || FS.root;
      status.innerHTML = `<span>Элементов: ${Object.keys(node.children || {}).length}</span>
        <span class="grow"></span><span>${sel ? esc(sel.name) : ''}</span>`;
    }
    function openItem(it){
      if (it.type === 'dir'){ go([...path, it.name]); return; }
      if (opts && opts.pick){ opts.pick({ name:it.name, path:path.slice(), body:it.body }); win.close(); return; }
      if (it.img) WM.open('photos', { img:it.img, name:it.name });
      else WM.open('notepad', { file:{ name:it.name, path:path.slice(), body:it.body } });
    }

    bar.querySelector('[data-a="up"]').onclick = () => go(path.slice(0, -1));
    bar.querySelector('[data-a="nf"]').onclick = () => { const n = prompt('Имя папки', 'Новая папка'); if (n){ FS.mkdir(path, n); draw(); } };
    bar.querySelector('[data-a="nt"]').onclick = () => { const n = prompt('Имя файла', 'Новый.txt'); if (n){ FS.write(path, n, ''); draw(); } };
    bar.querySelector('[data-a="view"]').onclick = e => { view = view === 'grid' ? 'list' : 'grid'; KV.set('files.view', view); e.target.textContent = view === 'grid' ? '▦' : '☰'; draw(); };
    list.oncontextmenu = e => { e.preventDefault();
      Shell.ctx(e.clientX, e.clientY, [
        { i:'📁', t:'Создать папку', f:() => { const n = prompt('Имя папки', 'Новая папка'); if (n){ FS.mkdir(path, n); draw(); } } },
        { i:'📄', t:'Создать файл', f:() => { const n = prompt('Имя файла', 'Новый.txt'); if (n){ FS.write(path, n, ''); draw(); } } },
        'hr', { i:'🔄', t:'Обновить', f:draw }
      ]);
    };

    go(opts && opts.path ? opts.path : ['Документы']);
  }
};

/* ==========================================================================
   КАЛЬКУЛЯТОР
   ========================================================================== */
APPS.calc = {
  name:'Калькулятор', glyph:'🧮', bg:'linear-gradient(140deg,#a5b4fc,#6366f1)', w:360, h:520, single:true,
  render(win){
    const wrap = el('div', 'app calc'); win.body.appendChild(wrap);
    const disp = el('div', 'calc-disp', '<div class="calc-hist"></div><div class="calc-val">0</div>');
    const keys = el('div', 'calc-keys');
    wrap.append(disp, keys);
    const vEl = $('.calc-val', disp), hEl = $('.calc-hist', disp);

    let cur = '0', prev = null, op = null, fresh = true;
    const fmt = n => {
      if (!isFinite(n)) return 'Ошибка';
      const s = Math.abs(n) >= 1e12 || (Math.abs(n) < 1e-9 && n !== 0) ? n.toExponential(6) : String(+n.toFixed(10));
      return s;
    };
    const show = () => { vEl.textContent = cur; hEl.textContent = prev != null ? `${fmt(prev)} ${op || ''}` : ''; };
    const calc = () => {
      const a = parseFloat(prev), b = parseFloat(cur);
      let r = b;
      if (op === '+') r = a + b; else if (op === '−') r = a - b;
      else if (op === '×') r = a * b; else if (op === '÷') r = a / b;
      else if (op === '^') r = Math.pow(a, b); else if (op === 'mod') r = a % b;
      return fmt(r);
    };
    const press = k => {
     
      if (/^[0-9]$/.test(k)){ cur = fresh || cur === '0' ? k : cur + k; fresh = false; }
      else if (k === ','){ if (fresh){ cur = '0.'; fresh = false; } else if (!cur.includes('.')) cur += '.'; }
      else if (['+','−','×','÷','^','mod'].includes(k)){
        if (op && !fresh){ cur = calc(); }
        prev = parseFloat(cur); op = k; fresh = true;
      }
      else if (k === '='){ if (op){ cur = calc(); prev = null; op = null; fresh = true; } }
      else if (k === 'C'){ cur = '0'; prev = null; op = null; fresh = true; }
      else if (k === '⌫'){ cur = cur.length > 1 ? cur.slice(0, -1) : '0'; }
      else if (k === '±'){ cur = String(-parseFloat(cur)); }
      else if (k === '%'){ cur = fmt(parseFloat(cur) / 100); }
      else if (k === '√'){ cur = fmt(Math.sqrt(parseFloat(cur))); fresh = true; }
      else if (k === 'x²'){ cur = fmt(Math.pow(parseFloat(cur), 2)); fresh = true; }
      else if (k === '1/x'){ cur = fmt(1 / parseFloat(cur)); fresh = true; }
      else if (k === 'π'){ cur = fmt(Math.PI); fresh = true; }
      show();
    };

    [['C','fn'],['±','fn'],['%','fn'],['⌫','fn'],
     ['√','fn'],['x²','fn'],['1/x','fn'],['÷','op'],
     ['7',''],['8',''],['9',''],['×','op'],
     ['4',''],['5',''],['6',''],['−','op'],
     ['1',''],['2',''],['3',''],['+','op'],
     ['π','fn'],['0',''],[',',''],['=','eq']].forEach(([k, c]) => {
      const b = el('button', c, k); b.onclick = () => press(k); keys.appendChild(b);
    });

    win.node.addEventListener('keydown', e => {
      const m = { Enter:'=', Backspace:'⌫', Escape:'C', '*':'×', '/':'÷', '-':'−', '.':',' };
      const k = m[e.key] || e.key;
      if (/^[0-9]$/.test(k) || ['+','−','×','÷','=','C','⌫',','].includes(k)){ e.preventDefault(); press(k); }
    });
    win.node.tabIndex = 0;
    show();
  }
};

/* ==========================================================================
   ТЕРМИНАЛ
   ========================================================================== */
APPS.term = {
  name:'Терминал', glyph:'⌨️', bg:'linear-gradient(140deg,#334155,#0f172a)', w:760, h:460,
  render(win){
    let cwd = [];
    const out = el('div', 'term'); win.body.appendChild(out);
    const print = (t, c = '') => { const l = el('div', 'term-line ' + c); l.innerHTML = t; out.insertBefore(l, inLine); out.scrollTop = out.scrollHeight; };
    const inLine = el('div', 'term-in');
    const prompt = el('span', 'pr');
    const inp = el('input'); inp.spellcheck = false;
    inLine.append(prompt, inp); out.appendChild(inLine);
    const upd = () => prompt.textContent = `D:\\${cwd.join('\\')}>`;

    print(`<b>Windows 12</b> [Версия 12.0.1200] — Терминал`, 'inf');
    print(`Введите <b>help</b> для списка команд.\n`, '');

    const hist = []; let hi = 0;
    const cmds = {
      help: () => print(`Доступные команды:
  help            — эта справка
  ls / dir        — содержимое папки
  cd &lt;путь&gt;       — сменить папку (.. — вверх)
  cat &lt;файл&gt;      — показать файл
  echo &lt;текст&gt;    — вывести текст
  write &lt;ф&gt; &lt;т&gt;   — записать текст в файл
  mkdir &lt;имя&gt;     — создать папку
  rm &lt;имя&gt;        — удалить
  open &lt;прил.&gt;    — открыть приложение (${Object.keys(APPS).join(', ')})
  theme dark|light — сменить тему
  accent &lt;0-9&gt;    — сменить акцент
  glass &lt;0-100&gt;   — плотность стекла
  neofetch        — о системе
  date            — дата и время
  clear           — очистить экран`),
      ls: () => { const n = FS.node(cwd);
        const items = Object.values(n.children || {});
        if (!items.length) return print('(пусто)', 'muted');
        items.forEach(i => print(i.type === 'dir' ? `<b class="inf">📁 ${esc(i.name)}</b>` : `📄 ${esc(i.name)}`)); },
      cd: a => { if (!a) return print('/' + cwd.join('/'));
        if (a === '..'){ cwd = cwd.slice(0, -1); return upd(); }
        if (a === '/' || a === '\\'){ cwd = []; return upd(); }
        const t = FS.node([...cwd, a]);
        if (t && t.type === 'dir'){ cwd = [...cwd, a]; upd(); } else print('Папка не найдена: ' + esc(a), 'er'); },
      cat: a => { const f = FS.node([...cwd, a]); f && f.type === 'file' ? print(esc(f.body || '(пусто)')) : print('Файл не найден', 'er'); },
      echo: (a, raw) => print(esc(raw)),
      write: (a, raw) => { const i = raw.indexOf(' '); if (i < 0) return print('write <файл> <текст>', 'er');
        FS.write(cwd, raw.slice(0, i), raw.slice(i + 1)); print('Записано.', 'inf'); },
      mkdir: a => FS.mkdir(cwd, a) ? print('Создано.', 'inf') : print('Ошибка', 'er'),
      rm: a => FS.rm(cwd, a) ? print('Удалено.', 'inf') : print('Не найдено', 'er'),
      open: a => APPS[a] ? (WM.open(a), print('Запуск ' + a + '...', 'inf')) : print('Нет такого приложения', 'er'),
      theme: a => { if (['dark','light'].includes(a)){ Store.set('theme', a); print('Тема: ' + a, 'inf'); } else print('theme dark|light', 'er'); },
      accent: a => { const i = +a; if (ACCENTS[i]){ S.accentCustom = null; Store.set('accent', i); print('Акцент: ' + ACCENTS[i].n, 'inf'); } else print('accent 0-' + (ACCENTS.length - 1), 'er'); },
      glass: a => { const v = clamp(+a, 0, 100) / 100; Store.set('glass', v); print('Стекло: ' + a + '%', 'inf'); },
      neofetch: () => print(`<span class="inf">      ▗▄▄▖▗▄▄▖ </span>   <b>${S.userName}@windows12</b>
<span class="inf">      ▝▀▀▘▝▀▀▘ </span>   ──────────────────
<span class="inf">      ▗▄▄▖▗▄▄▖ </span>   ОС:      Windows 12 Prototype
<span class="inf">      ▝▀▀▘▝▀▀▘ </span>   Оболочка: Liquid Glass Shell
                     Тема:    ${S.theme}
                     Стекло:  blur ${S.blur}px / ${Math.round(S.glass * 100)}%
                     Окон:    ${WM.wins.length}
                     Движок:  ${navigator.userAgent.includes('Firefox') ? 'Gecko' : 'Blink'}`),
      date: () => print(new Date().toLocaleString('ru-RU')),
      clear: () => { $$('.term-line', out).forEach(n => n.remove()); }
    };
    cmds.dir = cmds.ls;

    inp.addEventListener('keydown', e => {
      if (e.key === 'Enter'){
        const line = inp.value; inp.value = '';
        print(`<span class="pr">${prompt.textContent}</span> ${esc(line)}`);
        if (line.trim()){
          hist.push(line); hi = hist.length;
          const [c, ...rest] = line.trim().split(/\s+/);
          const fn = cmds[c.toLowerCase()];
          if (fn) { try { fn(rest[0], rest.join(' ')); } catch(err){ print(err.message, 'er'); } }
          else print(`«${esc(c)}» не является командой. Попробуйте help.`, 'er');
        }
        out.scrollTop = out.scrollHeight;
      }
      else if (e.key === 'ArrowUp'){ e.preventDefault(); if (hi > 0) inp.value = hist[--hi] || ''; }
      else if (e.key === 'ArrowDown'){ e.preventDefault(); hi = Math.min(hist.length, hi + 1); inp.value = hist[hi] || ''; }
      else if (e.key === 'l' && e.ctrlKey){ e.preventDefault(); cmds.clear(); }
    });
    out.onclick = () => inp.focus();
    upd(); setTimeout(() => inp.focus(), 120);
  }
};

/* ==========================================================================
   PAINT
   ========================================================================== */
APPS.paint = {
  name:'Paint', glyph:'🎨', bg:'linear-gradient(140deg,#f9a8d4,#a855f7)', w:900, h:620,
  render(win){
    const wrap = el('div', 'app'); win.body.appendChild(wrap);
    const tools = el('div', 'paint-tools');
    const right = el('div', 'col grow');
    const bar = el('div', 'toolbar');
    const cw = el('div', 'paint-canvas-wrap');
    const cv = el('canvas'); cv.id = 'paint-canvas'; cv.width = 900; cv.height = 560;
    cw.appendChild(cv); right.append(bar, cw); wrap.append(tools, right);

    const ctx = cv.getContext('2d');
    ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, cv.width, cv.height);
    ctx.lineCap = ctx.lineJoin = 'round';

    let tool = 'pen', color = '#3a86ff', size = 5, drawing = false, sx = 0, sy = 0, snap = null;
    const undoStack = [];
    const pushUndo = () => { undoStack.push(ctx.getImageData(0, 0, cv.width, cv.height)); if (undoStack.length > 25) undoStack.shift(); };

    [['pen','✏️'],['brush','🖌️'],['eraser','🧽'],['line','📏'],['rect','▭'],['circle','◯'],['fill','🪣'],['spray','💨']]
      .forEach(([t, e]) => {
        const b = el('button', 'pt' + (t === tool ? ' on' : ''), e); b.dataset.t = t;
        b.onclick = () => { tool = t; $$('.pt', tools).forEach(x => x.classList.remove('on')); b.classList.add('on'); };
        tools.appendChild(b);
      });

    const colors = ['#000000','#ffffff','#ef4444','#f97316','#facc15','#22c55e','#06b6d4','#3a86ff','#8b5cf6','#ec4899','#78350f','#64748b'];
    const sw = el('div', 'swatches');
    colors.forEach(c => { const s = el('div', 'sw' + (c === color ? ' on' : '')); s.style.background = c;
      s.onclick = () => { color = c; $$('.sw', sw).forEach(x => x.classList.remove('on')); s.classList.add('on'); }; sw.appendChild(s); });
    const cpick = el('input'); cpick.type = 'color'; cpick.value = color; cpick.className = 'inp'; cpick.style.width = '44px'; cpick.style.padding = '2px';
    cpick.oninput = () => { color = cpick.value; $$('.sw', sw).forEach(x => x.classList.remove('on')); };
    bar.append(sw, cpick, el('span', 'tiny muted', 'Толщина'));
    bar.appendChild(slider(() => size, v => size = v, 1, 40, 1, v => v + 'px'));
    const undoB = el('button', 'btn', '↶ Отмена'), clrB = el('button', 'btn', '🗑 Очистить'), saveB = el('button', 'btn pri', '💾 В файлы');
    bar.append(el('div', 'grow'), undoB, clrB, saveB);
    undoB.onclick = () => { const d = undoStack.pop(); if (d) ctx.putImageData(d, 0, 0); };
    clrB.onclick = () => { pushUndo(); ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, cv.width, cv.height); };
    saveB.onclick = () => {
      const name = prompt('Имя изображения', 'Рисунок.png'); if (!name) return;
      const d = FS.node(['Изображения']); d.children[name] = { type:'file', name, img:cv.toDataURL(), body:'' }; FS.save();
      Shell.toast('Сохранено', 'Изображения/' + name, '🖼️');
    };

    const pos = e => { const r = cv.getBoundingClientRect(); return [ (e.clientX - r.left) * cv.width / r.width, (e.clientY - r.top) * cv.height / r.height ]; };
    const flood = (x, y, hex) => {
      const img = ctx.getImageData(0, 0, cv.width, cv.height), d = img.data;
      const idx = (x, y) => (y * cv.width + x) * 4;
      const t = idx(x | 0, y | 0), tc = [d[t], d[t + 1], d[t + 2], d[t + 3]];
      const n = parseInt(hex.slice(1), 16), nc = [(n >> 16) & 255, (n >> 8) & 255, n & 255, 255];
      if (tc.join() === nc.join()) return;
      const st = [[x | 0, y | 0]];
      while (st.length){
        const [cx, cy] = st.pop(); if (cx < 0 || cy < 0 || cx >= cv.width || cy >= cv.height) continue;
        const i = idx(cx, cy);
        if (Math.abs(d[i] - tc[0]) + Math.abs(d[i+1] - tc[1]) + Math.abs(d[i+2] - tc[2]) > 30) continue;
        d[i] = nc[0]; d[i+1] = nc[1]; d[i+2] = nc[2]; d[i+3] = 255;
        st.push([cx+1, cy], [cx-1, cy], [cx, cy+1], [cx, cy-1]);
      }
      ctx.putImageData(img, 0, 0);
    };

    cv.addEventListener('mousedown', e => {
      pushUndo();
      const [x, y] = pos(e); sx = x; sy = y; drawing = true;
      ctx.strokeStyle = tool === 'eraser' ? '#fff' : color;
      ctx.fillStyle = tool === 'eraser' ? '#fff' : color;
      ctx.lineWidth = tool === 'brush' ? size * 2 : size;
      ctx.globalAlpha = tool === 'brush' ? .35 : 1;
      if (tool === 'fill'){ flood(x, y, color); drawing = false; return; }
      if (['line','rect','circle'].includes(tool)) snap = ctx.getImageData(0, 0, cv.width, cv.height);
      else { ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + .1, y); ctx.stroke(); }
    });
    cv.addEventListener('mousemove', e => {
      if (!drawing) return;
      const [x, y] = pos(e);
      if (['pen','brush','eraser'].includes(tool)){ ctx.lineTo(x, y); ctx.stroke(); }
      else if (tool === 'spray'){ for (let i = 0; i < 12; i++){ const a = Math.random() * 6.28, r = Math.random() * size * 1.6;
          ctx.fillRect(x + Math.cos(a) * r, y + Math.sin(a) * r, 1.4, 1.4); } }
      else { ctx.putImageData(snap, 0, 0); ctx.beginPath();
        if (tool === 'line'){ ctx.moveTo(sx, sy); ctx.lineTo(x, y); }
        else if (tool === 'rect') ctx.rect(sx, sy, x - sx, y - sy);
        else ctx.arc(sx, sy, Math.hypot(x - sx, y - sy), 0, 6.29);
        ctx.stroke(); }
    });
    addEventListener('mouseup', () => { if (drawing){ drawing = false; ctx.globalAlpha = 1; ctx.beginPath(); } });
  }
};

/* ==========================================================================
   ФОТО
   ========================================================================== */
APPS.photos = {
  name:'Фотографии', glyph:'🖼️', bg:'linear-gradient(140deg,#6ee7b7,#0ea5e9)', w:820, h:580,
  render(win, opts){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const bar = el('div', 'toolbar', '<b style="font-size:13px">Коллекция</b><div class="grow"></div>');
    const grid = el('div', 'ph-grid scroll');
    wrap.append(bar, grid);

    const draw = () => {
      grid.innerHTML = '';
      WALLPAPERS.forEach(w => {
        const p = el('div', 'ph'); p.style.backgroundImage = w.css; p.title = w.name;
        p.onclick = () => view(w.css, w.name);
        p.oncontextmenu = e => { e.preventDefault(); Shell.ctx(e.clientX, e.clientY, [
          { i:'🖥️', t:'Сделать обоями', f:() => { Store.set('wallpaper', w.id); Shell.toast('Обои изменены', w.name, '🖼️'); } }]); };
        grid.appendChild(p);
      });
      const dir = FS.node(['Изображения']);
      Object.values(dir.children || {}).filter(f => f.img).forEach(f => {
        const p = el('div', 'ph'); p.style.backgroundImage = `url(${f.img})`; p.title = f.name;
        p.onclick = () => view(`url(${f.img})`, f.name);
        grid.appendChild(p);
      });
    };
    function view(css, name){
      const v = el('div', 'ph-view');
      v.innerHTML = `<div class="big"></div><button class="ph-close">×</button>`;
      $('.big', v).style.backgroundImage = css;
      $('.ph-close', v).onclick = () => v.remove();
      v.onclick = e => { if (e.target === v) v.remove(); };
      win.body.appendChild(v);
      win.setSub(name);
    }
    draw();
    if (opts && opts.img) view(`url(${opts.img})`, opts.name || '');
  }
};

/* ==========================================================================
   МУЗЫКА (реальный синтезатор WebAudio)
   ========================================================================== */
APPS.music = {
  name:'Музыка', glyph:'🎵', bg:'linear-gradient(140deg,#fb7185,#a855f7)', w:520, h:600, single:true,
  render(win){
    const TRACKS = [
      { t:'Liquid Dreams', a:'Dymensity', scale:[0,3,5,7,10], root:220, bpm:96, e:'🌊' },
      { t:'Glass Horizon', a:'Aurora Fields', scale:[0,2,4,7,9], root:261.6, bpm:112, e:'🪟' },
      { t:'Night Shift',   a:'Mono Lake',    scale:[0,2,3,7,8], root:196, bpm:84, e:'🌙' },
      { t:'Sunrise Boot',  a:'Kernel Panic', scale:[0,4,7,11,14], root:293.7, bpm:124, e:'🌅' }
    ];
    let i = 0, playing = false, timer = null, step = 0, prog = 0, progTimer = null;
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const mu = el('div', 'mu');
    const list = el('div', 'scroll'); list.style.maxHeight = '150px'; list.style.padding = '0 14px 12px';
    wrap.append(mu, list);
    mu.innerHTML = `<div class="mu-art">🎵</div>
      <div style="text-align:center"><div class="mu-title"></div><div class="mu-artist"></div></div>
      <div class="mu-bar"><i></i></div>
      <div class="mu-ctl">
        <button data-a="prev">⏮</button><button class="pp" data-a="pp">▶</button><button data-a="next">⏭</button>
      </div>
      <div class="mu-viz"></div>`;
    const viz = $('.mu-viz', mu);
    for (let k = 0; k < 40; k++) viz.appendChild(el('i'));
    const bars = $$('i', viz);

    const ac = () => Snd.ac();
    let master = null, analyser = null;
    const ensure = () => {
      const c = ac(); if (!c) return null;
      if (!master){
        master = c.createGain(); master.gain.value = 0.16;
        analyser = c.createAnalyser(); analyser.fftSize = 128;
        master.connect(analyser); analyser.connect(c.destination);
      }
      return c;
    };
    const note = (f, dur, type = 'triangle', g = .5) => {
      const c = ensure(); if (!c) return;
      const o = c.createOscillator(), gg = c.createGain();
      o.type = type; o.frequency.value = f;
      gg.gain.setValueAtTime(0, c.currentTime);
      gg.gain.linearRampToValueAtTime(g * (S.volume / 100), c.currentTime + .02);
      gg.gain.exponentialRampToValueAtTime(.0001, c.currentTime + dur);
      o.connect(gg).connect(master); o.start(); o.stop(c.currentTime + dur + .05);
    };
    const tick = () => {
      const tr = TRACKS[i];
      const deg = tr.scale[step % tr.scale.length];
      const oct = Math.floor(step / tr.scale.length) % 3;
      note(tr.root * Math.pow(2, (deg + oct * 12) / 12), .55, 'triangle', .45);
      if (step % 4 === 0) note(tr.root / 2, .8, 'sine', .5);
      if (step % 8 === 3) note(tr.root * 3, .18, 'square', .12);
      step++;
    };
    const drawViz = () => {
      if (!analyser){ requestAnimationFrame(drawViz); return; }
      const d = new Uint8Array(analyser.frequencyBinCount);
      analyser.getByteFrequencyData(d);
      bars.forEach((b, k) => { b.style.height = Math.max(6, (d[k % d.length] / 255) * 100) + '%'; });
      if (win.node.isConnected) requestAnimationFrame(drawViz);
    };
    const load = () => {
      const tr = TRACKS[i];
      $('.mu-title', mu).textContent = tr.t;
      $('.mu-artist', mu).textContent = tr.a;
      $('.mu-art', mu).textContent = tr.e;
      win.setTitle(tr.t + ' — Музыка');
      prog = 0; step = 0;
      $$('.td-item', list).forEach((n, k) => n.classList.toggle('done', k === i));
      Shell.nowPlaying = playing ? tr : null;
      Shell.updateCC && Shell.updateCC();
    };
    const play = v => {
      playing = v;
      $('[data-a="pp"]', mu).textContent = v ? '⏸' : '▶';
      $('.mu-art', mu).classList.toggle('play', v);
      clearInterval(timer); clearInterval(progTimer);
      if (v){
        ensure();
        const ms = 60000 / TRACKS[i].bpm / 2;
        tick(); timer = setInterval(tick, ms);
        progTimer = setInterval(() => { prog = (prog + .4) % 100; $('.mu-bar i', mu).style.width = prog + '%'; }, 120);
      }
      Shell.nowPlaying = v ? TRACKS[i] : null;
      Shell.updateCC && Shell.updateCC();
    };
    $('[data-a="pp"]', mu).onclick = () => play(!playing);
    $('[data-a="next"]', mu).onclick = () => { i = (i + 1) % TRACKS.length; load(); if (playing) play(true); };
    $('[data-a="prev"]', mu).onclick = () => { i = (i - 1 + TRACKS.length) % TRACKS.length; load(); if (playing) play(true); };
    $('.mu-bar', mu).onclick = e => { const r = e.currentTarget.getBoundingClientRect();
      prog = (e.clientX - r.left) / r.width * 100; $('.mu-bar i', mu).style.width = prog + '%'; };

    TRACKS.forEach((t, k) => {
      const n = el('div', 'td-item');
      n.innerHTML = `<span style="font-size:17px">${t.e}</span><div class="tx">${t.t}<br><small class="muted">${t.a}</small></div><span class="tiny muted">${t.bpm} BPM</span>`;
      n.onclick = () => { i = k; load(); play(true); };
      list.appendChild(n);
    });

    win.onClose = () => { clearInterval(timer); clearInterval(progTimer); Shell.nowPlaying = null; };
    APPS.music.playPause = () => play(!playing);
    load(); drawViz();
  }
};

/* ==========================================================================
   КАЛЕНДАРЬ
   ========================================================================== */
APPS.calendar = {
  name:'Календарь', glyph:'📅', bg:'linear-gradient(140deg,#fca5a5,#ef4444)', w:760, h:600,
  render(win){
    let cur = new Date();
    const events = KV.get('cal.events', {});
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const head = el('div', 'cal-head');
    const grid = el('div', 'cal-grid scroll');
    wrap.append(head, grid);

    const M = ['Январь','Февраль','Март','Апрель','Май','Июнь','Июль','Август','Сентябрь','Октябрь','Ноябрь','Декабрь'];
    const draw = () => {
      head.innerHTML = '';
      const t = el('div', 'cal-m', `${M[cur.getMonth()]} ${cur.getFullYear()}`);
      const prev = el('button', 'btn', '‹'), next = el('button', 'btn', '›'), today = el('button', 'btn', 'Сегодня');
      prev.onclick = () => { cur = new Date(cur.getFullYear(), cur.getMonth() - 1, 1); draw(); };
      next.onclick = () => { cur = new Date(cur.getFullYear(), cur.getMonth() + 1, 1); draw(); };
      today.onclick = () => { cur = new Date(); draw(); };
      head.append(t, el('div', 'grow'), prev, today, next);

      grid.innerHTML = '';
      ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'].forEach(d => grid.appendChild(el('div', 'cal-dow', d)));
      const first = new Date(cur.getFullYear(), cur.getMonth(), 1);
      const start = (first.getDay() + 6) % 7;
      const days = new Date(cur.getFullYear(), cur.getMonth() + 1, 0).getDate();
      const pdays = new Date(cur.getFullYear(), cur.getMonth(), 0).getDate();
      const now = new Date();
      for (let k = 0; k < 42; k++){
        let d, mo = cur.getMonth(), yr = cur.getFullYear(), out = false;
        if (k < start){ d = pdays - start + k + 1; mo--; out = true; }
        else if (k - start < days){ d = k - start + 1; }
        else { d = k - start - days + 1; mo++; out = true; }
        const key = `${yr}-${mo}-${d}`;
        const isToday = !out && d === now.getDate() && mo === now.getMonth() && yr === now.getFullYear();
        const c = el('div', 'cal-d' + (out ? ' out' : '') + (isToday ? ' today' : ''), `<b>${d}</b>`);
        if (events[key]) { c.appendChild(el('div', 'ev')); c.title = events[key]; c.innerHTML += `<div class="tiny" style="margin-top:3px;opacity:.8">${esc(events[key]).slice(0, 14)}</div>`; }
        c.onclick = () => {
          const v = prompt('Событие на ' + d + ' ' + M[(mo + 12) % 12], events[key] || '');
          if (v === null) return;
          if (v) events[key] = v; else delete events[key];
          KV.set('cal.events', events); draw();
        };
        grid.appendChild(c);
      }
    };
    draw();
  }
};

/* ==========================================================================
   ЧАСЫ
   ========================================================================== */
APPS.clock = {
  name:'Часы', glyph:'🕐', bg:'linear-gradient(140deg,#93c5fd,#1d4ed8)', w:640, h:540,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const tabs = el('div', 'toolbar');
    const body = el('div', 'scroll pad');
    wrap.append(tabs, body);
    let tab = 'clock';
    ['Часы','Секундомер','Таймер','Мир'].forEach((n, i) => {
      const k = ['clock','sw','timer','world'][i];
      const b = el('button', 'btn' + (k === tab ? ' pri' : ''), n);
      b.onclick = () => { tab = k; $$('.btn', tabs).forEach(x => x.classList.remove('pri')); b.classList.add('pri'); draw(); };
      tabs.appendChild(b);
    });

    let iv = null, swT = 0, swRun = false, swIv = null, tmLeft = 0, tmIv = null;
    const stopAll = () => { clearInterval(iv); iv = null; };
    win.onClose = () => { clearInterval(iv); clearInterval(swIv); clearInterval(tmIv); };

    function draw(){
      stopAll(); body.innerHTML = '';
      if (tab === 'clock'){
        body.innerHTML = `<div class="clock-face"><div class="hand h"></div><div class="hand m"></div><div class="hand s"></div><div class="clock-pin"></div></div>
          <div class="big-num" id="ck-d" style="margin-top:18px"></div>
          <div style="text-align:center" class="muted" id="ck-dt"></div>`;
        const up = () => {
          const n = new Date();
          const h = $('.hand.h', body), m = $('.hand.m', body), s = $('.hand.s', body);
          h.style.transform = `rotate(${(n.getHours() % 12) * 30 + n.getMinutes() * .5}deg)`;
          m.style.transform = `rotate(${n.getMinutes() * 6 + n.getSeconds() * .1}deg)`;
          s.style.transform = `rotate(${n.getSeconds() * 6}deg)`;
          $('#ck-d', body).textContent = `${pad2(n.getHours())}:${pad2(n.getMinutes())}:${pad2(n.getSeconds())}`;
          $('#ck-dt', body).textContent = n.toLocaleDateString('ru-RU', { weekday:'long', day:'numeric', month:'long', year:'numeric' });
        };
        up(); iv = setInterval(up, 1000);
      }
      else if (tab === 'sw'){
        body.innerHTML = `<div class="big-num" id="sw">00:00.00</div>
          <div class="row" style="justify-content:center;gap:10px;margin-top:18px">
            <button class="btn pri" id="sw-s">Старт</button><button class="btn" id="sw-r">Сброс</button></div>
          <div id="laps" style="margin-top:16px"></div>`;
        const fmt = ms => `${pad2(Math.floor(ms / 60000))}:${pad2(Math.floor(ms / 1000) % 60)}.${pad2(Math.floor(ms / 10) % 100)}`;
        const upd = () => $('#sw', body).textContent = fmt(swT);
        upd();
        $('#sw-s', body).onclick = e => {
          swRun = !swRun; e.target.textContent = swRun ? 'Пауза' : 'Старт';
          e.target.classList.toggle('pri', !swRun);
          clearInterval(swIv);
          if (swRun){ const t0 = Date.now() - swT; swIv = setInterval(() => { swT = Date.now() - t0; upd(); }, 31); }
        };
        $('#sw-r', body).onclick = () => { swT = 0; upd(); };
      }
      else if (tab === 'timer'){
        body.innerHTML = `<div class="big-num" id="tm">00:00</div>
          <div class="row" style="justify-content:center;gap:8px;margin-top:14px">
            ${[1,3,5,10,25].map(m => `<button class="btn" data-m="${m}">${m} мин</button>`).join('')}
          </div>
          <div class="row" style="justify-content:center;gap:10px;margin-top:16px">
            <button class="btn pri" id="tm-s">Старт</button><button class="btn" id="tm-r">Сброс</button></div>`;
        const upd = () => $('#tm', body).textContent = `${pad2(Math.floor(tmLeft / 60))}:${pad2(tmLeft % 60)}`;
        upd();
        $$('[data-m]', body).forEach(b => b.onclick = () => { tmLeft = +b.dataset.m * 60; upd(); });
        $('#tm-s', body).onclick = () => {
          clearInterval(tmIv);
          tmIv = setInterval(() => {
            if (tmLeft <= 0){ clearInterval(tmIv); Shell.toast('Таймер', 'Время вышло!', '⏰'); Snd.note(); return; }
            tmLeft--; upd();
          }, 1000);
        };
        $('#tm-r', body).onclick = () => { clearInterval(tmIv); tmLeft = 0; upd(); };
      }
      else {
        const zones = [['Москва',3],['Лондон',0],['Нью-Йорк',-5],['Токио',9],['Дубай',4],['Сан-Франциско',-8],['Берлин',1],['Сидней',11]];
        const upd = () => {
          body.innerHTML = zones.map(([n, off]) => {
            const d = new Date(Date.now() + (off * 60 + new Date().getTimezoneOffset()) * 60000);
            return `<div class="wclock"><b>${n}</b><span style="font-variant-numeric:tabular-nums">${pad2(d.getHours())}:${pad2(d.getMinutes())}</span></div>`;
          }).join('');
        };
        upd(); iv = setInterval(upd, 1000);
      }
    }
    draw();
  }
};

/* ==========================================================================
   БРАУЗЕР
   ========================================================================== */
APPS.browser = {
  name:'Браузер', glyph:'🌐', bg:'linear-gradient(140deg,#38bdf8,#0369a1)', w:960, h:620,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const bar = el('div', 'br-bar');
    const page = el('div', 'br-page');
    wrap.append(bar, page);
    bar.innerHTML = `<button class="btn" data-a="back">‹</button><button class="btn" data-a="fwd">›</button>
      <button class="btn" data-a="rl">⟳</button>
      <div class="br-url">🔒 <input value="dymensity://home"></div>
      <button class="btn" data-a="go">→</button>`;
    const inp = $('input', bar);
    const hist = []; let hp = -1;

    const PAGES = {
      'dymensity://home': () => `
        <div class="br-hero"><h1>Dymensity</h1><p class="muted">Внутренняя сеть прототипа Windows 12</p></div>
        <div class="br-tiles">
          ${[['📰','Новости','dymensity://news'],['📚','Документация','dymensity://docs'],['🎨','Галерея','dymensity://gallery'],
             ['⚙️','О системе','dymensity://about'],['🧪','Тест стекла','dymensity://glass'],['🕹','Игра','dymensity://game']]
            .map(([e, n, u]) => `<div class="br-tile" data-u="${u}"><span class="e">${e}</span>${n}</div>`).join('')}
        </div>`,
      'dymensity://news': () => `<h2>Новости</h2>
        <p class="muted">Лента прототипа — статический контент.</p>
        ${[['Windows 12: плавающая оболочка','Панель задач отделилась от края экрана и превратилась в док.'],
           ['Liquid Glass везде','Материал реагирует на курсор бликом и лёгкой дисперсией по кромке.'],
           ['Снап-зоны','Потяните окно к краю экрана — появится подсказка размещения.']]
          .map(([t, d]) => `<div class="card" style="padding:16px"><b>${t}</b><div class="muted" style="margin-top:6px">${d}</div></div>`).join('')}`,
      'dymensity://docs': () => `<h2>Горячие клавиши</h2>
        <div class="card" style="padding:8px 0">
        ${[['Win / Meta','Меню Пуск'],['Win + Space','Поиск'],['Win + Tab','Просмотр задач'],['Win + D','Свернуть всё'],
           ['Win + ←/→','Прилипание окна'],['Win + ↑','Развернуть'],['Ctrl + Alt + ←/→','Рабочие столы'],
           ['Ctrl + W','Закрыть окно'],['Esc','Закрыть панель']]
          .map(([k, v]) => `<div class="set-row"><div class="l"><b>${k}</b></div><div class="ctl muted">${v}</div></div>`).join('')}</div>`,
      'dymensity://gallery': () => `<h2>Галерея</h2><div class="ph-grid" style="padding:0">
        ${WALLPAPERS.map(w => `<div class="ph" style="background-image:${w.css}"></div>`).join('')}</div>`,
      'dymensity://about': () => `<div class="about-logo"><div class="win-logo"><i></i><i></i><i></i><i></i></div>
        <h2 style="margin:0">Windows 12 Prototype</h2><p class="muted">HTML + CSS + JS, без внешних зависимостей</p></div>`,
      'dymensity://glass': () => `<h2>Тест материала</h2>
        <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:14px;margin-top:16px">
        ${[0,1,2,3,4,5].map(i => `<div class="glass lg" style="height:110px;border-radius:18px;background-image:${WALLPAPERS[i].css};opacity:.9"></div>`).join('')}</div>`,
      'dymensity://game': () => `<h2>Угадай число</h2><p class="muted">От 1 до 100</p>
        <div class="row" style="margin-top:12px"><input class="inp" id="g-in" placeholder="Ваш вариант"><button class="btn pri" id="g-go">Проверить</button></div>
        <div id="g-out" style="margin-top:14px"></div>`
    };

    const nav = (url, push = true) => {
      url = url.trim();
      if (!url.includes('://')) url = 'dymensity://' + url.replace(/^\/+/, '');
      inp.value = url;
      const fn = PAGES[url];
      page.style.opacity = '0';
      setTimeout(() => {
        page.innerHTML = fn ? fn() : `<div class="br-hero"><h1>🚫</h1><h2>Сайт недоступен</h2>
          <p class="muted">Прототип работает только с внутренней сетью <b>dymensity://</b></p>
          <button class="btn pri" onclick="this.closest('.br-page').previousElementSibling.querySelector('input').value='dymensity://home'">Вернуться</button></div>`;
        page.style.opacity = '1';
        $$('.br-tile', page).forEach(t => t.onclick = () => nav(t.dataset.u));
        const gg = $('#g-go', page);
        if (gg){
          let target = 1 + Math.floor(Math.random() * 100), tries = 0;
          gg.onclick = () => {
            const v = +$('#g-in', page).value; tries++;
            $('#g-out', page).innerHTML = v === target ? `<b style="color:#4ade80">Верно! Попыток: ${tries}</b>`
              : v < target ? 'Больше ↑' : 'Меньше ↓';
            if (v === target){ target = 1 + Math.floor(Math.random() * 100); tries = 0; }
          };
        }
        win.setSub(url);
      }, 120);
      page.style.transition = 'opacity .12s';
      if (push){ hist.splice(hp + 1); hist.push(url); hp = hist.length - 1; }
    };
    bar.querySelector('[data-a="back"]').onclick = () => { if (hp > 0) nav(hist[--hp], false); };
    bar.querySelector('[data-a="fwd"]').onclick = () => { if (hp < hist.length - 1) nav(hist[++hp], false); };
    bar.querySelector('[data-a="rl"]').onclick = () => nav(inp.value, false);
    bar.querySelector('[data-a="go"]').onclick = () => nav(inp.value);
    inp.onkeydown = e => { if (e.key === 'Enter') nav(inp.value); };
    nav('dymensity://home');
  }
};

/* ==========================================================================
   ЗАДАЧИ
   ========================================================================== */
APPS.todo = {
  name:'Задачи', glyph:'✅', bg:'linear-gradient(140deg,#86efac,#16a34a)', w:520, h:560, single:true,
  render(win){
    let items = KV.get('todo', [{ t:'Открыть Параметры → Liquid Glass', d:false }, { t:'Порисовать в Paint', d:false }]);
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const bar = el('div', 'toolbar');
    const inp = el('input', 'inp grow'); inp.placeholder = 'Новая задача…';
    const add = el('button', 'btn pri', '＋');
    bar.append(inp, add);
    const list = el('div', 'scroll pad');
    wrap.append(bar, list);
    const save = () => KV.set('todo', items);
    const draw = () => {
      list.innerHTML = '';
      if (!items.length) list.appendChild(el('div', 'empty', 'Задач нет 🎉'));
      items.forEach((it, i) => {
        const n = el('div', 'td-item' + (it.d ? ' done' : ''));
        n.style.setProperty('--i', i);
        n.innerHTML = `<div class="td-box">✓</div><div class="tx">${esc(it.t)}</div><button class="td-del">×</button>`;
        $('.td-box', n).onclick = () => { it.d = !it.d; save(); draw(); };
        $('.td-del', n).onclick = () => { items.splice(i, 1); save(); draw(); };
        list.appendChild(n);
      });
      win.setSub(`${items.filter(i => !i.d).length} активных`);
    };
    const doAdd = () => { if (!inp.value.trim()) return; items.unshift({ t:inp.value.trim(), d:false }); inp.value = ''; save(); draw(); };
    add.onclick = doAdd;
    inp.onkeydown = e => { if (e.key === 'Enter') doAdd(); };
    draw();
  }
};

/* ==========================================================================
   ДИСПЕТЧЕР ЗАДАЧ
   ========================================================================== */
APPS.taskmgr = {
  name:'Диспетчер задач', glyph:'📊', bg:'linear-gradient(140deg,#cbd5e1,#475569)', w:640, h:480, single:true,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const list = el('div', 'scroll'); wrap.appendChild(list);
    const load = new Map();
    const draw = () => {
      const rows = WM.wins.map(w => {
        if (!load.has(w.id)) load.set(w.id, { c:2 + Math.random() * 6, m:60 + Math.random() * 240 });
        const L = load.get(w.id);
        L.c = clamp(L.c + (Math.random() - .5) * 2, .2, 34);
        return { w, L };
      });
      list.innerHTML = `<div class="tm-row head"><div>Процесс</div><div>ЦП</div><div>Память</div><div></div></div>`;
      rows.forEach(({ w, L }) => {
        const r = el('div', 'tm-row');
        r.innerHTML = `<div>${w.app.glyph} ${esc(w.app.name)}<div class="bar"><i style="width:${L.c * 3}%"></i></div></div>
          <div>${L.c.toFixed(1)}%</div><div>${L.m.toFixed(0)} МБ</div><div><button class="btn">Снять</button></div>`;
        $('.btn', r).onclick = () => { WM.close(w); draw(); };
        list.appendChild(r);
      });
      const tc = rows.reduce((s, r) => s + r.L.c, 0), tm = rows.reduce((s, r) => s + r.L.m, 0);
      win.setSub(`ЦП ${tc.toFixed(0)}% · Память ${(tm / 1024).toFixed(1)} ГБ`);
      if (!rows.length) list.appendChild(el('div', 'empty', 'Нет запущенных приложений'));
    };
    const iv = setInterval(draw, 1200); draw();
    win.onClose = () => clearInterval(iv);
  }
};

/* ==========================================================================
   МАГАЗИН
   ========================================================================== */
APPS.store = {
  name:'Магазин', glyph:'🛍️', bg:'linear-gradient(140deg,#c4b5fd,#7c3aed)', w:820, h:600,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const s = el('div', 'scroll pad'); wrap.appendChild(s);
    s.innerHTML = `<div class="st-hero"><h2 style="margin:0 0 6px">Microsoft Store</h2>
      <div style="opacity:.85">Все приложения этого прототипа уже установлены</div></div>
      <div class="card-t">Приложения системы</div><div class="st-grid"></div>`;
    const g = $('.st-grid', s);
    Object.entries(APPS).forEach(([id, a]) => {
      const c = el('div', 'st-card');
      c.append(appIcon(a, 52));
      c.appendChild(el('div', '', `<b style="font-size:12.5px">${esc(a.name)}</b><br><small class="muted">Microsoft</small>`));
      const b = el('button', 'btn pri', S.pinned.includes(id) ? 'Открыть' : 'Закрепить');
      b.onclick = () => {
        if (S.pinned.includes(id)) WM.open(id);
        else { S.pinned.push(id); Store.save(); Shell.renderStart(); b.textContent = 'Открыть'; Shell.toast('Закреплено', a.name + ' в меню Пуск', a.glyph); }
      };
      c.appendChild(b);
      g.appendChild(c);
    });
  }
};

/* ==========================================================================
   ПАРАМЕТРЫ
   ========================================================================== */
APPS.settings = {
  name:'Параметры', glyph:'⚙️', bg:'linear-gradient(140deg,#94a3b8,#334155)', w:980, h:660, single:true,
  render(win, opts){
    const wrap = el('div', 'app'); win.body.appendChild(wrap);
    const side = el('div', 'sidebar');
    const main = el('div', 'scroll pad'); main.style.flex = '1';
    wrap.append(side, main);

    const SECTIONS = [
      { id:'home',   n:'Главная',                  e:'🏠' },
      { id:'system', n:'Система',                  e:'🖥️' },
      { id:'person', n:'Персонализация',           e:'🎨' },
      { id:'glass',  n:'Liquid Glass',             e:'🫧' },
      { id:'dock',   n:'Рабочий стол и док',       e:'🧱' },
      { id:'motion', n:'Движение и анимации',      e:'🌀' },
      { id:'sound',  n:'Звук',                     e:'🔊' },
      { id:'display',n:'Дисплей',                  e:'💡' },
      { id:'net',    n:'Сеть и Интернет',          e:'📶' },
      { id:'bt',     n:'Bluetooth и устройства',   e:'🎧' },
      { id:'notif',  n:'Уведомления и фокус',      e:'🔔' },
      { id:'apps',   n:'Приложения',               e:'📦' },
      { id:'acc',    n:'Учётные записи',           e:'👤' },
      { id:'time',   n:'Время и язык',             e:'🌍' },
      { id:'a11y',   n:'Спец. возможности',        e:'♿' },
      { id:'privacy',n:'Конфиденциальность',       e:'🔐' },
      { id:'update', n:'Центр обновления',         e:'🔄' },
      { id:'about',  n:'О системе',                e:'ℹ️' }
    ];
    let cur = (opts && opts.section) || 'home';

    const search = el('div', 'set-search');
    const si = el('input', 'inp'); si.placeholder = '🔎 Найти параметр'; si.style.width = '100%';
    search.appendChild(si); side.appendChild(search);
    const navBox = el('div'); side.appendChild(navBox);

    const KEYS = {
      glass:'стекло размытие прозрачность blur материал', person:'обои тема цвет акцент шрифт',
      dock:'док панель задач иконки', motion:'анимации скорость плавность', sound:'звук громкость',
      display:'яркость масштаб ночной свет', a11y:'доступность контраст курсор', about:'версия система'
    };
    const drawNav = (filter = '') => {
      navBox.innerHTML = '';
      SECTIONS.filter(s => !filter || s.n.toLowerCase().includes(filter) || (KEYS[s.id] || '').includes(filter))
        .forEach(s => {
          const b = el('button', 'sb-item' + (s.id === cur ? ' on' : ''), `<span>${s.e}</span><span>${s.n}</span>`);
          b.onclick = () => { cur = s.id; drawNav(si.value.toLowerCase()); drawMain(); };
          navBox.appendChild(b);
        });
    };
    si.oninput = () => drawNav(si.value.toLowerCase().trim());

    const set = (k, v) => Store.set(k, v);

    function drawMain(){
      main.innerHTML = '';
      const sec = SECTIONS.find(s => s.id === cur);
      win.setSub(sec.n);
      main.appendChild(el('h2', '', sec.e + ' ' + sec.n)).style.cssText = 'margin:0 0 16px;font-size:22px;font-weight:600';
      ({ home:pHome, system:pSystem, person:pPerson, glass:pGlass, dock:pDock, motion:pMotion, sound:pSound,
         display:pDisplay, net:pNet, bt:pBt, notif:pNotif, apps:pApps, acc:pAcc, time:pTime, a11y:pA11y,
         privacy:pPrivacy, update:pUpdate, about:pAbout })[cur]();
      main.scrollTop = 0;
    }

    /* --- Главная --- */
    function pHome(){
      const hero = el('div', 'set-hero');
      hero.innerHTML = `<div class="ava">${S.userName[0]}</div>
        <div><b style="font-size:17px">${esc(S.userName)}</b><div class="muted tiny">Локальная учётная запись · Windows 12 Pro</div></div>`;
      main.appendChild(hero);
      const c = card('Быстрые действия');
      c.appendChild(row('🎨', 'Персонализация', 'Обои, цвета, темы', el('div', 'muted', '›'), 'clickable'))
        .onclick = () => { cur = 'person'; drawNav(); drawMain(); };
      c.appendChild(row('🫧', 'Liquid Glass', 'Настроить материал интерфейса', el('div', 'muted', '›'), 'clickable'))
        .onclick = () => { cur = 'glass'; drawNav(); drawMain(); };
      c.appendChild(row('🌀', 'Анимации', 'Скорость и плавность', el('div', 'muted', '›'), 'clickable'))
        .onclick = () => { cur = 'motion'; drawNav(); drawMain(); };
      c.appendChild(row('🔄', 'Обновления', 'Система обновлена', el('div', 'muted', '›'), 'clickable'))
        .onclick = () => { cur = 'update'; drawNav(); drawMain(); };
      main.appendChild(c);

      const bk = card('Резервная копия системы');
      const exp = el('button', 'btn pri', '⬇️ Экспорт');
      exp.onclick = () => {
        const img = { v:1, date:Date.now(), settings:Store.s, fs:FS.root, kv:{} };
        Object.keys(localStorage).filter(k => k.startsWith('win12.')).forEach(k => img.kv[k] = localStorage[k]);
        const a = el('a'); a.setAttribute('download', 'windows12-backup.json');
        a.href = 'data:application/json;charset=utf-8,' + encodeURIComponent(JSON.stringify(img));
        document.body.appendChild(a); a.click(); a.remove();
        Shell.toast('Резервная копия', 'Образ системы сохранён в файл', '💾');
      };
      const imp = el('button', 'btn', '⬆️ Импорт');
      imp.onclick = () => {
        const f = el('input'); f.type = 'file'; f.accept = '.json,application/json';
        f.onchange = () => {
          const r = new FileReader();
          r.onload = () => {
            try {
              const img = JSON.parse(r.result);
              if (!img.settings || !img.fs) throw new Error('это не образ системы');
              if (!confirm('Заменить текущие настройки и файлы содержимым образа?')) return;
              Object.keys(img.kv || {}).forEach(k => localStorage[k] = img.kv[k]);
              localStorage.setItem(Store.key, JSON.stringify(img.settings));
              localStorage.setItem(FS.key, JSON.stringify(img.fs));
              location.reload();
            } catch(e){ alert('Не удалось прочитать образ: ' + e.message); }
          };
          r.readAsText(f.files[0]);
        };
        f.click();
      };
      bk.appendChild(row('💾', 'Экспорт всей системы', 'Настройки, файлы, заметки и задачи — одним файлом', exp));
      bk.appendChild(row('📂', 'Восстановить из файла', 'Полностью заменит текущее состояние', imp));
      main.appendChild(bk);

      const st = card('Хранилище');
      const used = JSON.stringify(localStorage).length / 1024;
      st.appendChild(row('💾', 'Локальные данные', used.toFixed(1) + ' КБ в localStorage',
        (() => { const b = el('button', 'btn', 'Очистить всё'); b.onclick = () => {
          if (confirm('Сбросить все настройки и файлы?')){ localStorage.clear(); location.reload(); } }; return b; })()));
      main.appendChild(st);
    }

    /* --- Система --- */
    function pSystem(){
      const c = card('Система');
      c.appendChild(row('💡', 'Дисплей', 'Яркость, ночной свет', el('div', 'muted', '›'), 'clickable')).onclick = () => { cur = 'display'; drawNav(); drawMain(); };
      c.appendChild(row('🔊', 'Звук', 'Громкость и устройства', el('div', 'muted', '›'), 'clickable')).onclick = () => { cur = 'sound'; drawNav(); drawMain(); };
      c.appendChild(row('🔔', 'Уведомления', 'Баннеры, фокусировка', el('div', 'muted', '›'), 'clickable')).onclick = () => { cur = 'notif'; drawNav(); drawMain(); };
      main.appendChild(c);

      const m = card('Многозадачность');
      m.appendChild(row('🪟', 'Подсказки прилипания', 'Показывать зоны при перетаскивании к краю', toggle(() => S.snapAssist, v => set('snapAssist', v))));
      m.appendChild(row('🖥️', 'Виртуальные рабочие столы', 'Ctrl + Alt + ← / →',
        seg([{ n:'2', v:2 }, { n:'3', v:3 }, { n:'4', v:4 }], () => WM.desks, v => { WM.desks = v; Shell.renderTaskview(); })));
      const tb = el('button', 'btn', 'Разложить окна'); tb.onclick = () => WM.tile();
      m.appendChild(row('▦', 'Мозаика окон', 'Разложить все окна плиткой', tb));
      const cb = el('button', 'btn', 'Каскад'); cb.onclick = () => WM.cascade();
      m.appendChild(row('🗂', 'Каскад', 'Расположить окна лесенкой', cb));
      main.appendChild(m);

      const p = card('Питание');
      p.appendChild(row('🔋', 'Режим питания', 'Баланс производительности',
        sel([{ n:'Экономия', v:'eco' }, { n:'Сбалансированный', v:'bal' }, { n:'Максимум', v:'max' }],
            () => KV.get('power', 'bal'), v => { KV.set('power', v); Shell.toast('Питание', 'Режим изменён', '🔋'); })));
      p.appendChild(row('😴', 'Спящий режим', 'Через 15 минут бездействия', toggle(() => KV.get('sleep', false), v => KV.set('sleep', v))));
      main.appendChild(p);
    }

    /* --- Персонализация --- */
    function pPerson(){
      const w = card('Фон рабочего стола');
      const g = el('div', 'walls');
      WALLPAPERS.forEach(x => {
        const d = el('div', 'wall' + (x.id === S.wallpaper ? ' on' : ''), `<span class="nm">${x.name}</span>`);
        d.style.backgroundImage = x.css;
        d.onclick = () => { set('wallpaper', x.id); $$('.wall', g).forEach(n => n.classList.remove('on')); d.classList.add('on'); };
        g.appendChild(d);
      });
      w.appendChild(g);
      w.appendChild(row('🔀', 'Слайд-шоу', 'Менять обои каждые 30 секунд', toggle(() => S.wallShuffle, v => { set('wallShuffle', v); Shell.wallShuffle(); })));
      const wUp = el('button', 'btn pri', '🖼 Выбрать файл…');
      wUp.onclick = () => {
        const f = el('input'); f.type = 'file'; f.accept = 'image/*';
        f.onchange = async () => {
          if (!f.files[0]) return;
          await IDB.put('wallpaper', f.files[0]);
          window.__customWall = URL.createObjectURL(f.files[0]);
          set('wallpaper', 'custom'); drawMain();
          Shell.toast('Обои', 'Своё изображение установлено', '🖼');
        };
        f.click();
      };
      w.appendChild(row('📤', 'Своё изображение', S.wallpaper === 'custom' ? 'Сейчас используется ваш файл' : 'Загрузить картинку с компьютера', wUp));
      main.appendChild(w);

      const t = card('Цвета и тема');
      t.appendChild(row('🌗', 'Режим', 'Светлая или тёмная тема',
        seg([{ n:'Тёмная', v:'dark' }, { n:'Светлая', v:'light' }], () => S.theme, v => set('theme', v))));
      const swWrap = el('div', 'swatches');
      ACCENTS.forEach((a, i) => {
        const s = el('div', 'sw' + (!S.accentCustom && i === S.accent ? ' on' : ''));
        s.style.background = `linear-gradient(140deg,${a.a},${a.b})`; s.title = a.n;
        s.onclick = () => { S.accentCustom = null; set('accent', i); $$('.sw', swWrap).forEach(x => x.classList.remove('on')); s.classList.add('on'); };
        swWrap.appendChild(s);
      });
      t.appendChild(row('🎯', 'Цвет акцента', 'Используется в кнопках и подсветке', swWrap));
      const cp = el('input'); cp.type = 'color'; cp.className = 'inp'; cp.style.cssText = 'width:48px;padding:2px';
      cp.value = S.accentCustom || accentPair().a;
      cp.oninput = () => { S.accentCustom = cp.value; Store.save(); applySettings(); };
      t.appendChild(row('🖌️', 'Свой цвет', 'Задать акцент вручную', cp));
      main.appendChild(t);

      const f = card('Шрифт и текст');
      f.appendChild(row('🔤', 'Системный шрифт', 'Гарнитура интерфейса',
        sel([{ n:'Segoe UI (Windows)', v:"'Segoe UI Variable','Segoe UI',system-ui,sans-serif" },
             { n:'SF Pro (macOS)', v:"-apple-system,'SF Pro Display','Helvetica Neue',sans-serif" },
             { n:'Inter / системный', v:"Inter,system-ui,sans-serif" },
             { n:'Georgia (с засечками)', v:"Georgia,'Times New Roman',serif" },
             { n:'Моноширинный', v:"'Cascadia Code',Consolas,monospace" }], () => S.font, v => set('font', v))));
      main.appendChild(f);

      const st = card('Меню Пуск');
      const pinBox = el('div'); pinBox.style.cssText = 'display:flex;flex-wrap:wrap;gap:6px;padding:10px 16px 14px';
      Object.entries(APPS).forEach(([id, a]) => {
        const on = S.pinned.includes(id);
        const b = el('button', 'btn' + (on ? ' pri' : ''), a.glyph + ' ' + a.name);
        b.onclick = () => {
          const i = S.pinned.indexOf(id);
          if (i >= 0) S.pinned.splice(i, 1); else S.pinned.push(id);
          Store.save(); Shell.renderStart(); b.classList.toggle('pri');
        };
        pinBox.appendChild(b);
      });
      st.appendChild(row('📌', 'Закреплённые приложения', 'Нажмите, чтобы закрепить или открепить', el('span')));
      st.appendChild(pinBox);
      main.appendChild(st);

      const wd = card('Виджеты рабочего стола');
      wd.appendChild(row('🧩', 'Показывать виджеты', 'Перетаскиваются мышью, удаляются крестиком',
        toggle(() => S.showDeskWidgets, v => { set('showDeskWidgets', v); Shell.renderDeskWidgets(); })));
      const wdBox = el('div'); wdBox.style.cssText = 'display:flex;flex-wrap:wrap;gap:6px;padding:4px 16px 14px';
      Object.entries(WIDGETS).forEach(([k, d]) => {
        const b = el('button', 'btn', d.e + ' ' + d.n);
        b.onclick = () => { addWidget(k); drawMain(); };
        wdBox.appendChild(b);
      });
      wd.appendChild(wdBox);
      (S.deskWidgets || []).forEach((w2, idx) => {
        const d = WIDGETS[w2.t]; if (!d) return;
        const rm = el('button', 'btn', '✖ Убрать');
        rm.onclick = () => { S.deskWidgets.splice(idx, 1); Store.save(); Shell.renderDeskWidgets(); drawMain(); };
        wd.appendChild(row(d.e, d.n, 'На рабочем столе', rm));
      });
      main.appendChild(wd);

      const lk = card('Экран блокировки');
      lk.appendChild(row('🖼️', 'Фон блокировки', 'Совпадает с рабочим столом', el('div', 'muted tiny', 'Обои')));
      lk.appendChild(row('🌤', 'Погода на экране блокировки', '', toggle(() => KV.get('lockWeather', true), v => KV.set('lockWeather', v))));
      main.appendChild(lk);
    }

    /* --- Liquid Glass --- */
    function pGlass(){
      const prev = el('div', 'card'); prev.style.cssText = 'height:150px;padding:0;position:relative;overflow:hidden';
      prev.style.backgroundImage = (WALLPAPERS.find(w => w.id === S.wallpaper) || WALLPAPERS[0]).css;
      prev.style.backgroundSize = 'cover';
      const pane = el('div', 'glass lg');
      pane.style.cssText = 'position:absolute;inset:26px 60px;display:grid;place-content:center;font-size:14px';
      pane.textContent = 'Предпросмотр материала';
      prev.appendChild(pane);
      main.appendChild(prev);

      const c = card('Материал');
      c.appendChild(row('🫧', 'Прозрачность', 'Выключите для непрозрачного интерфейса', toggle(() => S.transparency, v => set('transparency', v))));
      c.appendChild(row('💨', 'Размытие фона', 'blur() под панелями', slider(() => S.blur, v => set('blur', v), 0, 80, 1, v => v + 'px')));
      c.appendChild(row('🧊', 'Плотность стекла', 'Насколько материал светлее фона', slider(() => S.glass * 100, v => set('glass', v / 100), 0, 100, 1, v => v + '%')));
      c.appendChild(row('✨', 'Яркость кромки', 'Блик по краю панели', slider(() => S.glassEdge * 100, v => set('glassEdge', v / 100), 0, 100, 1, v => v + '%')));
      c.appendChild(row('🌈', 'Насыщенность', 'saturate() под стеклом', slider(() => S.saturate, v => set('saturate', v), 100, 300, 5, v => v + '%')));
      main.appendChild(c);

      const r = card('Форма');
      r.appendChild(row('⬜', 'Скругление панелей', '', slider(() => S.radius, v => set('radius', v), 0, 34, 1, v => v + 'px')));
      r.appendChild(row('🪟', 'Скругление окон', '', slider(() => S.winRadius, v => set('winRadius', v), 0, 30, 1, v => v + 'px')));
      main.appendChild(r);

      const p = card('Пресеты');
      const pr = el('div'); pr.style.cssText = 'display:flex;gap:8px;flex-wrap:wrap;padding:10px 16px 14px';
      [['Максимум стекла', { blur:60, glass:.5, glassEdge:.45, saturate:220, transparency:true, radius:22, winRadius:20 }],
       ['Windows 12', { blur:34, glass:.42, glassEdge:.28, saturate:180, transparency:true, radius:18, winRadius:16 }],
       ['macOS', { blur:44, glass:.34, glassEdge:.5, saturate:200, transparency:true, radius:14, winRadius:12 }],
       ['Матовое', { blur:22, glass:.7, glassEdge:.16, saturate:120, transparency:true, radius:16, winRadius:14 }],
       ['Без стекла', { blur:0, glass:.9, glassEdge:.1, saturate:100, transparency:false, radius:12, winRadius:10 }]
      ].forEach(([n, v]) => {
        const b = el('button', 'btn', n);
        b.onclick = () => { Object.assign(S, v); Store.save(); applySettings(); drawMain(); Shell.toast('Пресет применён', n, '🫧'); };
        pr.appendChild(b);
      });
      p.appendChild(pr);
      main.appendChild(p);
    }

    /* --- Док --- */
    function pDock(){
      const c = card('Док / панель задач');
      c.appendChild(row('📏', 'Размер значков', '', slider(() => S.dockSize, v => { set('dockSize', v); Shell.renderDock(); }, 34, 78, 1, v => v + 'px')));
      c.appendChild(row('🔍', 'Увеличение (macOS)', 'Значки растут под курсором', toggle(() => S.dockMagOn, v => set('dockMagOn', v))));
      c.appendChild(row('📐', 'Сила увеличения', '', slider(() => S.dockMag * 100, v => set('dockMag', v / 100), 100, 240, 5, v => (v / 100).toFixed(1) + '×')));
      c.appendChild(row('👻', 'Автоскрытие', 'Док появляется при наведении к низу экрана', toggle(() => S.dockAutohide, v => set('dockAutohide', v))));
      main.appendChild(c);

      const a = card('Приложения в доке');
      const box = el('div'); box.style.cssText = 'display:flex;flex-wrap:wrap;gap:6px;padding:10px 16px 14px';
      Object.entries(APPS).forEach(([id, ap]) => {
        const b = el('button', 'btn' + (S.dockApps.includes(id) ? ' pri' : ''), ap.glyph + ' ' + ap.name);
        b.onclick = () => {
          const i = S.dockApps.indexOf(id);
          if (i >= 0) S.dockApps.splice(i, 1); else S.dockApps.push(id);
          Store.save(); Shell.renderDock(); b.classList.toggle('pri');
        };
        box.appendChild(b);
      });
      a.appendChild(box);
      main.appendChild(a);

      const d = card('Рабочий стол');
      d.appendChild(row('🗂', 'Значки на рабочем столе', '', toggle(() => S.showDesktopIcons, v => set('showDesktopIcons', v))));
      d.appendChild(row('📊', 'Виджеты на рабочем столе', 'Часы, погода, заметка', toggle(() => S.showDeskWidgets, v => { set('showDeskWidgets', v); Shell.renderDeskWidgets(); })));
      d.appendChild(row('🎛', 'Кнопки окна', 'Стиль Windows или macOS',
        seg([{ n:'Windows', v:'win' }, { n:'macOS', v:'mac' }], () => S.wctl, v => set('wctl', v))));
      d.appendChild(row('📍', 'Активные углы', 'Правый верхний угол — просмотр задач', toggle(() => S.hotcorners, v => set('hotcorners', v))));
      d.appendChild(row('⬆️', 'Скрывать верхнюю панель', 'Панель и виджеты уходят, когда открыто окно',
        toggle(() => S.topbarAutohide !== false, v => { set('topbarAutohide', v); Shell.updateChrome(); })));
      d.appendChild(row('🪟', 'Панель задач как в Windows', 'При развёрнутом окне док растягивается на всю ширину',
        toggle(() => S.taskbarFull !== false, v => { set('taskbarFull', v); Shell.updateTaskbar(); })));
      d.appendChild(row('🕒', 'Часы и трей в панели', 'Когда верхняя панель скрыта, часы переезжают вниз',
        toggle(() => S.trayInDock !== false, v => { set('trayInDock', v); Shell.updateTaskbar(); })));
      main.appendChild(d);
    }

    /* --- Анимации --- */
    function pMotion(){
      const c = card('Плавность');
      c.appendChild(row('⚡', 'Скорость анимаций', '1× — стандарт, меньше — быстрее',
        slider(() => S.speed * 100, v => set('speed', v / 100), 30, 250, 5, v => (v / 100).toFixed(2) + '×')));
      c.appendChild(row('♿', 'Уменьшить движение', 'Почти мгновенные переходы', toggle(() => S.reduceMotion, v => set('reduceMotion', v))));
      main.appendChild(c);
      const d = card('Демонстрация');
      const b = el('button', 'btn pri', 'Показать');
      b.onclick = () => { ['calc','notepad','clock','photos'].forEach((a, i) => setTimeout(() => WM.open(a), i * 160)); setTimeout(() => WM.tile(), 900); };
      d.appendChild(row('🎬', 'Каскад открытия окон', 'Откроет 4 окна и разложит их плиткой', b));
      main.appendChild(d);
    }

    /* --- Звук --- */
    function pSound(){
      const c = card('Вывод');
      c.appendChild(row('🔊', 'Громкость', '', slider(() => S.volume, v => set('volume', v), 0, 100, 1, v => v + '%')));
      c.appendChild(row('🎚', 'Звуки интерфейса', 'Открытие и закрытие окон (по умолчанию выключены)',
        toggle(() => S.sounds, v => set('sounds', v))));
      c.appendChild(row('🔔', 'Звук уведомлений', 'Короткий сигнал у баннеров',
        toggle(() => S.soundNotif !== false, v => set('soundNotif', v))));
      c.appendChild(row('🎧', 'Устройство вывода', '',
        sel([{ n:'Динамики (Realtek)', v:'sp' }, { n:'Наушники', v:'hp' }, { n:'HDMI', v:'hdmi' }], () => KV.get('out', 'sp'), v => KV.set('out', v))));
      const t = el('button', 'btn', '▶ Тест');
      t.onclick = () => [0, 1, 2, 3].forEach(i => setTimeout(() => Snd.blip(440 * Math.pow(1.26, i), .18, 'triangle', .07), i * 150));
      c.appendChild(row('🎵', 'Проверить звук', 'Воспроизвести тестовый сигнал', t));
      main.appendChild(c);
    }

    /* --- Дисплей --- */
    function pDisplay(){
      const c = card('Яркость и цвет');
      c.appendChild(row('☀️', 'Яркость', '', slider(() => S.brightness, v => set('brightness', v), 30, 100, 1, v => v + '%')));
      c.appendChild(row('🌙', 'Ночной свет', 'Тёплые тона для вечера', toggle(() => S.nightLight, v => set('nightLight', v))));
      c.appendChild(row('🌗', 'Тема по времени суток', 'Светлая днём, тёмная ночью', toggle(() => S.autoTheme, v => { set('autoTheme', v); Shell.autoTheme(); })));
      main.appendChild(c);
      const m = card('Масштаб и разрешение');
      m.appendChild(row('🔎', 'Масштаб', 'Размер текста и элементов',
        seg([{ n:'90%', v:.9 }, { n:'100%', v:1 }, { n:'110%', v:1.1 }, { n:'125%', v:1.25 }],
            () => KV.get('zoom', 1), v => { KV.set('zoom', v); document.documentElement.style.fontSize = (16 * v) + 'px'; })));
      m.appendChild(row('🖥️', 'Разрешение', 'Окно браузера', el('div', 'muted tiny', innerWidth + ' × ' + innerHeight)));
      main.appendChild(m);
    }

    /* --- Сеть --- */
    function pNet(){
      const c = card('Wi-Fi');
      c.appendChild(row('📶', 'Wi-Fi', S.wifi ? 'Подключено к Dymensity-5G' : 'Отключено', toggle(() => S.wifi, v => { set('wifi', v); drawMain(); })));
      if (S.wifi){
        [['Dymensity-5G', '▮▮▮▮', true], ['Home_Net', '▮▮▮', false], ['Guest', '▮▮', false], ['MosMetro_Free', '▮', false]]
          .forEach(([n, s, on]) => c.appendChild(row(on ? '✅' : '📡', n, on ? 'Подключено, защищено' : 'Защищено (WPA2)', el('div', 'muted', s))));
      }
      main.appendChild(c);
      const v = card('VPN и прокси');
      v.appendChild(row('🛡', 'VPN', 'Не подключено', toggle(() => KV.get('vpn', false), x => KV.set('vpn', x))));
      v.appendChild(row('🌐', 'Прокси-сервер', 'Определять автоматически', toggle(() => KV.get('proxy', true), x => KV.set('proxy', x))));
      v.appendChild(row('✈️', 'Режим полёта', '', toggle(() => KV.get('air', false), x => { KV.set('air', x); if (x){ set('wifi', false); set('bluetooth', false); } drawMain(); })));
      main.appendChild(v);
    }

    /* --- Bluetooth --- */
    function pBt(){
      const c = card('Устройства');
      c.appendChild(row('🎧', 'Bluetooth', S.bluetooth ? 'Включён' : 'Выключен', toggle(() => S.bluetooth, v => { set('bluetooth', v); drawMain(); })));
      if (S.bluetooth) [['🎧','Dymensity Buds','Подключены · 82%'],['⌨️','Клавиатура K380','Сопряжено'],['🖱','Magic Mouse','Сопряжено'],['📱','Телефон','Не подключён']]
        .forEach(([e, n, s]) => c.appendChild(row(e, n, s, el('button', 'btn', '⋯'))));
      main.appendChild(c);
      const a = card('Общий доступ');
      a.appendChild(row('📤', 'AirDrop / Обмен с окружением', 'Видимость для устройств рядом', toggle(() => S.airdrop, v => set('airdrop', v))));
      a.appendChild(row('🖨', 'Принтеры и сканеры', 'Устройств не найдено', el('button', 'btn', 'Добавить')));
      main.appendChild(a);
    }

    /* --- Уведомления --- */
    function pNotif(){
      const c = card('Уведомления');
      c.appendChild(row('🔔', 'Уведомления', 'Показывать баннеры приложений', toggle(() => KV.get('notif', true), v => KV.set('notif', v))));
      c.appendChild(row('🌙', 'Не беспокоить', 'Скрывать баннеры и звуки', toggle(() => S.dnd, v => { set('dnd', v); Shell.updateCC(); })));
      c.appendChild(row('🎯', 'Фокусировка', 'Сессия концентрации',
        sel([{ n:'Выключено', v:'off' }, { n:'25 минут', v:'25' }, { n:'50 минут', v:'50' }],
            () => KV.get('focus', 'off'), v => { KV.set('focus', v); if (v !== 'off') Shell.toast('Фокусировка', v + ' минут концентрации', '🎯'); })));
      const t = el('button', 'btn', 'Показать'); t.onclick = () => Shell.toast('Тестовое уведомление', 'Так выглядят баннеры Windows 12', '🔔');
      c.appendChild(row('🧪', 'Проверить', 'Показать пример уведомления', t));
      main.appendChild(c);
    }

    /* --- Приложения --- */
    function pApps(){
      const c = card('Установленные приложения');
      Object.entries(APPS).forEach(([id, a]) => {
        const b = el('button', 'btn', 'Открыть'); b.onclick = () => WM.open(id);
        c.appendChild(row(a.glyph, a.name, 'Системное приложение · ' + (a.w || 800) + '×' + (a.h || 600), b));
      });
      main.appendChild(c);
      const au = card('Автозапуск');
      const auBox = el('div'); auBox.style.cssText = 'display:flex;flex-wrap:wrap;gap:6px;padding:4px 16px 14px';
      Object.entries(APPS).forEach(([id, a]) => {
        const on = (S.autostart || []).includes(id);
        const b = el('button', 'btn' + (on ? ' pri' : ''), a.glyph + ' ' + a.name);
        b.onclick = () => {
          S.autostart = S.autostart || [];
          const k = S.autostart.indexOf(id);
          if (k >= 0) S.autostart.splice(k, 1); else S.autostart.push(id);
          Store.save(); b.classList.toggle('pri');
        };
        auBox.appendChild(b);
      });
      au.appendChild(row('🚀', 'Запускать при входе', 'Отмеченные приложения откроются сразу после загрузки', el('span')));
      au.appendChild(auBox);
      main.appendChild(au);

      const d = card('Приложения по умолчанию');
      d.appendChild(row('🌐', 'Браузер', 'Dymensity Browser', el('div', 'muted', '›')));
      d.appendChild(row('📝', 'Текстовый редактор', 'Блокнот', el('div', 'muted', '›')));
      d.appendChild(row('🖼️', 'Просмотр фото', 'Фотографии', el('div', 'muted', '›')));
      main.appendChild(d);
    }

    /* --- Учётные записи --- */
    function pAcc(){
      const hero = el('div', 'set-hero');
      hero.innerHTML = `<div class="ava">${esc(S.userName[0])}</div><div><b style="font-size:17px">${esc(S.userName)}</b>
        <div class="muted tiny">Локальная учётная запись · Администратор</div></div>`;
      main.appendChild(hero);
      const c = card('Профиль');
      const i = el('input', 'inp'); i.value = S.userName;
      i.onchange = () => { set('userName', i.value || 'User'); Shell.renderShell(); };
      c.appendChild(row('👤', 'Имя пользователя', 'Отображается в Пуске и на экране блокировки', i));
      c.appendChild(row('🔑', 'Пароль', 'Не задан', el('button', 'btn', 'Изменить')));
      c.appendChild(row('👁', 'Windows Hello', 'Вход по лицу недоступен в браузере', el('div', 'muted tiny', 'Недоступно')));
      c.appendChild(row('☁️', 'Синхронизация', 'Настройки хранятся в localStorage', el('div', 'muted tiny', 'Локально')));
      main.appendChild(c);
    }

    /* --- Время --- */
    function pTime(){
      const c = card('Дата и время');
      c.appendChild(row('🕐', '24-часовой формат', '', toggle(() => S.clock24, v => set('clock24', v))));
      c.appendChild(row('⏱', 'Показывать секунды', 'В часах на панели', toggle(() => S.showSeconds, v => set('showSeconds', v))));
      c.appendChild(row('🌍', 'Часовой пояс', Intl.DateTimeFormat().resolvedOptions().timeZone, el('div', 'muted tiny', new Date().toString().match(/GMT[+-]\d+/) || '')));
      main.appendChild(c);
      const l = card('Язык и регион');
      l.appendChild(row('🗣', 'Язык интерфейса', 'Русский (Россия)', el('div', 'muted', '›')));
      const ci = el('input', 'inp'); ci.value = S.city;
      ci.onchange = () => { set('city', ci.value); Shell.renderShell(); };
      l.appendChild(row('🏙', 'Город для погоды', 'Влияет на виджет погоды', ci));
      main.appendChild(l);
    }

    /* --- Доступность --- */
    function pA11y(){
      const c = card('Зрение');
      c.appendChild(row('🔍', 'Масштаб текста', '',
        seg([{ n:'S', v:.9 }, { n:'M', v:1 }, { n:'L', v:1.15 }, { n:'XL', v:1.3 }],
            () => KV.get('zoom', 1), v => { KV.set('zoom', v); document.documentElement.style.fontSize = (16 * v) + 'px'; })));
      c.appendChild(row('🌗', 'Высокая контрастность', 'Отключает прозрачность', toggle(() => !S.transparency, v => set('transparency', !v))));
      c.appendChild(row('🌀', 'Уменьшить движение', '', toggle(() => S.reduceMotion, v => set('reduceMotion', v))));
      c.appendChild(row('🖱', 'Крупный курсор', '', toggle(() => KV.get('bigCursor', false), v => { KV.set('bigCursor', v);
        document.body.style.cursor = v ? 'url("data:image/svg+xml,%3Csvg xmlns=\'http://www.w3.org/2000/svg\' width=\'36\' height=\'36\'%3E%3Cpath d=\'M6 2l22 12-10 2-4 10z\' fill=\'white\' stroke=\'black\'/%3E%3C/svg%3E"), auto' : ''; })));
      main.appendChild(c);
      const s = card('Слух и ввод');
      s.appendChild(row('🔉', 'Моно-звук', '', toggle(() => KV.get('mono', false), v => KV.set('mono', v))));
      s.appendChild(row('⌨️', 'Залипание клавиш', '', toggle(() => KV.get('sticky', false), v => KV.set('sticky', v))));
      main.appendChild(s);
    }

    /* --- Приватность --- */
    function pPrivacy(){
      const c = card('Разрешения');
      [['📍','Геолокация','Для виджета погоды'],['📷','Камера','Нет приложений'],['🎤','Микрофон','Нет приложений'],
       ['📋','Буфер обмена','Копирование имён файлов'],['📊','Диагностика','Не отправляется']]
        .forEach(([e, n, d]) => c.appendChild(row(e, n, d, toggle(() => KV.get('perm.' + n, true), v => KV.set('perm.' + n, v)))));
      main.appendChild(c);
      const s = card('Безопасность');
      s.appendChild(row('🛡', 'Защитник Windows', 'Угроз не найдено', el('div', 'muted tiny', '✅ Активен')));
      s.appendChild(row('🔥', 'Брандмауэр', 'Все сети защищены', el('div', 'muted tiny', '✅ Включён')));
      s.appendChild(row('🔐', 'Шифрование устройства', 'BitLocker недоступен в браузере', el('div', 'muted tiny', '—')));
      main.appendChild(s);
    }

    /* --- Обновления --- */
    function pUpdate(){
      const c = card('Центр обновления Windows');
      const st = el('div'); st.innerHTML = `<div style="padding:18px 16px;display:flex;gap:14px;align-items:center">
        <div style="font-size:34px">✅</div><div><b>Установлены все обновления</b>
        <div class="muted tiny">Последняя проверка: сегодня, ${pad2(new Date().getHours())}:${pad2(new Date().getMinutes())}</div></div></div>`;
      c.appendChild(st);
      const b = el('button', 'btn pri', 'Проверить наличие обновлений');
      b.onclick = () => {
        b.textContent = 'Проверка…'; b.disabled = true;
        setTimeout(() => { b.textContent = 'Проверить наличие обновлений'; b.disabled = false;
          Shell.toast('Центр обновления', 'Обновлений нет — вы на последней версии', '🔄'); }, 1600);
      };
      c.appendChild(row('🔄', 'Проверка', '', b));
      c.appendChild(row('⏰', 'Часы активности', '08:00 — 23:00', el('div', 'muted', '›')));
      c.appendChild(row('🧪', 'Программа предварительной оценки', 'Канал Dev', el('div', 'muted tiny', 'Участник')));
      main.appendChild(c);
      const h = card('Журнал обновлений');
      [['12.0.1200','Прототип оболочки Liquid Glass'],['12.0.1180','Улучшения дока и анимаций'],['12.0.1150','Первая сборка']]
        .forEach(([v, d]) => h.appendChild(row('📦', 'Windows 12 · ' + v, d, el('div', 'muted tiny', 'Установлено'))));
      main.appendChild(h);
    }

    /* --- О системе --- */
    function pAbout(){
      const box = el('div', 'card');
      box.innerHTML = `<div class="about-logo">
        <div class="win-logo"><i></i><i></i><i></i><i></i></div>
        <h2 style="margin:0">Windows 12 Pro</h2>
        <div class="muted">Версия 12.0 (сборка 1200) · Прототип</div></div>`;
      main.appendChild(box);
      const c = card('Характеристики устройства');
      [['💻','Имя устройства','DYMENSITY-PC'],
       ['⚙️','Процессор', (navigator.hardwareConcurrency || 8) + ' логических ядер'],
       ['🧠','Память', (navigator.deviceMemory ? navigator.deviceMemory + ' ГБ' : 'н/д')],
       ['🖥️','Экран', screen.width + ' × ' + screen.height + ' @ ' + (devicePixelRatio || 1) + 'x'],
       ['🌐','Движок', navigator.userAgent.includes('Firefox') ? 'Gecko' : navigator.userAgent.includes('Safari') && !navigator.userAgent.includes('Chrome') ? 'WebKit' : 'Blink'],
       ['🗣','Язык', navigator.language]
      ].forEach(([e, n, v]) => c.appendChild(row(e, n, '', el('div', 'muted tiny', v))));
      main.appendChild(c);
      const r = card('Сброс');
      const b = el('button', 'btn', 'Сбросить настройки');
      b.onclick = () => { if (confirm('Вернуть настройки по умолчанию?')){ Store.reset(); Shell.renderShell(); drawMain(); Shell.toast('Готово', 'Настройки сброшены', '🔄'); } };
      r.appendChild(row('♻️', 'Настройки по умолчанию', 'Файлы останутся на месте', b));
      main.appendChild(r);
    }

    drawNav(); drawMain();
  }
};
/* повторное открытие Параметров с нужным разделом */
APPS.settings.onReopen = function(win, opts){
  if (!opts || !opts.section) return;
  win.body.replaceChildren();
  APPS.settings.render(win, opts);
};
