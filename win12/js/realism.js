/* ==========================================================================
   Приближение к поведению настоящих ОС:
   свободные значки стола, реальная Корзина, автоскрытие верхней панели,
   превью окон в доке, Snap Assist, «Выполнить», системное меню окна,
   Aero Shake, настоящие батарея и сеть
   ========================================================================== */
'use strict';

(function realism(){

/* ==========================================================================
   1. Верхняя панель прячется, когда открыто хоть одно окно
   ========================================================================== */
const hot = el('div', 'top-hotzone');
$('#desktop').appendChild(hot);

Shell.updateChrome = function(){
  const busy = WM.wins.some(w => !w.minimized && w.desk === WM.desk);
  const peek = document.body.classList.contains('top-peek')
    || document.body.classList.contains('start-open')
    || $('#cc').classList.contains('on') || $('#widgets').classList.contains('on')
    || $('#taskview').classList.contains('on');
  const hide = S.topbarAutohide !== false && busy && !peek;
  document.body.classList.toggle('chrome-hidden', hide);
  const dw = $('#desk-widgets');
  if (dw) {                       // виджеты стола прячутся вместе с панелью
    dw.style.transition = 'opacity var(--t3) var(--e-io), transform var(--t3) var(--e-out)';
    dw.style.opacity = (busy && S.topbarAutohide !== false) ? '0' : '';
    dw.style.transform = (busy && S.topbarAutohide !== false) ? 'translateY(18px)' : '';
    dw.style.pointerEvents = busy ? 'none' : '';
  }
};
hot.addEventListener('mouseenter', () => { document.body.classList.add('top-peek'); Shell.updateChrome(); });
addEventListener('mousemove', e => {
  if (!document.body.classList.contains('top-peek')) return;
  if (e.clientY > 96){ document.body.classList.remove('top-peek'); Shell.updateChrome(); }
});
['syncDock','toggleStart','closePanels','panel','taskview'].forEach(fn => {
  const orig = Shell[fn].bind(Shell);
  Shell[fn] = (...a) => { const r = orig(...a); Shell.updateChrome(); return r; };
});

/* ==========================================================================
   2. Корзина: реальное удаление с восстановлением
   ========================================================================== */
const rmOrig = FS.rm.bind(FS);
FS.rm = function(path, name, permanent){
  const d = this.node(path), it = d && d.children[name];
  if (it && !permanent){
    const t = KV.get('trash', []);
    t.unshift({ name, path:path.slice(), node:JSON.parse(JSON.stringify(it)), ts:Date.now() });
    KV.set('trash', t.slice(0, 50));
  }
  return rmOrig(path, name);
};
const trashCount = () => KV.get('trash', []).length;

APPS.trash = {
  name:'Корзина', glyph:'🗑️', bg:'linear-gradient(140deg,#94a3b8,#475569)', w:600, h:460, single:true,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const bar = el('div', 'toolbar');
    const list = el('div', 'scroll pad');
    wrap.append(bar, list);
    const empty = el('button', 'btn', '🧹 Очистить корзину');
    const restoreAll = el('button', 'btn', '↩ Восстановить всё');
    bar.append(restoreAll, empty, el('div', 'grow'));

    const restore = it => {
      const d = FS.node(it.path);
      if (!d){ Shell.toast('Корзина', 'Исходная папка не найдена', '⚠️'); return false; }
      d.children[it.name] = it.node; FS.save(); return true;
    };
    const draw = () => {
      const t = KV.get('trash', []);
      win.setSub(t.length ? t.length + ' эл.' : 'пусто');
      list.innerHTML = '';
      if (!t.length){ list.appendChild(el('div', 'empty', 'Корзина пуста')); Shell.renderIcons(); return; }
      t.forEach((it, i) => {
        const r = el('div', 'tr-row');
        r.innerHTML = `<div class="gl">${it.node.type === 'dir' ? '📁' : '📄'}</div>
          <div class="nm">${esc(it.name)}<div class="from">из /${esc(it.path.join('/'))} · ${new Date(it.ts).toLocaleString('ru-RU')}</div></div>`;
        const rb = el('button', 'btn', '↩ Восстановить');
        const db = el('button', 'btn', '✖');
        rb.onclick = () => { if (restore(it)){ t.splice(i, 1); KV.set('trash', t); draw(); Shell.renderIcons(); Shell.toast('Восстановлено', it.name, '↩'); } };
        db.onclick = () => { t.splice(i, 1); KV.set('trash', t); draw(); };
        r.append(rb, db);
        list.appendChild(r);
      });
      Shell.renderIcons();
    };
    empty.onclick = async () => {
      if (!await Dlg.confirm('Очистить корзину', 'Все элементы будут удалены безвозвратно.',
          { icon:'🧹', okText:'Очистить', danger:true })) return;
      KV.set('trash', []); draw();
    };
    restoreAll.onclick = () => { const t = KV.get('trash', []); t.forEach(restore); KV.set('trash', []); draw(); Shell.renderIcons(); };
    draw();
  }
};

const dockTrash = $('#dock-trash');
dockTrash.onclick = () => WM.open('trash');
dockTrash.oncontextmenu = e => { e.preventDefault();
  Shell.ctx(e.clientX, e.clientY, [
    { i:'📂', t:'Открыть корзину', f:() => WM.open('trash') },
    { i:'🧹', t:'Очистить корзину', f:() => { KV.set('trash', []); Shell.renderIcons(); Shell.toast('Корзина', 'Очищена', '🗑️'); } }]); };

/* ==========================================================================
   3. Значки рабочего стола: свободное размещение, рамка выделения, клавиши
   ========================================================================== */
const ICONW = 100, ICONH = 106, TOPPAD = 88, LEFTPAD = 14;

Shell.renderIcons = function(){
  const box = $('#desktop-icons');
  box.innerHTML = '';
  const pos = KV.get('iconPos', {});
  const rows = Math.max(1, Math.floor((innerHeight - TOPPAD - 130) / ICONH));

  const items = [
    { g:'💻', n:'Этот компьютер', open:() => WM.open('files', { path:[] }) },
    { g: trashCount() ? '🗑️' : '🗑', n:'Корзина', open:() => WM.open('trash') }
  ];
  const desk = FS.node(['Рабочий стол']);
  Object.values(desk ? desk.children : {}).forEach(f => items.push({
    g: f.type === 'dir' ? '📁' : f.img ? '🖼️' : '📄', n:f.name, file:f,
    open: () => f.type === 'dir' ? WM.open('files', { path:['Рабочий стол', f.name] })
        : f.img ? WM.open('photos', { img:f.img, name:f.name })
        : WM.open('notepad', { file:{ name:f.name, path:['Рабочий стол'], body:f.body } })
  }));

  items.forEach((it, i) => {
    const n = el('div', 'di', `<div class="glyph">${it.g}</div><div class="lbl">${esc(it.n)}</div>`);
    n.tabIndex = 0;
    n.dataset.name = it.n;
    const p = pos[it.n] || { x:LEFTPAD + Math.floor(i / rows) * ICONW, y:TOPPAD + (i % rows) * ICONH };
    n.style.left = clamp(p.x, 0, innerWidth - 100) + 'px';
    n.style.top = clamp(p.y, 60, innerHeight - 150) + 'px';
    n._it = it;

    n.addEventListener('mousedown', e => {
      e.stopPropagation();
      if (!e.ctrlKey && !e.metaKey && !n.classList.contains('sel'))
        $$('.di', box).forEach(x => x.classList.remove('sel'));
      n.classList.add('sel'); n.focus();

      const sx = e.clientX, sy = e.clientY;
      const start = $$('.di.sel', box).map(d => ({ d, x:parseFloat(d.style.left), y:parseFloat(d.style.top) }));
      let moved = false;
      const mv = ev => {
        const dx = ev.clientX - sx, dy = ev.clientY - sy;
        if (!moved && Math.hypot(dx, dy) < 5) return;
        moved = true;
        start.forEach(s => {
          s.d.classList.add('drag');
          s.d.style.left = clamp(s.x + dx, 0, innerWidth - 100) + 'px';
          s.d.style.top = clamp(s.y + dy, 60, innerHeight - 150) + 'px';
        });
      };
      const up = () => {
        document.removeEventListener('mousemove', mv); document.removeEventListener('mouseup', up);
        if (moved){
          const P = KV.get('iconPos', {});
          start.forEach(s => { s.d.classList.remove('drag');
            P[s.d.dataset.name] = { x:parseFloat(s.d.style.left), y:parseFloat(s.d.style.top) }; });
          KV.set('iconPos', P);
        }
      };
      document.addEventListener('mousemove', mv); document.addEventListener('mouseup', up);
    });

    n.ondblclick = it.open;
    n.onkeydown = e => {
      if (e.key === 'Enter') it.open();
      else if (e.key === 'F2' && it.file) renameIcon(it);
      else if (e.key === 'Delete' && it.file) delIcon(it);
    };
    n.oncontextmenu = e => {
      e.preventDefault(); e.stopPropagation();
      $$('.di', box).forEach(x => x.classList.remove('sel')); n.classList.add('sel');
      Shell.ctx(e.clientX, e.clientY, [
        { i:'📂', t:'Открыть', f:it.open, k:'Enter' },
        ...(it.n === 'Корзина' ? [{ i:'🧹', t:'Очистить корзину', f:() => { KV.set('trash', []); Shell.renderIcons(); Shell.toast('Корзина', 'Очищена', '🗑️'); } }] : []),
        ...(it.file ? [
          { i:'✏️', t:'Переименовать', f:() => renameIcon(it), k:'F2' },
          { i:'🗑️', t:'Удалить', f:() => delIcon(it), k:'Del' }] : []),
        'hr',
        { i:'⬛', t:'Выровнять значки', f:() => { KV.set('iconPos', {}); Shell.renderIcons(); } }
      ]);
    };
    box.appendChild(n);
  });

  async function renameIcon(it){
    const nn = await Dlg.prompt('Переименовать', 'Новое имя', it.n, '✏️');
    if (!nn || nn === it.n) return;
    FS.rename(['Рабочий стол'], it.n, nn);
    const P = KV.get('iconPos', {});
    if (P[it.n]){ P[nn] = P[it.n]; delete P[it.n]; KV.set('iconPos', P); }
    Shell.renderIcons();
  }
  function delIcon(it){
    FS.rm(['Рабочий стол'], it.n);
    Shell.toast('Удалено', it.n + ' перемещён в корзину', '🗑️');
    Shell.renderIcons();
  }
};

/* рамка выделения на пустом месте стола */
$('#desktop').addEventListener('mousedown', e => {
  if (e.button !== 0) return;
  if (e.target.closest('.win, .dock, .start, .di, #cc, #widgets, .topbar, .ctx, .desk-widgets')) return;
  $$('.di').forEach(x => x.classList.remove('sel'));
  const sx = e.clientX, sy = e.clientY;
  const m = el('div', 'marquee');
  let live = false;
  const mv = ev => {
    const x = Math.min(sx, ev.clientX), y = Math.min(sy, ev.clientY);
    const w = Math.abs(ev.clientX - sx), h = Math.abs(ev.clientY - sy);
    if (!live && w + h < 8) return;
    if (!live){ live = true; $('#desktop-icons').appendChild(m); }
    Object.assign(m.style, { left:x + 'px', top:y + 'px', width:w + 'px', height:h + 'px' });
    $$('.di').forEach(d => {
      const r = d.getBoundingClientRect();
      d.classList.toggle('sel', !(r.right < x || r.left > x + w || r.bottom < y || r.top > y + h));
    });
  };
  const up = () => { document.removeEventListener('mousemove', mv); document.removeEventListener('mouseup', up); m.remove(); };
  document.addEventListener('mousemove', mv); document.addEventListener('mouseup', up);
});

/* ==========================================================================
   4. Превью открытых окон в доке
   ========================================================================== */
const prev = el('div', 'dock-prev glass lg');
$('#desktop').appendChild(prev);
let prevTimer = null;

function showPrev(btn, appId){
  const wins = WM.wins.filter(w => w.appId === appId);
  if (!wins.length){ hidePrev(); return; }
  prev.innerHTML = `<div class="dp-head">${esc(APPS[appId].name)}</div>`;
  wins.forEach(w => {
    const r = el('div', 'dp-row');
    r.innerHTML = `<div class="dp-thumb" style="background:${w.app.bg}">${w.app.glyph}</div>
      <div class="nm">${esc(w.titleEl.textContent)}</div><button class="x">×</button>`;
    r.onclick = e => {
      if (e.target.classList.contains('x')){ WM.close(w); setTimeout(() => showPrev(btn, appId), 260); return; }
      if (w.minimized) WM.restore(w); else WM.focus(w);
      hidePrev();
    };
    prev.appendChild(r);
  });
  prev.classList.add('on');
  const b = btn.getBoundingClientRect(), p = prev.getBoundingClientRect();
  prev.style.left = clamp(b.left + b.width / 2 - p.width / 2, 10, innerWidth - p.width - 10) + 'px';
  prev.style.top = (b.top - p.height - 12) + 'px';
}
function hidePrev(){ prev.classList.remove('on'); }
prev.addEventListener('mouseenter', () => clearTimeout(prevTimer));
prev.addEventListener('mouseleave', () => { prevTimer = setTimeout(hidePrev, 220); });
$('#dock').addEventListener('mouseover', e => {
  const b = e.target.closest('.dock-item');
  clearTimeout(prevTimer);
  if (!b || !b.dataset.app || !WM.wins.some(w => w.appId === b.dataset.app)){ prevTimer = setTimeout(hidePrev, 200); return; }
  prevTimer = setTimeout(() => showPrev(b, b.dataset.app), 320);
});
$('#dock').addEventListener('mouseleave', () => { prevTimer = setTimeout(hidePrev, 250); });

/* ==========================================================================
   5. Snap Assist — подбор окна во вторую половину
   ========================================================================== */
const assist = el('div', 'snap-assist');
$('#desktop').appendChild(assist);
const snapOrig = WM.snap.bind(WM);
WM.snap = function(win, zone){
  snapOrig(win, zone);
  if (!S.snapAssist || !['left','right'].includes(zone)) return;
  const other = zone === 'left' ? 'right' : 'left';
  const cands = WM.wins.filter(w => w !== win && !w.minimized && w.desk === WM.desk);
  if (!cands.length) return;
  const r = WM.zoneRect(other);
  Object.assign(assist.style, { left:r.left + 'px', top:r.top + 'px', width:r.width + 'px', height:r.height + 'px' });
  assist.innerHTML = '';
  cands.forEach((w, i) => {
    const c = el('div', 'sa-card');
    c.style.setProperty('--i', i);
    c.innerHTML = `<div class="sa-head"><span>${w.app.glyph}</span><span>${esc(w.titleEl.textContent)}</span></div>
      <div class="sa-body" style="background:${w.app.bg}">${w.app.glyph}</div>`;
    c.onclick = () => { close(); snapOrig(w, other); WM.focus(w); };
    assist.appendChild(c);
  });
  assist.appendChild(el('div', 'sa-hint', 'Выберите окно для второй половины · Esc — отмена'));
  assist.classList.add('on');
  const close = () => { assist.classList.remove('on'); document.removeEventListener('mousedown', out, true); };
  const out = e => { if (!e.target.closest('.snap-assist')) close(); };
  setTimeout(() => document.addEventListener('mousedown', out, true), 40);
  addEventListener('keydown', function esc(e){ if (e.key === 'Escape'){ close(); removeEventListener('keydown', esc); } });
};

/* ==========================================================================
   6. Диалог «Выполнить» (Win+R) и системные горячие клавиши
   ========================================================================== */
const RUN = { notepad:'notepad', блокнот:'notepad', calc:'calc', калькулятор:'calc', cmd:'term', powershell:'term',
  терминал:'term', explorer:'files', проводник:'files', control:'settings', параметры:'settings', settings:'settings',
  mspaint:'paint', paint:'paint', msedge:'browser', browser:'browser', браузер:'browser', taskmgr:'taskmgr',
  диспетчер:'taskmgr', music:'music', музыка:'music', calendar:'calendar', календарь:'calendar', clock:'clock',
  часы:'clock', photos:'photos', фото:'photos', store:'store', магазин:'store', todo:'todo', задачи:'todo',
  trash:'trash', корзина:'trash' };

const runDlg = el('div', 'run-dlg glass lg');
runDlg.innerHTML = `<h4>Выполнить</h4>
  <p>Введите имя программы, и Windows откроет её: notepad, calc, cmd, explorer, control, mspaint, taskmgr…</p>
  <div class="run-row"><input class="inp" id="run-in" placeholder="Открыть:" autocomplete="off">
  <button class="btn pri" id="run-ok">ОК</button><button class="btn" id="run-cancel">Отмена</button></div>
  <div class="run-err" id="run-err"></div>`;
$('#desktop').appendChild(runDlg);
const runShow = on => {
  runDlg.classList.toggle('on', on);
  $('#run-err').textContent = '';
  if (on){ $('#run-in').value = ''; setTimeout(() => $('#run-in').focus(), 120); }
};
const runGo = () => {
  const v = $('#run-in').value.trim().toLowerCase().replace(/\.exe$/, '');
  const id = RUN[v] || (APPS[v] ? v : null);
  if (!id){ $('#run-err').textContent = `Не удаётся найти «${v}». Проверьте правильность имени.`; return; }
  runShow(false); WM.open(id);
};
$('#run-ok').onclick = runGo;
$('#run-cancel').onclick = () => runShow(false);
$('#run-in').onkeydown = e => { if (e.key === 'Enter') runGo(); if (e.key === 'Escape') runShow(false); };

addEventListener('keydown', e => {
  const meta = e.metaKey || (e.ctrlKey && e.altKey);
  const typing = /INPUT|TEXTAREA/.test(document.activeElement.tagName);
  if (meta && e.key.toLowerCase() === 'r' && !typing){ e.preventDefault(); Shell.closePanels(); runShow(true); }
  else if (meta && e.key.toLowerCase() === 'e' && !typing){ e.preventDefault(); WM.open('files'); }
  else if (e.ctrlKey && e.shiftKey && e.key === 'Escape'){ e.preventDefault(); WM.open('taskmgr'); }
  else if (e.altKey && e.key === 'F4'){ e.preventDefault(); const t = WM.top(); if (t) WM.close(t); }
  else if (e.key === 'F5' && !typing && !e.ctrlKey){ e.preventDefault(); Shell.renderShell(); }
  else if (e.key === 'Escape'){ runShow(false); }
  else if (e.key === 'F2' || e.key === 'Delete'){
    const s = $('.di.sel');
    if (s && !typing && s._it && s._it.file){ e.preventDefault(); s.dispatchEvent(new KeyboardEvent('keydown', { key:e.key })); }
  }
});

/* ==========================================================================
   7. Системное меню окна (ПКМ по заголовку) + Aero Shake
   ========================================================================== */
$('#windows').addEventListener('contextmenu', e => {
  const tb = e.target.closest('.titlebar');
  if (!tb) return;
  e.preventDefault();
  const node = tb.closest('.win');
  const w = WM.wins.find(x => x.node === node);
  if (!w) return;
  Shell.ctx(e.clientX, e.clientY, [
    { i:'🗕', t:'Свернуть', f:() => WM.minimize(w) },
    { i:'🗖', t:w.maximized ? 'Восстановить' : 'Развернуть', f:() => WM.toggleMax(w) },
    'hr',
    { i:'◧', t:'Прилепить влево', f:() => WM.snap(w, 'left'), k:'Win+←' },
    { i:'◨', t:'Прилепить вправо', f:() => WM.snap(w, 'right'), k:'Win+→' },
    { i:'▦', t:'Разложить все окна', f:() => WM.tile() },
    'hr',
    { i:'🗙', t:'Закрыть', f:() => WM.close(w), k:'Alt+F4' }
  ]);
});

let shake = { last:0, dir:0, n:0, t:0 };
addEventListener('mousemove', e => {
  if (!document.querySelector('.win.dragging')) { shake.n = 0; return; }
  const d = Math.sign(e.clientX - shake.last);
  shake.last = e.clientX;
  if (d && d !== shake.dir){
    shake.dir = d;
    const now = Date.now();
    if (now - shake.t > 700) shake.n = 0;
    shake.t = now;
    if (++shake.n >= 6){
      shake.n = 0;
      const keep = WM.wins.find(w => w.node.classList.contains('dragging'));
      const others = WM.wins.filter(w => w !== keep && !w.minimized && w.desk === WM.desk);
      if (others.length){ others.forEach(w => WM.minimize(w)); Shell.toast('Aero Shake', 'Остальные окна свёрнуты', '🫨'); }
    }
  }
});

/* ==========================================================================
   8. Настоящие батарея и сеть
   ========================================================================== */
if (navigator.getBattery){
  navigator.getBattery().then(bat => {
    const upd = () => {
      const pct = Math.round(bat.level * 100);
      const b = $('#tray-bat');
      if (b){
        b.dataset.tip = `Батарея ${pct}%${bat.charging ? ' · заряжается' : ''}`;
        const f = $('.fill', b);
        if (f){ f.setAttribute('width', Math.max(1, pct / 10)); f.style.fill = pct < 20 && !bat.charging ? '#ff5f57' : ''; }
      }
    };
    upd();
    ['levelchange','chargingchange'].forEach(ev => bat.addEventListener(ev, upd));
  }).catch(() => {});
}
const netState = () => {
  const on = navigator.onLine;
  if (!on && S.wifi) Shell.toast('Сеть', 'Нет подключения к интернету', '📴');
  const b = $('#tray-cc');
  if (b) b.dataset.tip = on ? 'Центр управления · сеть в порядке' : 'Центр управления · нет сети';
};
addEventListener('online', netState);
addEventListener('offline', netState);

/* ==========================================================================
   Перерисовка после загрузки
   ========================================================================== */
Shell.renderIcons();
Shell.updateChrome();
let rl = null;
addEventListener('resize', () => { clearTimeout(rl); rl = setTimeout(() => Shell.renderIcons(), 250); });

})();
