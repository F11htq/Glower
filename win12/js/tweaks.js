/* ==========================================================================
   Панель задач в стиле Windows, управляемые виджеты стола,
   расширенный терминал и Музыка со своими аудиофайлами
   ========================================================================== */
'use strict';

/* ---------- IndexedDB для больших данных (аудио, свои обои) ---------- */
const IDB = {
  db:null,
  open(){
    if (this.db) return Promise.resolve(this.db);
    return new Promise((res, rej) => {
      const r = indexedDB.open('win12', 1);
      r.onupgradeneeded = () => r.result.createObjectStore('blobs');
      r.onsuccess = () => { this.db = r.result; res(this.db); };
      r.onerror = () => rej(r.error);
    });
  },
  async tx(mode){ const db = await this.open(); return db.transaction('blobs', mode).objectStore('blobs'); },
  async put(k, v){ const s = await this.tx('readwrite'); return new Promise(r => { s.put(v, k).onsuccess = () => r(true); }); },
  async get(k){ const s = await this.tx('readonly'); return new Promise(r => { const q = s.get(k); q.onsuccess = () => r(q.result); q.onerror = () => r(null); }); },
  async del(k){ const s = await this.tx('readwrite'); return new Promise(r => { s.delete(k).onsuccess = () => r(true); }); }
};
window.IDB = IDB;

