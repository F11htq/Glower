/* ==========================================================================
   Файловая подсистема второго поколения:
   метаданные, буфер обмена, полноценный Проводник,
   импорт настоящих файлов перетаскиванием и выгрузка на реальный диск
   ========================================================================== */
'use strict';

(function fs2(){

/* ---------- метаданные ---------- */
const sizeOf = n => n.type === 'dir'
  ? Object.values(n.children || {}).reduce((s, c) => s + sizeOf(c), 0)
  : (n.img ? Math.round(n.img.length * 0.75) : new Blob([n.body || '']).size);
const fmtSize = b => b < 1024 ? b + ' Б' : b < 1048576 ? (b / 1024).toFixed(1) + ' КБ' : (b / 1048576).toFixed(2) + ' МБ';
const fmtDate = t => t ? new Date(t).toLocaleString('ru-RU', { day:'2-digit', month:'2-digit', year:'numeric', hour:'2-digit', minute:'2-digit' }) : '—';
const extOf = n => n.type === 'dir' ? 'Папка' : (n.name.match(/\.([a-z0-9]+)$/i) || [,''])[1].toUpperCase() + ' файл';
const glyphOf = n => n.type === 'dir' ? '📁' : n.img ? '🖼️' : /\.(md|txt|log)$/i.test(n.name) ? '📄'
  : /\.(js|json|css|html|py)$/i.test(n.name) ? '📜' : /\.(mp3|wav|ogg)$/i.test(n.name) ? '🎵' : '📦';

const writeOrig = FS.write.bind(FS);
FS.write = function(path, name, body){
  const d = this.node(path); if (!d || d.type !== 'dir') return false;
  const old = d.children[name];
  const r = writeOrig(path, name, body);
  const n = d.children[name];
  if (n){ n.ctime = (old && old.ctime) || Date.now(); n.mtime = Date.now(); this.save(); }
  return r;
};
const mkdirOrig = FS.mkdir.bind(FS);
FS.mkdir = function(path, name){
  const r = mkdirOrig(path, name);
  const n = this.node([...path, name]);
  if (n){ n.ctime = n.mtime = Date.now(); this.save(); }
  return r;
};
FS.put = function(path, node){                    // положить готовый узел
  const d = this.node(path); if (!d) return false;
  let name = node.name, i = 1;
  while (d.children[name]) name = node.name.replace(/(\.[^.]+)?$/, ` (${i++})$1`);
  d.children[name] = { ...node, name, ctime:node.ctime || Date.now(), mtime:Date.now() };
  this.save(); return name;
};
FS.uniqueName = function(path, name){
  const d = this.node(path); if (!d) return name;
  let n = name, i = 1;
  while (d.children[n]) n = name.replace(/(\.[^.]+)?$/, ` (${i++})$1`);
  return n;
};

/* даты для файлов, созданных до появления метаданных */
(function migrate(){
  if (KV.get('fs.migrated', false)) return;
  const t = Date.now() - 86400000;
  (function walk(n){ Object.values(n.children || {}).forEach(c => {
    if (!c.ctime) c.ctime = t;
    if (!c.mtime) c.mtime = t;
    if (c.type === 'dir') walk(c);
  }); })(FS.root);
  FS.save(); KV.set('fs.migrated', true);
})();

/* ---------- буфер обмена ---------- */
const Clip = { items:[], cut:false,
  copy(path, names){ this.items = names.map(n => ({ path:path.slice(), name:n })); this.cut = false; },
  take(path, names){ this.copy(path, names); this.cut = true; },
  paste(path){
    if (!this.items.length) return 0;
    let k = 0;
    this.items.forEach(it => {
      const src = FS.node([...it.path, it.name]); if (!src) return;
      if (String(it.path) === String(path) && this.cut) return;
      FS.put(path, JSON.parse(JSON.stringify(src)));
      if (this.cut) FS.rm(it.path, it.name, true);
      k++;
    });
    if (this.cut) this.items = [];
    return k;
  }
};

/* ---------- выгрузка на настоящий диск ---------- */
async function download(node){
  // если браузер умеет File System Access — показываем настоящий диалог сохранения
  if (window.showSaveFilePicker){
    try {
      const h = await showSaveFilePicker({ suggestedName:node.name });
      const w = await h.createWritable();
      await w.write(node.img ? await (await fetch(node.img)).blob() : new Blob([node.body || ''], { type:'text/plain;charset=utf-8' }));
      await w.close();
      Shell.toast('Сохранено', node.name + ' записан на диск', '💾');
      return;
    } catch(e){ if (e && e.name === 'AbortError') return; }   // пользователь отменил
  }
  const a = document.createElement('a');
  a.setAttribute('download', node.name || 'file.txt');
  // data: вместо blob: — при открытии по file:// blob имеет opaque origin
  // и Chromium игнорирует имя файла из атрибута download
  a.href = node.img || ('data:text/plain;charset=utf-8,' + encodeURIComponent(node.body || ''));
  a.style.display = 'none';
  document.body.appendChild(a); a.click();
  setTimeout(() => a.remove(), 1000);
  Shell.toast('Выгружено', node.name + ' сохранён на ваш компьютер', '⬇️');
}

/* ---------- импорт настоящих файлов ---------- */
function importFiles(fileList, path){
  const files = [...fileList];
  let done = 0;
  files.forEach(f => {
    const r = new FileReader();
    const img = /^image\//.test(f.type);
    r.onload = () => {
      const node = img ? { type:'file', name:f.name, img:r.result, body:'' }
                       : { type:'file', name:f.name, body:String(r.result).slice(0, 400000) };
      node.ctime = f.lastModified; node.mtime = f.lastModified;
      FS.put(path, node);
      if (++done === files.length){
        Shell.toast('Импортировано', `${files.length} ф. → /${path.join('/')}`, '📥');
        Shell.renderIcons();
        WM.wins.filter(w => w.appId === 'files' && w.data.refresh).forEach(w => w.data.refresh());
      }
    };
    if (img) r.readAsDataURL(f); else r.readAsText(f);
  });
}
window.__glowerImport = importFiles;

/* приём файлов, перетащенных из настоящей ОС на рабочий стол */
const dropHint = el('div', 'drop-hint', '<div>📥<br>Отпустите — файлы попадут на рабочий стол</div>');
document.body.appendChild(dropHint);
let dragDepth = 0;
addEventListener('dragenter', e => {
  if (!e.dataTransfer || ![...e.dataTransfer.types].includes('Files')) return;
  e.preventDefault();
  if (++dragDepth === 1 && !e.target.closest('.win')) dropHint.classList.add('on');
});
addEventListener('dragover', e => { if ([...(e.dataTransfer||{types:[]}).types].includes('Files')) e.preventDefault(); });
addEventListener('dragleave', e => { if (--dragDepth <= 0){ dragDepth = 0; dropHint.classList.remove('on'); } });
addEventListener('drop', e => {
  if (!e.dataTransfer || !e.dataTransfer.files.length) return;
  e.preventDefault(); dragDepth = 0; dropHint.classList.remove('on');
  const win = e.target.closest('.win');
  const w = win && WM.wins.find(x => x.node === win);
  const path = (w && w.appId === 'files' && w.data.path) ? w.data.path : ['Рабочий стол'];
  importFiles(e.dataTransfer.files, path);
});

/* ==========================================================================
   ПРОВОДНИК
   ========================================================================== */
APPS.files = {
  name:'Проводник', glyph:'📁', bg:'linear-gradient(140deg,#fcd34d,#f59e0b)', w:940, h:600,
  render(win, opts){
    let path = [], hist = [], hp = -1, sel = [], last = null;
    let view = KV.get('files.view', 'grid'), sort = KV.get('files.sort', 'name'), q = '';

    const wrap = el('div', 'app'); win.body.appendChild(wrap);
    const side = el('div', 'sidebar');
    const main = el('div', 'col grow');
    const bar = el('div', 'toolbar');
    const bar2 = el('div', 'toolbar');
    const list = el('div', 'scroll');
    const status = el('div', 'statusbar');
    main.append(bar, bar2, list, status); wrap.append(side, main);

    /* --- боковая панель --- */
    const quick = [['🏠','Этот компьютер',[]],['🖥️','Рабочий стол',['Рабочий стол']],['📄','Документы',['Документы']],
                   ['🖼️','Изображения',['Изображения']],['🎵','Музыка',['Музыка']],['⬇️','Загрузки',['Загрузки']]];
    side.appendChild(el('div', 'sb-title', 'Быстрый доступ'));
    quick.forEach(([e, n, p]) => {
      const b = el('button', 'sb-item', `<span>${e}</span><span>${n}</span>`);
      b.onclick = () => go(p);
      b.ondragover = ev => { ev.preventDefault(); b.classList.add('on'); };
      b.ondragleave = () => b.classList.remove('on');
      b.ondrop = ev => {
        ev.preventDefault(); ev.stopPropagation(); b.classList.remove('on');
        if (ev.dataTransfer.files.length) return importFiles(ev.dataTransfer.files, p);
        const names = JSON.parse(ev.dataTransfer.getData('text/glower') || '[]');
        if (names.length){ Clip.take(path, names); Clip.paste(p); draw(); }
      };
      side.appendChild(b);
    });
    side.appendChild(el('div', 'sb-title', 'Система'));
    const tr = el('button', 'sb-item', '<span>🗑️</span><span>Корзина</span>');
    tr.onclick = () => WM.open('trash');
    side.appendChild(tr);

    /* --- панели инструментов --- */
    bar.innerHTML = `<button class="btn" data-a="back" title="Назад">‹</button>
      <button class="btn" data-a="fwd" title="Вперёд">›</button>
      <button class="btn" data-a="up" title="Вверх">↑</button>
      <div class="fe-crumbs"></div>
      <input class="inp fe-find" placeholder="🔎 Поиск в папке" style="width:170px">`;
    bar2.innerHTML = `<button class="btn" data-a="nf">📁 Папка</button>
      <button class="btn" data-a="nt">📄 Файл</button>
      <span class="fe-sep"></span>
      <button class="btn" data-a="copy">⧉ Копировать</button>
      <button class="btn" data-a="cut">✂ Вырезать</button>
      <button class="btn" data-a="paste">📋 Вставить</button>
      <button class="btn" data-a="ren">✏️ Имя</button>
      <button class="btn" data-a="del">🗑️ Удалить</button>
      <button class="btn" data-a="dl">⬇️ Скачать</button>
      <div class="grow"></div>
      <select class="inp" data-a="sort" style="width:130px">
        <option value="name">По имени</option><option value="size">По размеру</option>
        <option value="date">По дате</option><option value="type">По типу</option></select>
      <button class="btn" data-a="view">▦</button>`;
    const crumbs = $('.fe-crumbs', bar);
    const find = $('.fe-find', bar);
    $('[data-a="sort"]', bar2).value = sort;

    /* --- навигация --- */
    const go = (p, push = true) => {
      path = p.slice(); sel = []; q = ''; find.value = '';
      if (push){ hist.splice(hp + 1); hist.push(path.slice()); hp = hist.length - 1; }
      win.data.path = path;
      draw();
    };
    win.data.refresh = () => draw();

    /* --- отрисовка --- */
    function items(){
      const node = FS.node(path) || FS.root;
      let arr = Object.values(node.children || {});
      if (q) arr = arr.filter(i => i.name.toLowerCase().includes(q));
      const dir = (a, b) => a.type === b.type ? 0 : a.type === 'dir' ? -1 : 1;
      arr.sort((a, b) => dir(a, b) || (
        sort === 'size' ? sizeOf(b) - sizeOf(a) :
        sort === 'date' ? (b.mtime || 0) - (a.mtime || 0) :
        sort === 'type' ? extOf(a).localeCompare(extOf(b)) || a.name.localeCompare(b.name) :
        a.name.localeCompare(b.name)));
      return arr;
    }

    function draw(){
      const node = FS.node(path) || FS.root;
      win.setTitle((path.length ? path[path.length - 1] : 'Этот компьютер') + ' — Проводник');
      win.setSub('/' + path.join('/'));

      crumbs.innerHTML = '';
      const home = el('button', '', '💻 Этот компьютер'); home.onclick = () => go([]); crumbs.appendChild(home);
      path.forEach((p, i) => {
        crumbs.appendChild(el('span', 'muted', '›'));
        const b = el('button', '', esc(p)); b.onclick = () => go(path.slice(0, i + 1)); crumbs.appendChild(b);
      });

      const arr = items();
      list.innerHTML = '';
      const cont = el('div', view === 'grid' ? 'fe-grid' : 'fe-table');

      if (view === 'table'){
        const h = el('div', 'fe-tr head');
        h.innerHTML = `<div>Имя</div><div>Дата изменения</div><div>Тип</div><div>Размер</div>`;
        cont.appendChild(h);
      }

      arr.forEach(it => {
        const n = el('div', view === 'grid' ? 'fe-it' : 'fe-tr');
        n.innerHTML = view === 'grid'
          ? `<div class="gl">${glyphOf(it)}</div><div class="nm">${esc(it.name)}</div>`
          : `<div class="c1"><span class="gl">${glyphOf(it)}</span>${esc(it.name)}</div>
             <div>${fmtDate(it.mtime)}</div><div>${extOf(it)}</div>
             <div>${it.type === 'dir' ? '' : fmtSize(sizeOf(it))}</div>`;
        n.dataset.name = it.name;
        n.classList.toggle('sel', sel.includes(it.name));
        n.draggable = true;

        n.onclick = e => {
          e.stopPropagation();
          if (e.shiftKey && last){
            const names = arr.map(x => x.name);
            const a = names.indexOf(last), b = names.indexOf(it.name);
            sel = names.slice(Math.min(a, b), Math.max(a, b) + 1);
          } else if (e.ctrlKey || e.metaKey){
            sel = sel.includes(it.name) ? sel.filter(x => x !== it.name) : [...sel, it.name];
            last = it.name;
          } else { sel = [it.name]; last = it.name; }
          paint();
        };
        n.ondblclick = () => open(it);
        n.ondragstart = e => {
          if (!sel.includes(it.name)) { sel = [it.name]; paint(); }
          e.dataTransfer.setData('text/glower', JSON.stringify(sel));
          e.dataTransfer.effectAllowed = 'move';
        };
        if (it.type === 'dir'){
          n.ondragover = e => { e.preventDefault(); n.classList.add('drop'); };
          n.ondragleave = () => n.classList.remove('drop');
          n.ondrop = e => {
            e.preventDefault(); e.stopPropagation(); n.classList.remove('drop');
            if (e.dataTransfer.files.length) return importFiles(e.dataTransfer.files, [...path, it.name]);
            const names = JSON.parse(e.dataTransfer.getData('text/glower') || '[]').filter(x => x !== it.name);
            if (!names.length) return;
            Clip.take(path, names); Clip.paste([...path, it.name]); sel = []; draw();
          };
        }
        n.oncontextmenu = e => {
          e.preventDefault(); e.stopPropagation();
          if (!sel.includes(it.name)){ sel = [it.name]; paint(); }
          Shell.ctx(e.clientX, e.clientY, [
            { i:'📂', t:'Открыть', f:() => open(it), k:'Enter' },
            'hr',
            { i:'⧉', t:'Копировать', f:() => act('copy'), k:'Ctrl+C' },
            { i:'✂', t:'Вырезать', f:() => act('cut'), k:'Ctrl+X' },
            { i:'✏️', t:'Переименовать', f:() => act('ren'), k:'F2' },
            { i:'⬇️', t:'Скачать на компьютер', f:() => act('dl') },
            'hr',
            { i:'🗑️', t:'Удалить', f:() => act('del'), k:'Del' },
            { i:'ℹ️', t:'Свойства', f:() => window.WM.open('props', { node:it, path:path.slice() }) }
          ]);
        };
        cont.appendChild(n);
      });

      if (!arr.length) cont.appendChild(el('div', 'empty', q ? 'Ничего не найдено' : 'Папка пуста'));
      list.appendChild(cont);
      paint();
    }

    function paint(){
      $$('.fe-it[data-name], .fe-tr[data-name]', list)
        .forEach(n => n.classList.toggle('sel', sel.includes(n.dataset.name)));
      const node = FS.node(path) || FS.root;
      const total = Object.keys(node.children || {}).length;
      const selSize = sel.reduce((s, n) => { const x = FS.node([...path, n]); return s + (x ? sizeOf(x) : 0); }, 0);
      status.innerHTML = `<span>Элементов: ${total}</span>
        ${sel.length ? `<span>Выбрано: ${sel.length} · ${fmtSize(selSize)}</span>` : ''}
        <span class="grow"></span>
        ${Clip.items.length ? `<span>В буфере: ${Clip.items.length}${Clip.cut ? ' (вырезано)' : ''}</span>` : ''}
        <span>${view === 'grid' ? 'Плитка' : 'Таблица'}</span>`;
    }

    function open(it){
      if (it.type === 'dir') return go([...path, it.name]);
      if (opts && opts.pick){ opts.pick({ name:it.name, path:path.slice(), body:it.body }); win.close(); return; }
      if (window.Assoc) return Assoc.open(it, path.slice());      // файловые ассоциации
      if (it.img) WM.open('photos', { img:it.img, name:it.name });
      else WM.open('notepad', { file:{ name:it.name, path:path.slice(), body:it.body } });
    }


    /* --- действия --- */
    async function act(a){
      switch(a){
        case 'back': if (hp > 0){ hp--; go(hist[hp], false); } break;
        case 'fwd':  if (hp < hist.length - 1){ hp++; go(hist[hp], false); } break;
        case 'up':   if (path.length) go(path.slice(0, -1)); break;
        case 'nf': { const n = await Dlg.prompt('Создать папку', 'Имя новой папки', 'Новая папка', '📁');
          if (n){ FS.mkdir(path, FS.uniqueName(path, n)); draw(); } break; }
        case 'nt': { const n = await Dlg.prompt('Создать файл', 'Имя нового файла', 'Новый.txt', '📄');
          if (n){ FS.write(path, FS.uniqueName(path, n), ''); draw(); } break; }
        case 'copy': if (sel.length){ Clip.copy(path, sel); paint(); Shell.toast('Скопировано', sel.length + ' эл. в буфере', '⧉'); } break;
        case 'cut':  if (sel.length){ Clip.take(path, sel); paint(); } break;
        case 'paste': { const k = Clip.paste(path); if (k){ draw(); Shell.toast('Вставлено', k + ' эл.', '📋'); } break; }
        case 'ren': { if (sel.length !== 1) return;
          const nn = await Dlg.prompt('Переименовать', 'Новое имя', sel[0], '✏️');
          if (nn && nn !== sel[0]){ FS.rename(path, sel[0], nn); sel = [nn]; draw(); } break; }
        case 'del': { if (!sel.length) return;
          if (!await Dlg.confirm('Удалить в корзину',
              sel.length === 1 ? `«${sel[0]}» будет перемещён в корзину.` : `${sel.length} элементов будет перемещено в корзину.`,
              { icon:'🗑️', okText:'Удалить', danger:true })) return;
          sel.forEach(n => FS.rm(path, n)); sel = []; draw(); Shell.renderIcons(); break; }
        case 'dl': { sel.forEach(n => { const x = FS.node([...path, n]); if (x && x.type === 'file') download(x); }); break; }
        case 'view': view = view === 'grid' ? 'table' : 'grid'; KV.set('files.view', view);
          $('[data-a="view"]', bar2).textContent = view === 'grid' ? '▦' : '☰'; draw(); break;
      }
    }
    $$('[data-a]', bar).forEach(b => b.onclick = () => act(b.dataset.a));
    $$('[data-a]', bar2).forEach(b => { if (b.tagName === 'BUTTON') b.onclick = () => act(b.dataset.a); });
    $('[data-a="sort"]', bar2).onchange = e => { sort = e.target.value; KV.set('files.sort', sort); draw(); };
    find.oninput = () => { q = find.value.trim().toLowerCase(); draw(); };

    /* --- клавиатура --- */
    win.node.addEventListener('keydown', e => {
      if (/INPUT|TEXTAREA/.test(e.target.tagName)) return;
      const arr = items();
      const k = e.key.toLowerCase();
      if ((e.ctrlKey || e.metaKey) && k === 'a'){ e.preventDefault(); sel = arr.map(i => i.name); paint(); }
      else if ((e.ctrlKey || e.metaKey) && k === 'c'){ act('copy'); }
      else if ((e.ctrlKey || e.metaKey) && k === 'x'){ act('cut'); }
      else if ((e.ctrlKey || e.metaKey) && k === 'v'){ act('paste'); }
      else if (e.key === 'Delete'){ act('del'); }
      else if (e.key === 'F2'){ act('ren'); }
      else if (e.key === 'Backspace'){ act('up'); }
      else if (e.key === 'Enter'){ const it = FS.node([...path, sel[0]]); if (it) open(it); }
      else if (e.key === 'Escape'){ sel = []; paint(); }
    });

    /* --- фон списка --- */
    list.onclick = () => { sel = []; paint(); };
    list.oncontextmenu = e => {
      if (e.target.closest('.fe-it, .fe-tr')) return;
      e.preventDefault();
      Shell.ctx(e.clientX, e.clientY, [
        { i:'📁', t:'Создать папку', f:() => act('nf') },
        { i:'📄', t:'Создать файл', f:() => act('nt') },
        { i:'📋', t:'Вставить', f:() => act('paste'), k:'Ctrl+V' },
        'hr',
        { i:'🔄', t:'Обновить', f:draw, k:'F5' },
        { i:'📥', t:'Импорт с компьютера…', f:() => {
            const inp = el('input'); inp.type = 'file'; inp.multiple = true;
            inp.onchange = () => importFiles(inp.files, path); inp.click(); } }
      ]);
    };
    list.ondragover = e => { if (e.dataTransfer.types.includes('Files')) e.preventDefault(); };

    win.node.tabIndex = 0;
    go(opts && opts.path ? opts.path : ['Документы']);
  }
};

/* --------------------------------------------------------------------------
   Один конвейер вместо цепочки обёрток.

   Раньше каждый следующий файл заворачивал APPS.files.render в свою функцию,
   и получалось четыре слоя, в которых уже дважды заводились ошибки. Теперь
   базовая отрисовка одна, а дополнения просто встают в очередь и вызываются
   по порядку: dialogs → fileops → tabs.
   -------------------------------------------------------------------------- */
APPS.files.paint = APPS.files.render;      // базовая отрисовка, без дополнений
APPS.files.parts = [];                     // очередь дополнений

APPS.files.use = function(name, fn){ this.parts.push({ name, fn }); };

APPS.files.render = function(win, opts){
  this.paint(win, opts);
  this.parts.forEach(p => {
    try { p.fn(win, opts); }
    catch(e){ console.error('Проводник · дополнение «' + p.name + '»:', e); }
  });
};

/* ==========================================================================
   Блокнот: поиск/замена и «Скачать»
   ========================================================================== */
const npRender = APPS.notepad.render;
APPS.notepad.render = function(win, opts){
  npRender.call(this, win, opts);
  const bar = $('.toolbar', win.body), area = $('.np-area', win.body);
  if (!bar || !area) return;

  const dl = el('button', 'btn', '⬇️ Скачать');
  dl.onclick = () => download({ name:(win.titleEl.textContent.split(' — ')[0] || 'file.txt'), body:area.value });
  const fnd = el('button', 'btn', '🔎 Найти');
  bar.insertBefore(fnd, bar.querySelector('.grow'));
  bar.insertBefore(dl, bar.querySelector('.grow'));

  const panel = el('div', 'np-find');
  panel.innerHTML = `<input class="inp" placeholder="Найти"><input class="inp" placeholder="Заменить на">
    <button class="btn" data-a="next">↓</button><button class="btn" data-a="all">Заменить всё</button>
    <span class="tiny muted cnt"></span><button class="btn" data-a="close">×</button>`;
    win.body.querySelector('.app').insertBefore(panel, area);
  const [qi, ri] = $$('input', panel);
  let pos = 0;
  const count = () => {
    const v = qi.value; if (!v) return $('.cnt', panel).textContent = '';
    const n = area.value.split(v).length - 1;
    $('.cnt', panel).textContent = n ? `совпадений: ${n}` : 'не найдено';
  };
  const next = () => {
    const v = qi.value; if (!v) return;
    const i = area.value.indexOf(v, pos);
    const j = i < 0 ? area.value.indexOf(v) : i;
    if (j < 0) return;
    area.focus(); area.setSelectionRange(j, j + v.length); pos = j + v.length;
  };
  qi.oninput = () => { pos = 0; count(); };
  qi.onkeydown = e => { if (e.key === 'Enter') next(); if (e.key === 'Escape') panel.classList.remove('on'); };
  $('[data-a="next"]', panel).onclick = next;
  $('[data-a="all"]', panel).onclick = () => {
    if (!qi.value) return;
    const n = area.value.split(qi.value).length - 1;
    area.value = area.value.split(qi.value).join(ri.value);
    area.dispatchEvent(new Event('input')); count();
    Shell.toast('Блокнот', `Заменено: ${n}`, '🔁');
  };
  $('[data-a="close"]', panel).onclick = () => panel.classList.remove('on');
  const openFind = () => { panel.classList.add('on'); qi.focus(); qi.select(); count(); };
  fnd.onclick = openFind;
  area.addEventListener('keydown', e => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f'){ e.preventDefault(); openFind(); }
  });
};

/* ==========================================================================
   Полноэкранный режим и системные уведомления
   ========================================================================== */
addEventListener('keydown', e => {
  if (e.key === 'F11'){
    e.preventDefault();
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen().catch(() => {});
  }
});
const toastOrig = Shell.toast.bind(Shell);
Shell.toast = function(title, text, icon, ms){
  toastOrig(title, text, icon, ms);
  if (document.hidden && window.Notification && Notification.permission === 'granted')
    try { new Notification(title, { body:text }); } catch(e){}
};
/* Уведомления самого браузера нужны только когда система живёт во вкладке:
   там она может оказаться в фоне. В своём сеансе это единственное окно на
   экране — просить разрешение незачем, и окно браузера поверх рабочего
   стола выглядело бы как чужое. */
document.addEventListener('click', function once(){
  const ownSession = matchMedia('(display-mode: fullscreen)').matches
    || location.search.includes('session=1')
    || document.body.classList.contains('os-native');
  if (!ownSession && window.Notification && Notification.permission === 'default')
    Notification.requestPermission().catch(() => {});
  document.removeEventListener('click', once);
}, { once:true });

})();
