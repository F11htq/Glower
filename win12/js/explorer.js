/* ==========================================================================
   Проводник как в Windows 11:
   вкладки, редактируемая адресная строка, поиск по всему компьютеру,
   сортировка кликом по заголовку столбца
   ========================================================================== */
'use strict';

(function explorer(){

const baseRender = APPS.files.render;          // вся цепочка предыдущих доработок

/* перерисовать окно на нужном пути, не создавая новое */
function mount(win, path){
  win.body.replaceChildren();
  baseRender.call(APPS.files, win, { path });
  decorate(win);
}

/* ==========================================================================
   Вкладки
   ========================================================================== */
function tabsBar(win){
  const d = win.data;
  const bar = el('div', 'fe-tabs');

  d.tabs.forEach((t, i) => {
    const name = t.path.length ? t.path[t.path.length - 1] : 'Этот компьютер';
    const b = el('div', 'fe-tab' + (i === d.tab ? ' on' : ''));
    b.innerHTML = `<span class="gl">${t.path.length ? '📁' : '💻'}</span><span class="nm">${esc(name)}</span>`;
    if (d.tabs.length > 1){
      const x = el('button', 'x', '×');
      x.onclick = e => { e.stopPropagation(); closeTab(win, i); };
      b.appendChild(x);
    }
    b.onclick = () => switchTab(win, i);
    b.oncontextmenu = e => {
      e.preventDefault();
      Shell.ctx(e.clientX, e.clientY, [
        { i:'➕', t:'Новая вкладка', f:() => newTab(win), k:'Ctrl+T' },
        { i:'✖', t:'Закрыть вкладку', f:() => closeTab(win, i), k:'Ctrl+W' },
        { i:'🪟', t:'Открыть в новом окне', f:() => WM.open('files', { path:t.path }) }
      ]);
    };
    bar.appendChild(b);
  });

  const add = el('button', 'fe-tab-add', '＋');
  add.dataset.tip = 'Новая вкладка (Ctrl+T)';
  add.onclick = () => newTab(win);
  bar.appendChild(add);
  return bar;
}

function switchTab(win, i){
  const d = win.data;
  if (!d.tabs[i]) return;
  d.tab = i;
  mount(win, d.tabs[i].path);
}
function newTab(win){
  win.data.tabs.push({ path:['Документы'] });
  switchTab(win, win.data.tabs.length - 1);
}
function closeTab(win, i){
  const d = win.data;
  if (d.tabs.length <= 1) return win.close();
  d.tabs.splice(i, 1);
  switchTab(win, Math.min(d.tab, d.tabs.length - 1));
}

/* ==========================================================================
   Адресная строка, поиск везде, сортировка по заголовкам
   ========================================================================== */
function decorate(win){
  const d = win.data;
  const bar = $$('.toolbar', win.body)[0];
  const bar2 = $$('.toolbar', win.body)[1];
  const list = $('.scroll', win.body);
  const crumbs = $('.fe-crumbs', win.body);
  const find = $('.fe-find', win.body);
  if (!bar || !list) return;

  /* вкладки сверху */
  d.tabs = d.tabs || [{ path:(d.path || []).slice() }];
  d.tab = d.tab || 0;
  if (d.tabs[d.tab]) d.tabs[d.tab].path = (d.path || []).slice();
  win.body.style.flexDirection = 'column';    // иначе вкладки встанут колонкой слева
  win.body.prepend(tabsBar(win));

  /* обновляем подписи вкладок после навигации внутри */
  win.body.addEventListener('click', () => setTimeout(() => {
    if (!d.tabs[d.tab]) return;
    if (String(d.tabs[d.tab].path) !== String(d.path)){
      d.tabs[d.tab].path = (d.path || []).slice();
      const old = $('.fe-tabs', win.body);
      if (old) old.replaceWith(tabsBar(win));
    }
  }, 60));

  /* --- адресная строка --- */
  if (crumbs){
    const edit = el('button', 'fe-path-edit', '✎');
    edit.dataset.tip = 'Ввести путь';
    const openEditor = () => {
      const inp = el('input', 'inp fe-path-inp');
      inp.value = '/' + (d.path || []).join('/');
      crumbs.style.display = 'none';
      edit.style.display = 'none';
      crumbs.parentElement.insertBefore(inp, crumbs);
      inp.focus(); inp.select();
      let closed = false;
      const done = ok => {
        if (closed) return;                       // remove() вызывает blur — второй раз не входим
        closed = true;
        inp.onblur = null;
        const v = inp.value.trim();
        if (inp.parentElement) inp.remove();
        crumbs.style.display = ''; edit.style.display = '';
        if (!ok) return;
        const parts = v.split('/').filter(Boolean);
        const node = FS.node(parts);
        if (node && node.type === 'dir'){ d.tabs[d.tab].path = parts; mount(win, parts); }
        else Shell.toast('Проводник', 'Папка не найдена: ' + v, '⚠️');
      };
      inp.onkeydown = e => { e.stopPropagation();
        if (e.key === 'Enter') done(true);
        if (e.key === 'Escape') done(false); };
      inp.onblur = () => done(false);
    };
    edit.onclick = openEditor;
    crumbs.parentElement.insertBefore(edit, crumbs.nextSibling);
    crumbs.addEventListener('dblclick', openEditor);
  }

  /* --- поиск по всему компьютеру --- */
  if (find){
    const all = el('button', 'btn fe-all', '🌐 Везде');
    all.dataset.tip = 'Искать во всех папках';
    let global = false;
    all.onclick = () => {
      global = !global;
      all.classList.toggle('pri', global);
      find.placeholder = global ? '🔎 Поиск по компьютеру' : '🔎 Поиск в папке';
      if (find.value.trim()) run(find.value.trim());
      else if (!global && d.refresh) d.refresh();
    };
    find.parentElement.insertBefore(all, find.nextSibling);

    const run = q => {
      if (!global) return;
      const res = Search.scan(q);

      list.innerHTML = '';
      const inside = res.filter(r => r.hit).length;
      const head = el('div', 'fe-search-head',
        `Найдено: ${res.length} · по всему компьютеру` + (inside ? ` · из них ${inside} по содержимому` : ''));
      list.appendChild(head);
      const box = el('div', 'fe-table');
      res.slice(0, 200).forEach(r => {
        const n = el('div', 'fe-tr');
        n.dataset.name = r.node.name;
        n.innerHTML = `<div class="c1"><span class="gl">${r.node.type === 'dir' ? '📁' : '📄'}</span>${esc(r.node.name)}
            ${r.hit ? `<div class="fe-hit tiny">…${esc(r.hit)}…</div>` : ''}</div>
          <div class="tiny muted">/${esc(r.path.join('/'))}</div><div></div><div></div>`;
        n.ondblclick = () => {
          if (r.node.type === 'dir'){ d.tabs[d.tab].path = [...r.path, r.node.name]; mount(win, [...r.path, r.node.name]); }
          else Assoc.open(r.node, r.path);
        };
        n.onclick = () => { $$('.fe-tr', box).forEach(x => x.classList.remove('sel')); n.classList.add('sel'); };
        n.oncontextmenu = e => { e.preventDefault();
          Shell.ctx(e.clientX, e.clientY, [
            { i:'📂', t:'Открыть', f:() => n.ondblclick() },
            { i:'📁', t:'Показать папку', f:() => { d.tabs[d.tab].path = r.path.slice(); mount(win, r.path.slice()); } },
            { i:'ℹ️', t:'Свойства', f:() => WM.open('props', { node:r.node, path:r.path }) }
          ]); };
        box.appendChild(n);
      });
      if (!res.length) box.appendChild(el('div', 'empty', 'Ничего не найдено'));
      list.appendChild(box);
    };
    find.addEventListener('input', () => { if (global) run(find.value.trim()); });
  }

  /* --- сортировка кликом по заголовку --- */
  const sortSel = bar2 && $('[data-a="sort"]', bar2);
  const applySort = () => {
    const head = $('.fe-tr.head', list);
    if (!head || !sortSel) return;
    const cols = ['name', 'date', 'type', 'size'];
    [...head.children].forEach((c, i) => {
      c.classList.add('sortable');
      if (sortSel.value === cols[i]) c.classList.add('sorted');
      c.onclick = () => { sortSel.value = cols[i]; sortSel.dispatchEvent(new Event('change')); setTimeout(applySort, 60); };
    });
  };
  setTimeout(applySort, 60);
  list.addEventListener('click', () => setTimeout(applySort, 60));

  /* --- клавиатура вкладок --- */
  if (!win.data._tabKeys){
    win.data._tabKeys = true;
    win.node.addEventListener('keydown', e => {
      if (/INPUT|TEXTAREA/.test(e.target.tagName)) return;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't'){ e.preventDefault(); e.stopImmediatePropagation(); newTab(win); }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'w'){ e.preventDefault(); e.stopImmediatePropagation(); closeTab(win, win.data.tab); }
      if ((e.ctrlKey || e.metaKey) && e.key === 'l'){ e.preventDefault(); const b = $('.fe-path-edit', win.body); if (b) b.click(); }
    }, true);
  }
}

/* открытие папки из другого места добавляет вкладку в активном окне */
APPS.files.render = function(win, opts){
  baseRender.call(this, win, opts);
  decorate(win);
};

})();