(function tweaks(){

/* ==========================================================================
   1. Нижняя панель: док ↔ панель задач Windows
   ========================================================================== */
const dock = $('#dock');
const tray = $('#tb-tray');
const topbar = $('.topbar');
const hot = $('.top-hotzone');
if (hot) hot.remove();                       // верхняя панель больше не выезжает по наведению

const tbSearch = el('button', 'tb-dock-search', `<svg viewBox="0 0 24 24" class="ic"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg><span>Поиск</span>`);
tbSearch.onclick = () => Shell.spot(true);
tbSearch.dataset.tip = 'Поиск (Win+Space)';

Shell.updateTaskbar = function(){
  const mine = WM.wins.filter(w => w.desk === WM.desk && !w.minimized);
  const anyMax = mine.some(w => w.maximized);
  const busy = mine.length > 0;

  document.body.classList.toggle('taskbar', anyMax && S.taskbarFull !== false);

  // трей переезжает в панель, когда верхняя панель скрыта
  const inDock = busy && S.trayInDock !== false;
  if (inDock && tray.parentElement !== dock){
    dock.appendChild(tray); tray.classList.add('in-dock');
    dock.insertBefore(tbSearch, $('#dock-items', dock));
  } else if (!inDock && tray.parentElement !== topbar){
    topbar.appendChild(tray); tray.classList.remove('in-dock');
    tbSearch.remove();
  }

  Shell.fitDock();

  // развёрнутые окна не должны залезать под панель задач
  WM.wins.filter(w => w.maximized).forEach(w => {
    const m = WM.maxRect();
    Object.assign(w.node.style, { left:m.left + 'px', top:m.top + 'px', width:m.width + 'px', height:m.height + 'px' });
    if (w.app.onResize) w.app.onResize(w);
  });
};

/* ==========================================================================
   Панель не должна наезжать сама на себя

   В режиме панели задач значки стоят по центру, а трей прижат к правому
   краю поверх потока — и когда открытых окон много, центральная группа
   заезжает под него. Резервируем ширину трея с обеих сторон (чтобы центр
   остался центром) и, если этого мало, уменьшаем значки, как делает
   настоящая панель задач.
   ========================================================================== */
Shell.fitDock = function(){
  const inDock = tray.parentElement === dock;
  const taskbar = document.body.classList.contains('taskbar');
  const pad = taskbar && inDock ? tray.offsetWidth + 22 : 0;
  dock.style.paddingLeft = dock.style.paddingRight = pad ? pad + 'px' : '';

  const root = document.documentElement;
  let size = S.dockSize;
  root.style.setProperty('--dock-size', size + 'px');
  for (let i = 0; i < 14 && dock.scrollWidth > dock.clientWidth + 1 && size > 30; i++){
    size -= 2;
    root.style.setProperty('--dock-size', size + 'px');
  }
};
addEventListener('resize', () => Shell.fitDock());

['syncDock','updateChrome'].forEach(fn => {
  const orig = Shell[fn].bind(Shell);
  Shell[fn] = (...a) => { const r = orig(...a); Shell.updateTaskbar(); return r; };
});
const maxOrig = WM.toggleMax.bind(WM);
WM.toggleMax = w => { maxOrig(w); setTimeout(() => Shell.updateTaskbar(), 30); };

/* ==========================================================================
   2. Виджеты рабочего стола: свои, перемещаемые, удаляемые
   ========================================================================== */
const WIDGETS = {
  clock:{ n:'Часы', e:'🕐', h(){ const d = new Date();
    return `<div class="w-t">Часы</div><div class="w-big dw-clock">${pad2(d.getHours())}:${pad2(d.getMinutes())}</div>
      <div class="tiny muted">${d.toLocaleDateString('ru-RU', { weekday:'long', day:'numeric', month:'long' })}</div>`; } },
  weather:{ n:'Погода', e:'🌤', h(){ const w = Shell.weather();
    return `<div class="w-t">Погода · ${esc(S.city)}${w.real ? '' : ' · демо'}</div><div class="row"><div style="font-size:30px">${w.ico}</div>
      <div><div class="w-big">${w.t > 0 ? '+' : ''}${w.t}°</div><div class="tiny muted">${w.desc}</div></div></div>`; } },
  calendar:{ n:'Календарь', e:'📅', h(){ const d = new Date();
    const ev = Object.entries(KV.get('cal.events', {})).slice(0, 2);
    return `<div class="w-t">Календарь</div><div class="w-big">${d.getDate()}</div>
      <div class="tiny muted">${d.toLocaleDateString('ru-RU', { weekday:'long', month:'long' })}</div>
      ${ev.map(([, v]) => { const e2 = window.Reminders ? Reminders.norm(v) : { t:v, time:'' };
        return `<div class="w-row"><span>${esc(e2.t)}</span><span class="muted tiny">${e2.time || ''}</span></div>`; }).join('')}`; } },
  tasks:{ n:'Задачи', e:'✅', h(){ const t = KV.get('todo', []).filter(x => !x.d).slice(0, 4);
    return `<div class="w-t">Задачи</div>${t.length ? t.map(x => `<div class="w-row"><span>○ ${esc(x.t)}</span></div>`).join('')
      : '<div class="w-row muted">Всё сделано 🎉</div>'}`; } },
  system:{ n:'Система', e:'📊', h(){
    return `<div class="w-t">Система</div>
      <div class="w-row"><span>Окон</span><b>${WM.wins.length}</b></div>
      <div class="w-row"><span>Стол</span><b>${WM.desk + 1}/${WM.desks}</b></div>
      <div class="w-row"><span>Размытие</span><b>${S.blur}px</b></div>`; } },
  note:{ n:'Заметка', e:'📝', h(){
    return `<div class="w-t">Заметка</div><div class="dw-note" contenteditable spellcheck="false">${esc(KV.get('dwNote', 'Нажмите и пишите…'))}</div>`; } }
};
window.WIDGETS = WIDGETS;

Shell.renderDeskWidgets = function(){
  const box = $('#desk-widgets');
  if (!box) return;
  box.innerHTML = '';
  if (!S.showDeskWidgets) return;

  (S.deskWidgets || []).forEach((w, i) => {
    const def = WIDGETS[w.t]; if (!def) return;
    const n = el('div', 'w-card glass dw', def.h());
    n.style.left = (w.x != null ? w.x : innerWidth - 260) + 'px';
    n.style.top  = (w.y != null ? w.y : 96 + i * 140) + 'px';
    n.appendChild(el('button', 'dw-x', '×'));
    $('.dw-x', n).onclick = e => { e.stopPropagation(); removeWidget(i); };

    const note = $('.dw-note', n);
    if (note) note.onblur = () => KV.set('dwNote', note.textContent.trim());

    n.addEventListener('mousedown', e => {
      if (e.target.closest('.dw-x, .dw-note')) return;
      const r = n.getBoundingClientRect(), sx = e.clientX, sy = e.clientY;
      let moved = false;
      const mv = ev => {
        const dx = ev.clientX - sx, dy = ev.clientY - sy;
        if (!moved && Math.hypot(dx, dy) < 4) return;
        moved = true; n.classList.add('drag');
        n.style.left = clamp(r.left + dx, 4, innerWidth - r.width - 4) + 'px';
        n.style.top  = clamp(r.top + dy, 4, innerHeight - r.height - 20) + 'px';
      };
      const up = () => {
        document.removeEventListener('mousemove', mv); document.removeEventListener('mouseup', up);
        n.classList.remove('drag');
        if (moved){ S.deskWidgets[i].x = parseFloat(n.style.left); S.deskWidgets[i].y = parseFloat(n.style.top); Store.save(); }
      };
      document.addEventListener('mousemove', mv); document.addEventListener('mouseup', up);
    });

    n.oncontextmenu = e => {
      e.preventDefault(); e.stopPropagation();
      Shell.ctx(e.clientX, e.clientY, [
        { i:'🗑️', t:'Убрать виджет', f:() => removeWidget(i) },
        'hr',
        ...Object.entries(WIDGETS).map(([k, d]) => ({ i:d.e, t:'Добавить: ' + d.n, f:() => addWidget(k) }))
      ]);
    };
    box.appendChild(n);
  });
};
function addWidget(t){
  S.deskWidgets = S.deskWidgets || [];
  S.deskWidgets.push({ t, x:innerWidth - 260, y:96 + S.deskWidgets.length * 140 });
  if (!S.showDeskWidgets) S.showDeskWidgets = true;
  Store.save(); Shell.renderDeskWidgets();
  Shell.toast('Виджеты', WIDGETS[t].n + ' добавлен на рабочий стол', WIDGETS[t].e);
}
function removeWidget(i){ S.deskWidgets.splice(i, 1); Store.save(); Shell.renderDeskWidgets(); }
window.addWidget = addWidget;

/* добавление виджетов из контекстного меню рабочего стола */
$('#desktop').addEventListener('contextmenu', e => {
  if (e.target.closest('.win, .dock, .start, .di, #cc, #widgets, .w-card')) return;
  setTimeout(() => {
    const c = $('#ctx');
    if (!c.classList.contains('on')) return;
    const b = el('button', '', `<span class="em">🧩</span><span>Добавить виджет…</span>`);
    b.onclick = () => {
      c.classList.remove('on');
      /* открываем список следующим кадром: меню не должно перестраиваться
         посреди обработки того же щелчка */
      setTimeout(() => Shell.ctx(e.clientX, e.clientY,
        Object.entries(WIDGETS).map(([k, d]) => ({ i:d.e, t:d.n, f:() => addWidget(k) }))), 10);
    };
    c.insertBefore(b, c.children[3] || null);
  }, 0);
});

/* часы в виджете идут */
setInterval(() => {
  const c = $('.dw-clock');
  if (c){ const d = new Date(); c.textContent = `${pad2(d.getHours())}:${pad2(d.getMinutes())}`; }
}, 1000);

/* ==========================================================================
   3. Терминал: полноценный набор команд
   ========================================================================== */
APPS.term = {
  name:'Терминал', glyph:'⌨️', bg:'linear-gradient(140deg,#334155,#0f172a)', w:820, h:520,
  render(win){
    let cwd = [];
    const out = el('div', 'term'); win.body.appendChild(out);
    const inLine = el('div', 'term-in');
    const prompt = el('span', 'pr');
    const inp = el('input'); inp.spellcheck = false; inp.autocapitalize = 'off';
    inLine.append(prompt, inp); out.appendChild(inLine);
    const print = (t, c = '') => { const l = el('div', 'term-line ' + c); l.innerHTML = t; out.insertBefore(l, inLine); out.scrollTop = out.scrollHeight; };
    const upd = () => prompt.textContent = `${S.userName.toLowerCase()}@${Brand.short.toLowerCase()}:/${cwd.join('/')}$`;
    const P = (...p) => [...cwd, ...p.filter(Boolean)];
    const resolve = a => {
      if (!a) return cwd.slice();
      if (a === '/' || a === '~') return [];
      let p = a.startsWith('/') ? [] : cwd.slice();
      a.replace(/^\//, '').split('/').filter(Boolean).forEach(s => {
        if (s === '.') return;
        if (s === '..') p.pop(); else p.push(s);
      });
      return p;
    };
    const hist = KV.get('term.hist', []); let hi = hist.length;

    const C = {};
    const def = (name, help, fn) => { C[name] = fn; fn.help = help; };

    def('help', 'список команд', a => {
      if (a && C[a]) return print(`${a} — ${C[a].help}`);
      const groups = {
        'Файлы':['ls','cd','pwd','cat','write','append','mkdir','rm','mv','cp','touch','tree','find','stat','du','head','tail','wc'],
        'Система':['ps','kill','open','sysinfo','neofetch','uptime','whoami','hostname','date','free','env'],
        'Оболочка':['theme','accent','glass','wall','volume','widget','notify','clear','history','echo','expr','exit']
      };
      print('<b class="inf">Доступные команды</b> (help &lt;команда&gt; — подробнее)');
      Object.entries(groups).forEach(([g, l]) => print(`  <b>${g}:</b> ${l.join(', ')}`));
    });

    def('ls', 'содержимое папки: ls [путь] [-l]', (a, raw) => {
      const long = /\s-l\b/.test(raw);
      const p = resolve((a && a !== '-l') ? a : '');
      const n = FS.node(p);
      if (!n || n.type !== 'dir') return print('ls: нет такой папки', 'er');
      const items = Object.values(n.children || {});
      if (!items.length) return print('(пусто)', 'muted');
      if (long) items.forEach(i => print(`${i.type === 'dir' ? 'd' : '-'}rw-r--r--  ${String(size(i)).padStart(8)}  ${new Date(i.mtime || 0).toLocaleDateString('ru-RU')}  ${i.type === 'dir' ? '<b class="inf">' + esc(i.name) + '</b>' : esc(i.name)}`));
      else print(items.map(i => i.type === 'dir' ? `<b class="inf">${esc(i.name)}/</b>` : esc(i.name)).join('   '));
    });
    def('cd', 'сменить папку: cd &lt;путь&gt;', a => {
      const p = resolve(a); const n = FS.node(p);
      if (!n || n.type !== 'dir') return print('cd: нет такой папки: ' + esc(a || ''), 'er');
      cwd = p; upd();
    });
    def('pwd', 'текущий путь', () => print('/' + cwd.join('/')));
    def('cat', 'показать файл', a => { const f = FS.node(resolve(a));
      f && f.type === 'file' ? print(esc(f.body || '(пусто)')) : print('cat: файл не найден', 'er'); });
    def('head', 'первые строки: head &lt;файл&gt; [n]', (a, raw) => lines(a, raw, true));
    def('tail', 'последние строки: tail &lt;файл&gt; [n]', (a, raw) => lines(a, raw, false));
    def('wc', 'счётчики строк/слов/символов', a => { const f = FS.node(resolve(a));
      if (!f || f.type !== 'file') return print('wc: файл не найден', 'er');
      const b = f.body || '';
      print(`${b.split('\n').length} строк  ${b.trim() ? b.trim().split(/\s+/).length : 0} слов  ${b.length} символов  ${esc(a)}`); });
    def('write', 'записать текст в файл: write &lt;файл&gt; &lt;текст&gt;', (a, raw) => {
      const i = raw.indexOf(' '); if (i < 0) return print('write &lt;файл&gt; &lt;текст&gt;', 'er');
      FS.write(cwd, raw.slice(0, i), raw.slice(i + 1)); print('Записано.', 'inf'); });
    def('append', 'дописать строку в конец файла', (a, raw) => {
      const i = raw.indexOf(' '); if (i < 0) return print('append &lt;файл&gt; &lt;текст&gt;', 'er');
      const name = raw.slice(0, i), f = FS.node([...cwd, name]);
      FS.write(cwd, name, ((f && f.body) || '') + (f && f.body ? '\n' : '') + raw.slice(i + 1)); print('Дописано.', 'inf'); });
    def('touch', 'создать пустой файл', a => { if (!a) return print('touch &lt;имя&gt;', 'er');
      FS.write(cwd, a, FS.node([...cwd, a]) ? FS.node([...cwd, a]).body : ''); print('OK', 'inf'); });
    def('mkdir', 'создать папку', a => FS.mkdir(cwd, a) ? print('OK', 'inf') : print('mkdir: не удалось', 'er'));
    def('rm', 'удалить (в корзину; -f — безвозвратно)', (a, raw) => {
      const f = /\s-f\b/.test(raw);
      FS.rm(cwd, a, f) ? print(f ? 'Удалено безвозвратно.' : 'Перемещено в корзину.', 'inf') : print('rm: не найдено', 'er');
      Shell.renderIcons(); });
    def('cp', 'копировать: cp &lt;что&gt; &lt;куда&gt;', (a, raw) => move(raw, false));
    def('mv', 'переместить/переименовать: mv &lt;что&gt; &lt;куда&gt;', (a, raw) => move(raw, true));
    def('tree', 'дерево папок', a => {
      const walk = (n, pre) => Object.values(n.children || {}).forEach((c, i, arr) => {
        const lastOne = i === arr.length - 1;
        print(pre + (lastOne ? '└── ' : '├── ') + (c.type === 'dir' ? `<b class="inf">${esc(c.name)}</b>` : esc(c.name)));
        if (c.type === 'dir') walk(c, pre + (lastOne ? '    ' : '│   '));
      });
      const n = FS.node(resolve(a)); if (!n) return print('tree: нет пути', 'er');
      print('/' + resolve(a).join('/')); walk(n, '');
    });
    def('find', 'поиск файлов по имени', a => {
      if (!a) return print('find &lt;подстрока&gt;', 'er');
      let k = 0;
      (function walk(n, p){ Object.values(n.children || {}).forEach(c => {
        if (c.name.toLowerCase().includes(a.toLowerCase())){ print('/' + [...p, c.name].join('/')); k++; }
        if (c.type === 'dir') walk(c, [...p, c.name]); }); })(FS.node(cwd), cwd);
      print(k ? `найдено: ${k}` : 'ничего не найдено', k ? 'inf' : 'muted');
    });
    def('stat', 'сведения о файле', a => { const f = FS.node(resolve(a));
      if (!f) return print('stat: не найдено', 'er');
      print(`  Имя:      ${esc(f.name)}\n  Тип:      ${f.type === 'dir' ? 'папка' : 'файл'}\n  Размер:   ${size(f)} Б\n  Создан:   ${new Date(f.ctime || 0).toLocaleString('ru-RU')}\n  Изменён:  ${new Date(f.mtime || 0).toLocaleString('ru-RU')}`); });
    def('du', 'размер папки', a => { const n = FS.node(resolve(a)); if (!n) return print('du: нет пути', 'er');
      print(`${(size(n) / 1024).toFixed(1)} КБ  /${resolve(a).join('/')}`); });

    def('ps', 'запущенные приложения', () => {
      print('  PID  ПРИЛОЖЕНИЕ            ОКНО');
      WM.wins.forEach((w, i) => print(`  ${String(1000 + i).padEnd(5)}${w.app.name.padEnd(22)}${esc(w.titleEl.textContent)}`));
      if (!WM.wins.length) print('  (нет запущенных приложений)', 'muted');
    });
    def('kill', 'закрыть окно: kill &lt;PID&gt;', a => {
      const w = WM.wins[+a - 1000];
      w ? (WM.close(w), print('Закрыто: ' + w.app.name, 'inf')) : print('kill: нет процесса ' + esc(a || ''), 'er'); });
    def('open', 'запустить приложение', a => APPS[a] ? (WM.open(a), print('Запуск ' + a, 'inf'))
      : print('open: нет приложения. Доступны: ' + Object.keys(APPS).join(', '), 'er'));
    def('sysinfo', 'сведения о системе', () => print(
      `  ОС:        ${Brand.full()} ${Brand.versionLine()}\n  Ядро:      ${navigator.userAgent.includes('Firefox') ? 'Gecko' : 'Blink'}\n` +
      `  Экран:     ${screen.width}×${screen.height} @${devicePixelRatio}x\n  Ядер ЦП:   ${navigator.hardwareConcurrency || '?'}\n` +
      `  Память:    ${navigator.deviceMemory ? navigator.deviceMemory + ' ГБ' : 'н/д'}\n  Сеть:      ${navigator.onLine ? 'подключено' : 'нет сети'}\n` +
      `  Хранилище: ${(JSON.stringify(localStorage).length / 1024).toFixed(1)} КБ`));
    def('neofetch', 'логотип и сводка', () => print(`<span class="inf">   ▗▄▄▖▗▄▄▖ </span>  <b>${S.userName}@${Brand.short.toLowerCase()}</b>
<span class="inf">   ▝▀▀▘▝▀▀▘ </span>  ──────────────────
<span class="inf">   ▗▄▄▖▗▄▄▖ </span>  ОС:     ${Brand.full()}
<span class="inf">   ▝▀▀▘▝▀▀▘ </span>  Тема:   ${S.theme} · акцент ${accentPair().a}
                  Стекло: ${S.blur}px / ${Math.round(S.glass * 100)}%
                  Окон:   ${WM.wins.length}`));
    def('uptime', 'время работы сессии', () => {
      const s = Math.round(performance.now() / 1000);
      print(`работает ${Math.floor(s / 3600)} ч ${Math.floor(s / 60) % 60} мин ${s % 60} с`); });
    def('whoami', 'текущий пользователь', () => print(S.userName));
    def('hostname', 'имя устройства', () => print(Brand.hostname));
    def('date', 'дата и время', () => print(new Date().toLocaleString('ru-RU')));
    def('free', 'использование хранилища', () => {
      const used = JSON.stringify(localStorage).length;
      print(`  localStorage: ${(used / 1024).toFixed(1)} КБ из ~5120 КБ (${(used / 51200).toFixed(1)}%)`); });
    def('env', 'переменные окружения', () => print(
      `  USER=${S.userName}\n  HOME=/\n  THEME=${S.theme}\n  CITY=${S.city}\n  LANG=${navigator.language}`));

    def('theme', 'тема: theme dark|light', a => ['dark','light'].includes(a)
      ? (Store.set('theme', a), print('Тема: ' + a, 'inf')) : print('theme dark|light', 'er'));
    def('accent', 'акцент: accent 0-9 или #hex', a => {
      if (/^#[0-9a-f]{6}$/i.test(a)){ S.accentCustom = a; Store.save(); applySettings(); return print('Акцент: ' + a, 'inf'); }
      const i = +a; if (!ACCENTS[i]) return print('accent 0-' + (ACCENTS.length - 1) + ' | #rrggbb\n' + ACCENTS.map((x, k) => `  ${k} ${x.n}`).join('\n'), 'er');
      S.accentCustom = null; Store.set('accent', i); print('Акцент: ' + ACCENTS[i].n, 'inf'); });
    def('glass', 'плотность стекла: glass 0-100', a => { const v = clamp(+a, 0, 100);
      Store.set('glass', v / 100); print('Стекло: ' + v + '%', 'inf'); });
    def('wall', 'обои: wall &lt;id|list&gt;', a => {
      if (!a || a === 'list') return print(WALLPAPERS.map(w => `  ${w.id.padEnd(9)}${w.name}`).join('\n'));
      WALLPAPERS.find(w => w.id === a) ? (Store.set('wallpaper', a), print('Обои: ' + a, 'inf')) : print('нет таких обоев', 'er'); });
    def('volume', 'громкость: volume 0-100', a => { const v = clamp(+a, 0, 100); Store.set('volume', v); print('Громкость: ' + v, 'inf'); });
    def('widget', 'виджет на стол: widget &lt;тип|list&gt;', a => {
      if (!a || a === 'list') return print(Object.entries(WIDGETS).map(([k, d]) => `  ${k.padEnd(10)}${d.n}`).join('\n'));
      WIDGETS[a] ? (addWidget(a), print('Добавлен виджет ' + a, 'inf')) : print('нет такого виджета', 'er'); });
    def('notify', 'показать уведомление', (a, raw) => { Shell.toast('Терминал', raw || 'Тест', '🔔'); print('OK', 'inf'); });
    def('echo', 'вывести текст', (a, raw) => print(esc(raw)));
    def('expr', 'вычислить выражение', (a, raw) => {
      try { print(String(Function('"use strict";return (' + raw.replace(/[^0-9+\-*/(). %]/g, '') + ')')())); }
      catch(e){ print('expr: не удалось вычислить', 'er'); } });
    def('history', 'история команд', () => hist.slice(-25).forEach((h, i) => print(`  ${i + 1}  ${esc(h)}`)));
    def('clear', 'очистить экран', () => $$('.term-line', out).forEach(n => n.remove()));
    def('exit', 'закрыть терминал', () => win.close());
    C.dir = C.ls; C.cls = C.clear; C.type = C.cat; C.del = C.rm; C.md = C.mkdir; C.cwd = C.pwd;

    function size(n){ return n.type === 'dir'
      ? Object.values(n.children || {}).reduce((s, c) => s + size(c), 0)
      : new Blob([n.body || '']).size + (n.img ? n.img.length : 0); }
    function lines(a, raw, head){
      const f = FS.node(resolve(a)); if (!f || f.type !== 'file') return print('файл не найден', 'er');
      const k = parseInt((raw.split(/\s+/)[1] || '10'), 10) || 10;
      const L = (f.body || '').split('\n');
      print(esc((head ? L.slice(0, k) : L.slice(-k)).join('\n')));
    }
    function move(raw, cut){
      const [src, dst] = raw.split(/\s+/);
      if (!src || !dst) return print('нужно: <что> <куда>', 'er');
      const node = FS.node(resolve(src));
      if (!node) return print('не найдено: ' + esc(src), 'er');
      const dstDir = FS.node(resolve(dst));
      if (dstDir && dstDir.type === 'dir'){
        FS.put(resolve(dst), JSON.parse(JSON.stringify(node)));
        if (cut) FS.rm(resolve(src).slice(0, -1), node.name, true);
      } else {                                   // переименование
        const p = resolve(src).slice(0, -1);
        if (cut) FS.rename(p, node.name, dst);
        else { const c = JSON.parse(JSON.stringify(node)); c.name = dst; FS.put(p, c); }
      }
      print(cut ? 'Перемещено.' : 'Скопировано.', 'inf'); Shell.renderIcons();
    }

    print(`<b class="inf">${Brand.name}</b> ${Brand.versionLine()} — Терминал`);
    print('Введите <b>help</b> для списка команд, Tab — автодополнение.\n');

    inp.addEventListener('keydown', e => {
      if (e.key === 'Enter'){
        const line = inp.value; inp.value = '';
        print(`<span class="pr">${esc(prompt.textContent)}</span> ${esc(line)}`);
        if (line.trim()){
          hist.push(line); hi = hist.length; KV.set('term.hist', hist.slice(-100));
          const [c, ...rest] = line.trim().split(/\s+/);
          const fn = C[c.toLowerCase()];
          if (fn){ try { fn(rest[0], rest.join(' ')); } catch(err){ print(esc(c) + ': ' + esc(err.message), 'er'); } }
          else print(`${esc(c)}: команда не найдена. help — список команд.`, 'er');
        }
        out.scrollTop = out.scrollHeight;
      }
      else if (e.key === 'ArrowUp'){ e.preventDefault(); if (hi > 0) inp.value = hist[--hi] || ''; }
      else if (e.key === 'ArrowDown'){ e.preventDefault(); hi = Math.min(hist.length, hi + 1); inp.value = hist[hi] || ''; }
      else if (e.key === 'Tab'){
        e.preventDefault();
        const parts = inp.value.split(/\s+/), w = parts[parts.length - 1] || '';
        const pool = parts.length <= 1 ? Object.keys(C)
          : Object.keys((FS.node(cwd) || {}).children || {});
        const hits = pool.filter(x => x.toLowerCase().startsWith(w.toLowerCase()));
        if (hits.length === 1){ parts[parts.length - 1] = hits[0]; inp.value = parts.join(' '); }
        else if (hits.length > 1) print(hits.join('   '), 'muted');
      }
      else if (e.key === 'l' && e.ctrlKey){ e.preventDefault(); C.clear(); }
    });
    out.onclick = () => { if (!getSelection().toString()) inp.focus(); };
    upd(); setTimeout(() => inp.focus(), 120);
  }
};

/* ==========================================================================
   4. Музыка: свои аудиофайлы + синтезатор
   ========================================================================== */
const SYNTH = [
  { t:'Liquid Dreams', a:'Синтезатор', scale:[0,3,5,7,10], root:220, bpm:96, e:'🌊', synth:true },
  { t:'Glass Horizon', a:'Синтезатор', scale:[0,2,4,7,9], root:261.6, bpm:112, e:'🪟', synth:true },
  { t:'Night Shift',   a:'Синтезатор', scale:[0,2,3,7,8], root:196, bpm:84, e:'🌙', synth:true }
];

APPS.music = {
  name:'Музыка', glyph:'🎵', bg:'linear-gradient(140deg,#fb7185,#a855f7)', w:560, h:660, single:true,
  render(win){
    let tracks = [...SYNTH], i = 0, playing = false, timer = null, step = 0, prog = 0, progTimer = null;
    const audio = new Audio(); audio.crossOrigin = 'anonymous';
    let srcNode = null, master = null, analyser = null;

    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const mu = el('div', 'mu');
    const bar = el('div', 'toolbar');
    const list = el('div', 'scroll'); list.style.cssText = 'max-height:210px;padding:0 12px 12px';
    wrap.append(mu, bar, list);
    mu.innerHTML = `<div class="mu-art">🎵</div>
      <div style="text-align:center"><div class="mu-title"></div><div class="mu-artist"></div></div>
      <div class="mu-bar"><i></i></div><div class="mu-time tiny muted"></div>
      <div class="mu-ctl"><button data-a="prev">⏮</button><button class="pp" data-a="pp">▶</button><button data-a="next">⏭</button></div>
      <div class="mu-viz"></div>`;
    const viz = $('.mu-viz', mu);
    for (let k = 0; k < 40; k++) viz.appendChild(el('i'));
    const bars = $$('i', viz);

    const addBtn = el('button', 'btn pri', '➕ Добавить аудио');
    const volRow = slider(() => S.volume, v => { Store.set('volume', v); audio.volume = v / 100; }, 0, 100, 1, v => v + '%');
    bar.append(addBtn, el('span', 'tiny muted', 'Громкость'), volRow);
    addBtn.onclick = () => pickFiles();

    /* --- звуковой граф --- */
    const ensure = () => {
      const c = Snd.ac(); if (!c) return null;
      if (c.state === 'suspended') c.resume();
      if (!master){
        master = c.createGain(); master.gain.value = .18;
        analyser = c.createAnalyser(); analyser.fftSize = 128;
        master.connect(analyser); analyser.connect(c.destination);
      }
      if (!srcNode){
        try { srcNode = c.createMediaElementSource(audio); srcNode.connect(analyser); analyser.connect(c.destination); }
        catch(e){}
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
      const tr = tracks[i]; if (!tr.synth) return;
      const deg = tr.scale[step % tr.scale.length], oct = Math.floor(step / tr.scale.length) % 3;
      note(tr.root * Math.pow(2, (deg + oct * 12) / 12), .55, 'triangle', .45);
      if (step % 4 === 0) note(tr.root / 2, .8, 'sine', .5);
      step++;
    };
    const drawViz = () => {
      if (analyser){
        const d = new Uint8Array(analyser.frequencyBinCount);
        analyser.getByteFrequencyData(d);
        bars.forEach((b, k) => b.style.height = Math.max(5, (d[k % d.length] / 255) * 100) + '%');
      }
      if (win.node.isConnected) requestAnimationFrame(drawViz);
    };

    /* --- свои файлы --- */
    async function loadUserTracks(){
      const meta = KV.get('music.files', []);
      const loaded = [];
      for (const m of meta){
        const blob = await IDB.get(m.key);
        if (blob) loaded.push({ t:m.t, a:m.a || 'Мои файлы', e:'🎧', url:URL.createObjectURL(blob), key:m.key });
      }
      tracks = [...loaded, ...SYNTH];
      drawList();
    }
    function pickFiles(){
      const inp = el('input'); inp.type = 'file'; inp.accept = 'audio/*'; inp.multiple = true;
      inp.onchange = () => addFiles(inp.files);
      inp.click();
    }
    async function addFiles(files){
      const meta = KV.get('music.files', []);
      for (const f of [...files]){
        if (!/^audio\//.test(f.type) && !/\.(mp3|wav|ogg|m4a|flac)$/i.test(f.name)) continue;
        const key = 'audio.' + Date.now() + '.' + Math.random().toString(36).slice(2, 7);
        await IDB.put(key, f);
        meta.push({ key, t:f.name.replace(/\.[^.]+$/, ''), a:'Мои файлы' });
      }
      KV.set('music.files', meta);
      await loadUserTracks();
      Shell.toast('Музыка', 'Аудио добавлено в библиотеку', '🎧');
    }
    win.data.addFiles = addFiles;

    /* приём перетащенных аудио прямо в окно */
    win.node.addEventListener('drop', e => {
      if (!e.dataTransfer.files.length) return;
      e.preventDefault(); e.stopPropagation(); addFiles(e.dataTransfer.files);
    }, true);
    win.node.addEventListener('dragover', e => e.preventDefault(), true);

    /* --- воспроизведение --- */
    const fmtT = s => isFinite(s) ? `${Math.floor(s / 60)}:${pad2(Math.floor(s % 60))}` : '0:00';
    const load = () => {
      const tr = tracks[i]; if (!tr) return;
      $('.mu-title', mu).textContent = tr.t;
      $('.mu-artist', mu).textContent = tr.a;
      $('.mu-art', mu).textContent = tr.e;
      win.setTitle(tr.t + ' — Музыка');
      step = 0; prog = 0;
      if (!tr.synth){ audio.src = tr.url; audio.volume = S.volume / 100; }
      else { audio.pause(); audio.removeAttribute('src'); }
      drawList();
      Shell.nowPlaying = playing ? tr : null;
      Shell.updateCC && Shell.updateCC();
    };
    const play = v => {
      playing = v;
      const tr = tracks[i];
      $('[data-a="pp"]', mu).textContent = v ? '⏸' : '▶';
      $('.mu-art', mu).classList.toggle('play', v);
      clearInterval(timer); clearInterval(progTimer);
      if (v){
        ensure();
        if (tr.synth){
          tick(); timer = setInterval(tick, 60000 / tr.bpm / 2);
          progTimer = setInterval(() => { prog = (prog + .4) % 100;
            $('.mu-bar i', mu).style.width = prog + '%'; }, 120);
        } else {
          audio.play().catch(() => Shell.toast('Музыка', 'Не удалось воспроизвести файл', '⚠️'));
          progTimer = setInterval(() => {
            $('.mu-bar i', mu).style.width = (audio.currentTime / (audio.duration || 1) * 100) + '%';
            $('.mu-time', mu).textContent = `${fmtT(audio.currentTime)} / ${fmtT(audio.duration)}`;
          }, 200);
        }
      } else audio.pause();
      Shell.nowPlaying = v ? tr : null;
      Shell.updateCC && Shell.updateCC();
    };
    audio.onended = () => { i = (i + 1) % tracks.length; load(); play(true); };

    $('[data-a="pp"]', mu).onclick = () => play(!playing);
    $('[data-a="next"]', mu).onclick = () => { i = (i + 1) % tracks.length; load(); if (playing) play(true); };
    $('[data-a="prev"]', mu).onclick = () => { i = (i - 1 + tracks.length) % tracks.length; load(); if (playing) play(true); };
    $('.mu-bar', mu).onclick = e => {
      const r = e.currentTarget.getBoundingClientRect(), f = (e.clientX - r.left) / r.width;
      if (!tracks[i].synth && audio.duration) audio.currentTime = f * audio.duration;
      else { prog = f * 100; $('.mu-bar i', mu).style.width = prog + '%'; }
    };

    function drawList(){
      list.innerHTML = '';
      tracks.forEach((t, k) => {
        const n = el('div', 'td-item' + (k === i ? ' done' : ''));
        n.innerHTML = `<span style="font-size:17px">${t.e}</span>
          <div class="tx">${esc(t.t)}<br><small class="muted">${esc(t.a)}</small></div>`;
        if (t.key){
          const d = el('button', 'td-del', '×');
          d.onclick = async e => {
            e.stopPropagation();
            await IDB.del(t.key);
            KV.set('music.files', KV.get('music.files', []).filter(m => m.key !== t.key));
            if (k === i){ play(false); i = 0; }
            await loadUserTracks(); load();
          };
          n.appendChild(d);
        }
        n.onclick = e => { if (e.target.classList.contains('td-del')) return; i = k; load(); play(true); };
        list.appendChild(n);
      });
      const hint = el('div', 'tiny muted', 'Перетащите сюда аудиофайлы или нажмите «Добавить аудио»');
      hint.style.padding = '8px 4px';
      list.appendChild(hint);
    }

    win.onClose = () => { clearInterval(timer); clearInterval(progTimer); audio.pause(); Shell.nowPlaying = null; };
    loadUserTracks().then(() => { load(); });
    load(); drawViz();
  }
};

/* ==========================================================================
   5. Автозапуск приложений
   ========================================================================== */
setTimeout(() => (S.autostart || []).forEach((id, k) => {
  if (APPS[id]) setTimeout(() => WM.open(id), k * 250);
}), 1200);

/* свои обои из IndexedDB */
IDB.get('wallpaper').then(b => {
  if (b){ window.__customWall = URL.createObjectURL(b); if (S.wallpaper === 'custom') applySettings(); }
}).catch(() => {});

/* применяем при старте */
Shell.renderDeskWidgets();
Shell.updateTaskbar();

})();
