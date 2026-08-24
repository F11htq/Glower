/* ==========================================================================
   Магазин с настоящей установкой приложений
   Каталог, установка/удаление, свои приложения из .json-пакета
   ========================================================================== */
'use strict';

/* ==========================================================================
   Каталог: полноценные приложения, которых нет в системе по умолчанию
   ========================================================================== */
const CATALOG = [

/* ---------- Помидор ---------- */
{ id:'pomodoro', name:'Помидор', glyph:'🍅', bg:'linear-gradient(140deg,#fb7185,#dc2626)',
  w:380, h:480, author:Brand.name, desc:'Таймер концентрации 25/5 с циклами и звонком',
  render(win){
    const wrap = el('div', 'app col'); wrap.style.cssText = 'align-items:center;justify-content:center;gap:18px;padding:24px';
    win.body.appendChild(wrap);
    let mode = 'work', left = 25 * 60, run = false, iv = null, done = KV.get('pomo.done', 0);
    const LEN = { work:25 * 60, brk:5 * 60, long:15 * 60 };
    const ring = el('div', 'pomo-ring', '<span></span>');
    const label = el('div', 'tiny muted');
    const btns = el('div', 'row');
    const start = el('button', 'btn pri', '▶ Старт');
    const reset = el('button', 'btn', '↻');
    const stat = el('div', 'tiny muted');
    btns.append(start, reset);
    wrap.append(ring, label, btns, stat);

    const paint = () => {
      $('span', ring).textContent = `${Math.floor(left / 60)}:${pad2(left % 60)}`;
      ring.style.setProperty('--p', (1 - left / LEN[mode]) * 100 + '%');
      label.textContent = mode === 'work' ? 'Работа' : mode === 'brk' ? 'Короткий перерыв' : 'Длинный перерыв';
      stat.textContent = `Помидоров сегодня: ${done}`;
      win.setSub(label.textContent + ' · ' + $('span', ring).textContent);
    };
    const next = () => {
      if (mode === 'work'){ done++; KV.set('pomo.done', done);
        mode = done % 4 === 0 ? 'long' : 'brk';
        Shell.toast('Помидор', 'Время перерыва', '🍅');
      } else { mode = 'work'; Shell.toast('Помидор', 'За работу', '🍅'); }
      left = LEN[mode]; Snd.note(); paint();
    };
    start.onclick = () => {
      run = !run; start.textContent = run ? '⏸ Пауза' : '▶ Старт';
      clearInterval(iv);
      if (run) iv = setInterval(() => { if (--left <= 0) next(); paint(); }, 1000);
    };
    reset.onclick = () => { left = LEN[mode]; paint(); };
    win.onClose = () => clearInterval(iv);
    paint();
  }
},

/* ---------- 2048 ---------- */
{ id:'g2048', name:'2048', glyph:'🎲', bg:'linear-gradient(140deg,#fbbf24,#f97316)',
  w:460, h:560, author:Brand.name, desc:'Классическая головоломка со счётом и рекордом',
  render(win){
    const wrap = el('div', 'app col'); wrap.style.cssText = 'padding:16px;gap:12px';
    win.body.appendChild(wrap);
    const head = el('div', 'row');
    const score = el('div', 'g-score', 'Счёт<b>0</b>');
    const best = el('div', 'g-score', 'Рекорд<b>' + KV.get('g2048.best', 0) + '</b>');
    const again = el('button', 'btn', 'Заново');
    head.append(score, best, el('div', 'grow'), again);
    const board = el('div', 'g-board');
    const hint = el('div', 'tiny muted', 'Стрелки или WASD · свайп на сенсорном экране');
    wrap.append(head, board, hint);

    let g, sc;
    const rnd = () => {
      const free = [];
      g.forEach((r, y) => r.forEach((v, x) => { if (!v) free.push([x, y]); }));
      if (!free.length) return;
      const [x, y] = free[Math.floor(Math.random() * free.length)];
      g[y][x] = Math.random() < .9 ? 2 : 4;
    };
    const start = () => { g = [0,1,2,3].map(() => [0,0,0,0]); sc = 0; rnd(); rnd(); paint(); };
    const paint = () => {
      board.innerHTML = '';
      g.forEach(r => r.forEach(v => {
        const c = el('div', 'g-cell' + (v ? ' v' + Math.min(v, 2048) : ''), v || '');
        board.appendChild(c);
      }));
      $('b', score).textContent = sc;
      const b = Math.max(sc, KV.get('g2048.best', 0));
      KV.set('g2048.best', b); $('b', best).textContent = b;
    };
    const slide = row => {
      const a = row.filter(Boolean);
      for (let i = 0; i < a.length - 1; i++)
        if (a[i] === a[i + 1]){ a[i] *= 2; sc += a[i]; a.splice(i + 1, 1); }
      while (a.length < 4) a.push(0);
      return a;
    };
    const move = dir => {
      const before = JSON.stringify(g);
      const rot = n => { for (let k = 0; k < n; k++) g = g[0].map((_, i) => g.map(r => r[i]).reverse()); };
      const back = { left:0, up:3, right:2, down:1 }[dir];
      rot({ left:0, up:1, right:2, down:3 }[dir]);
      g = g.map(slide);
      rot(back);
      if (JSON.stringify(g) !== before){ rnd(); paint(); check(); }
    };
    const check = () => {
      const full = g.every(r => r.every(Boolean));
      const stuck = full && g.every((r, y) => r.every((v, x) =>
        (x < 3 && v === g[y][x + 1]) === false && (y < 3 && v === g[y + 1][x]) === false));
      if (stuck) setTimeout(() => Shell.toast('2048', 'Игра окончена · счёт ' + sc, '🎲'), 200);
    };
    again.onclick = start;
    win.node.tabIndex = 0;
    win.node.addEventListener('keydown', e => {
      const m = { ArrowLeft:'left', ArrowRight:'right', ArrowUp:'up', ArrowDown:'down',
                  a:'left', d:'right', w:'up', s:'down', ф:'left', в:'right', ц:'up', ы:'down' };
      const d = m[e.key] || m[e.key.toLowerCase()];
      if (d){ e.preventDefault(); move(d); }
    });
    let tx = 0, ty = 0;
    board.addEventListener('touchstart', e => { tx = e.touches[0].clientX; ty = e.touches[0].clientY; }, { passive:true });
    board.addEventListener('touchend', e => {
      const dx = e.changedTouches[0].clientX - tx, dy = e.changedTouches[0].clientY - ty;
      if (Math.max(Math.abs(dx), Math.abs(dy)) < 24) return;
      move(Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? 'right' : 'left') : (dy > 0 ? 'down' : 'up'));
    });
    setTimeout(() => win.node.focus(), 100);
    start();
  }
},

/* ---------- Конвертер величин ---------- */
{ id:'convert', name:'Конвертер', glyph:'📐', bg:'linear-gradient(140deg,#67e8f9,#0891b2)',
  w:520, h:480, author:Brand.name, desc:'Длина, масса, температура, данные и время',
  render(win){
    const U = {
      'Длина':{ м:1, км:1000, см:0.01, мм:0.001, миля:1609.34, фут:0.3048, дюйм:0.0254, ярд:0.9144 },
      'Масса':{ кг:1, г:0.001, т:1000, фунт:0.453592, унция:0.0283495 },
      'Данные':{ 'КБ':1, 'МБ':1024, 'ГБ':1048576, 'ТБ':1073741824, 'байт':1 / 1024 },
      'Время':{ с:1, мин:60, ч:3600, сут:86400, нед:604800 },
      'Скорость':{ 'км/ч':1, 'м/с':3.6, 'миль/ч':1.60934, узел:1.852 }
    };
    const wrap = el('div', 'app col'); wrap.style.cssText = 'padding:18px;gap:14px';
    win.body.appendChild(wrap);
    let cat = 'Длина';
    const tabs = el('div', 'row'); tabs.style.flexWrap = 'wrap';
    const body = el('div', 'col'); body.style.gap = '10px';
    wrap.append(tabs, body);

    Object.keys(U).concat('Температура').forEach(k => {
      const b = el('button', 'btn' + (k === cat ? ' pri' : ''), k);
      b.onclick = () => { cat = k; $$('.btn', tabs).forEach(x => x.classList.remove('pri')); b.classList.add('pri'); draw(); };
      tabs.appendChild(b);
    });

    function draw(){
      body.innerHTML = '';
      const inp = el('input', 'inp'); inp.type = 'number'; inp.value = 1; inp.style.width = '100%';
      body.appendChild(inp);
      const out = el('div', 'col'); out.style.gap = '6px';
      body.appendChild(out);
      const calc = () => {
        const v = parseFloat(inp.value) || 0;
        out.innerHTML = '';
        if (cat === 'Температура'){
          [['°C', v], ['°F', v * 9 / 5 + 32], ['K', v + 273.15]].forEach(([n, r]) =>
            out.appendChild(el('div', 'conv-row', `<span>${n}</span><b>${(+r.toFixed(4))}</b>`)));
          return;
        }
        const units = Object.entries(U[cat]);
        const base = units[0][1];
        units.forEach(([n, k]) =>
          out.appendChild(el('div', 'conv-row', `<span>${n}</span><b>${+(v * base / k).toFixed(6)}</b>`)));
      };
      inp.oninput = calc; calc();
      const first = cat === 'Температура' ? '°C' : Object.keys(U[cat])[0];
      win.setSub(cat + ' · из ' + first);
    }
    draw();
  }
},

/* ---------- Палитра ---------- */
{ id:'palette', name:'Палитра', glyph:'🎨', bg:'linear-gradient(140deg,#c084fc,#7c3aed)',
  w:560, h:440, author:Brand.name, desc:'Генератор цветовых схем, клик — копирует HEX',
  render(win){
    const wrap = el('div', 'app col'); wrap.style.cssText = 'padding:0;gap:0';
    win.body.appendChild(wrap);
    const bar = el('div', 'toolbar');
    const grid = el('div', 'pal-grid');
    wrap.append(bar, grid);
    const gen = el('button', 'btn pri', '🎲 Сгенерировать');
    const acc = el('button', 'btn', '🎯 В акцент системы');
    bar.append(gen, acc, el('div', 'grow'), el('span', 'tiny muted', 'Клик по цвету — копировать'));
    let colors = [];
    const make = () => {
      const h = Math.random() * 360, s = 55 + Math.random() * 30;
      colors = [0, 30, 60, 180, 210].map((d, i) =>
        `hsl(${(h + d) % 360} ${s}% ${28 + i * 12}%)`);
      draw();
    };
    const hex = c => {
      const d = document.createElement('div'); d.style.color = c; document.body.appendChild(d);
      const m = getComputedStyle(d).color.match(/\d+/g); d.remove();
      return '#' + m.slice(0, 3).map(x => (+x).toString(16).padStart(2, '0')).join('');
    };
    const draw = () => {
      grid.innerHTML = '';
      colors.forEach(c => {
        const n = el('div', 'pal'); n.style.background = c;
        const h = hex(c);
        n.appendChild(el('span', '', h));
        n.onclick = () => {
          if (navigator.clipboard) navigator.clipboard.writeText(h).catch(() => {});
          Shell.toast('Палитра', h + ' скопирован', '🎨', 1800);
        };
        grid.appendChild(n);
      });
    };
    gen.onclick = make;
    acc.onclick = () => { S.accentCustom = hex(colors[3]); Store.save(); applySettings();
      Shell.toast('Палитра', 'Акцент системы обновлён', '🎯'); };
    make();
  }
}
];

/* ==========================================================================
   Установка и удаление
   ========================================================================== */
const AppStore = {
  installedKey:'store.installed',
  customKey:'store.custom',

  installedIds(){ return KV.get(this.installedKey, []); },
  custom(){ return KV.get(this.customKey, []); },

  /* регистрация всего установленного при запуске */
  boot(){
    this.installedIds().forEach(id => {
      const a = CATALOG.find(x => x.id === id);
      if (a) APPS[id] = a;
    });
    this.custom().forEach(p => this.register(p));
  },

  /* пользовательское приложение из пакета */
  register(pkg){
    try {
      const fn = new Function('win', 'api', pkg.code);
      APPS[pkg.id] = {
        name:pkg.name, glyph:pkg.glyph || '📦',
        bg:pkg.bg || 'linear-gradient(140deg,#94a3b8,#475569)',
        w:pkg.w || 640, h:pkg.h || 460, custom:true, pkg,
        render(win){
          try { fn(win, { el, $, $$, KV, FS, Shell, WM, S, esc, pad2, clamp }); }
          catch(e){ win.body.innerHTML = `<div class="pad">Ошибка приложения:<br><b>${esc(e.message)}</b></div>`; }
        }
      };
      return true;
    } catch(e){
      Shell.toast('Магазин', 'Пакет содержит ошибку: ' + e.message, '⚠️');
      return false;
    }
  },

  install(id){
    const a = CATALOG.find(x => x.id === id); if (!a) return false;
    const list = this.installedIds();
    if (!list.includes(id)){ list.push(id); KV.set(this.installedKey, list); }
    APPS[id] = a;
    if (!S.pinned.includes(id)){ S.pinned.push(id); Store.save(); }
    Shell.renderStart();
    Shell.toast('Магазин', a.name + ' установлено', a.glyph);
    return true;
  },

  installPkg(pkg){
    if (!pkg || !pkg.id || !pkg.name || !pkg.code) { Shell.toast('Магазин', 'Это не пакет приложения', '⚠️'); return false; }
    if (APPS[pkg.id] && !this.custom().some(p => p.id === pkg.id)){
      Shell.toast('Магазин', 'Идентификатор занят системным приложением', '⚠️'); return false;
    }
    if (!this.register(pkg)) return false;
    const list = this.custom().filter(p => p.id !== pkg.id);
    list.push(pkg); KV.set(this.customKey, list);
    if (!S.pinned.includes(pkg.id)){ S.pinned.push(pkg.id); Store.save(); }
    Shell.renderStart();
    Shell.toast('Магазин', pkg.name + ' установлено', pkg.glyph || '📦');
    return true;
  },

  uninstall(id){
    const open = WM.wins.filter(w => w.appId === id);
    open.forEach(w => WM.close(w));
    // окно закрывается с анимацией — снимаем регистрацию после неё
    setTimeout(() => { delete APPS[id]; Shell.renderDock(); }, open.length ? 320 : 0);
    delete APPS[id];
    KV.set(this.installedKey, this.installedIds().filter(x => x !== id));
    KV.set(this.customKey, this.custom().filter(p => p.id !== id));
    S.pinned = S.pinned.filter(x => x !== id);
    S.dockApps = S.dockApps.filter(x => x !== id);
    Store.save();
    Shell.renderStart(); Shell.renderDock();
    Shell.toast('Магазин', 'Приложение удалено', '🗑️');
  },

  isInstalled(id){ return !!APPS[id]; },

  /* шаблон пакета для своих приложений */
  template(){
    return {
      id:'my-app', name:'Моё приложение', glyph:'✨',
      bg:'linear-gradient(140deg,#f472b6,#8b5cf6)', w:520, h:400,
      code:[
        "// win  — окно: win.body, win.setTitle(), win.setSub(), win.close()",
        "// api  — { el, $, $$, KV, FS, Shell, WM, S }",
        "const box = api.el('div', 'pad');",
        "box.innerHTML = '<h2>Привет!</h2><p class=\"muted\">Это приложение установлено из пакета.</p>';",
        "const b = api.el('button', 'btn pri', 'Нажми меня');",
        "let n = 0;",
        "b.onclick = () => { n++; b.textContent = 'Нажато ' + n; api.Shell.toast('Моё приложение', 'Клик №' + n, '✨'); };",
        "box.appendChild(b);",
        "win.body.appendChild(box);"
      ].join('\n')
    };
  }
};
window.AppStore = AppStore;
AppStore.boot();

/* ==========================================================================
   Витрина магазина
   ========================================================================== */
APPS.store = {
  name:'Магазин', glyph:'🛍️', bg:'linear-gradient(140deg,#c4b5fd,#7c3aed)', w:880, h:640, single:true,
  render(win){
    const wrap = el('div', 'app col'); win.body.appendChild(wrap);
    const bar = el('div', 'toolbar');
    const body = el('div', 'scroll pad');
    wrap.append(bar, body);
    let tab = 'catalog';

    [['catalog','🛍 Каталог'],['mine','📦 Установленные'],
     ['linux','🐧 Программы Linux'],['dev','🧑‍💻 Своё приложение']].forEach(([k, n]) => {
      const b = el('button', 'btn' + (k === tab ? ' pri' : ''), n);
      b.onclick = () => { tab = k; $$('.btn', bar).forEach(x => x.classList.remove('pri')); b.classList.add('pri'); draw(); };
      bar.appendChild(b);
    });

    function cardFor(a, installed, custom){
      const c = el('div', 'st-card');
      const ico = el('div', 'app-ico', a.glyph); ico.style.background = a.bg;
      ico.style.width = ico.style.height = '54px'; ico.style.fontSize = '26px';
      c.appendChild(ico);
      c.appendChild(el('div', '', `<b style="font-size:13px">${esc(a.name)}</b>
        <div class="tiny muted" style="margin-top:3px;line-height:1.35">${esc(a.desc || (custom ? 'Своё приложение' : 'Системное приложение'))}</div>
        <div class="tiny muted" style="margin-top:4px;opacity:.5">${esc(a.author || 'вы')}</div>`));
      const row2 = el('div', 'row');
      if (installed){
        const open = el('button', 'btn pri', 'Открыть');
        open.onclick = () => WM.open(a.id);
        row2.appendChild(open);
        if (a.removable !== false){
          const del = el('button', 'btn', '🗑');
          del.onclick = async () => {
            if (!await Dlg.confirm('Удалить приложение', `«${a.name}» будет удалено из системы.`,
                { icon:'🗑️', okText:'Удалить', danger:true })) return;
            AppStore.uninstall(a.id); draw();
          };
          row2.appendChild(del);
        }
      } else {
        const ins = el('button', 'btn pri', '⬇ Установить');
        ins.onclick = () => { AppStore.install(a.id); draw(); };
        row2.appendChild(ins);
      }
      c.appendChild(row2);
      return c;
    }

    /* ---------- программы машины: настоящие пакеты из репозиториев ---------- */
    const Pkg = {
      state(){ return Platform.rpc('pkg.state'); },
      search(query){ return Platform.rpc('pkg.search', { query }); },
      info(name, source){ return Platform.rpc('pkg.info', { name, source }); },
      installed(){ return Platform.rpc('pkg.installed'); },
      install(name, source){ return Platform.rpc('pkg.install', { name, source }); },
      remove(name, source){ return Platform.rpc('pkg.remove', { name, source }); },
      update(source){ return Platform.rpc('pkg.update', { source }); },
      flathub(){ return Platform.rpc('pkg.flathub'); },
      job(){ return Platform.rpc('pkg.job'); },
      cancel(){ return Platform.rpc('pkg.cancel'); }
    };
    const размер = b => !b ? '' : b > 1048576 ? (b / 1048576).toFixed(1) + ' МБ'
                                              : Math.round(b / 1024) + ' КБ';
    let поискСтрока = '', поискСписок = null, работа = null;

    async function следиЗаРаботой(перерисовать){
      for (let i = 0; i < 900; i++){
        let j;
        try { j = await Pkg.job(); } catch(e){ break; }
        работа = j.running ? j : null;
        перерисовать(j);
        if (!j.running){
          Shell.toast('Программы машины',
            j.ok ? (j.action === 'install' ? 'Установлено: ' + j.name + ' — ищите в Пуске'
                  : j.action === 'remove' ? 'Удалено: ' + j.name : 'Списки обновлены')
                 : 'Не вышло: ' + (j.error || 'неизвестная причина'),
            j.ok ? '✅' : '⚠️', 8000);
          break;
        }
        await new Promise(r => setTimeout(r, 1200));
      }
      работа = null;
      draw();
    }

    async function drawLinux(){
      const st = await Pkg.state().catch(e => ({ reason:String(e.message || e) }));
      body.innerHTML = '';
      body.appendChild(el('div', 'st-hero', `<h2 style="margin:0 0 6px">Программы Linux</h2>
        <div style="opacity:.85">Поиск идёт по двум источникам: репозитории Ubuntu и Flathub —
        там живут Telegram, Firefox, Spotify и прочее, чего в Ubuntu уже нет. Поставленное
        появляется в «Программах машины» и запускается как обычная программа.${
          st && st.flatpak === false ? ' <b>Flathub на этой машине недоступен</b>.' : ''}</div>`));

      if (!st || st.reason){
        body.appendChild(el('div', 'set-note', esc((st && st.reason) ||
          'Установка программ доступна, только когда система управляет машиной')));
        win.setSub('недоступно');
        return;
      }

      /* Живая система держит всё в памяти: об этом честнее сказать заранее,
         чем показать падение dpkg после получаса скачивания. */
      if (st.live){
        const гб = b => b ? (b / 1073741824).toFixed(1) + ' ГБ' : '?';
        body.appendChild(el('div', 'set-note',
          'Система сейчас работает из памяти: всё поставленное занимает оперативку и исчезнет ' +
          'при выключении. Свободно ' + гб(st.free) + '. Для больших программ сначала установите ' +
          'систему на диск — там места столько же, сколько на диске.'));
      }

      /* Flathub подключается отдельно: у него свои списки, и без них поиск по
         нему честно ничего не находит. Показываем это прямо, а не молчим. */
      if (st.flatpak && !st.flathubData){
        const box = el('div', 'card', '');
        box.style.padding = '14px';
        box.innerHTML = `<b>Flathub ещё не подключён</b>
          <div class="muted tiny" style="margin-top:6px;line-height:1.45">
            Telegram, Spotify, Firefox и прочее живёт там. Списки Flathub качаются
            отдельно — это около 30 МБ и одна-две минуты, зато потом поиск находит всё.</div>`;
        const b3 = el('button', 'btn pri', '🫙 Подключить Flathub');
        b3.style.marginTop = '10px';
        b3.disabled = !!работа;
        b3.onclick = async () => {
          try {
            await Pkg.flathub(); draw();
            const j = await дождисьРаботы();
            if (j.ok){ st.flathubData = true; st.flathub = true; }
            Shell.toast('Программы Linux', j.ok ? 'Flathub подключён' :
              'Не вышло подключить Flathub: ' + (j.error || ''), j.ok ? '✅' : '⚠️', 7000);
            if (j.ok && поискСтрока.length >= 2) найти(); else draw();
          } catch(e){ Dlg.alert('Flathub', String(e.message || e), '⚠️'); }
        };
        box.appendChild(b3);
        body.appendChild(box);
      }

      const bar2 = el('div', 'row');
      const inp = el('input', 'inp grow');
      inp.placeholder = '🔎 Название программы: gimp, vlc, telegram…';
      inp.value = поискСтрока;
      const go = el('button', 'btn pri', 'Найти');
      const upd = el('button', 'btn', '🔄 Обновить списки');
      bar2.append(inp, go, upd);
      body.appendChild(bar2);

      /* Обновление списков идёт минуту-другую. Если поиск запустить поверх него,
         apt ответит по пустым спискам — и человек увидит «ничего не найдено»
         под бегущей полосой обновления. Поэтому ждём. */
      const дождисьРаботы = async () => {
        for (let i = 0; i < 900; i++){
          const j = await Pkg.job().catch(() => ({ running:false }));
          работа = j.running ? j : null;
          if (!j.running) return j;
          draw();
          await new Promise(r => setTimeout(r, 1200));
        }
        return { running:false, ok:false };
      };

      const найти = async () => {
        поискСтрока = inp.value.trim();
        if (поискСтрока.length < 2) return;

        if (работа && работа.action === 'update'){
          поискСписок = 'обновляю';
          draw();
          const j = await дождисьРаботы();
          st.lists = st.lists || !!j.ok;
        }

        /* В свежей системе списки пакетов пусты — их вычищают при сборке
           образа. Обновляем сами: человек не должен догадываться, что перед
           первым поиском надо нажать отдельную кнопку. */
        if (!st.lists && st.allowed && !работа){
          поискСписок = 'обновляю';
          draw();
          try {
            await Pkg.update();
            await new Promise(готово => {
              const жду = setInterval(async () => {
                const j = await Pkg.job().catch(() => ({ running:false }));
                работа = j.running ? j : null;
                if (!j.running){ clearInterval(жду); st.lists = !!j.ok; готово(); }
              }, 1200);
            });
          } catch(e){ поискСписок = { ошибка:String(e.message || e) }; return draw(); }
        }

        поискСписок = 'ищу';
        draw();
        try {
          const r = await Pkg.search(поискСтрока);
          const list = r.list.slice(0, 24);
          /* подробности берём только для показанных: иначе это сотни запросов */
          поискСписок = await Promise.all(list.map(async x => {
            try { return Object.assign(x, await Pkg.info(x.name, x.source)); } catch(e){ return x; }
          }));
        } catch(e){ поискСписок = { ошибка:String(e.message || e) }; }
        draw();
      };
      go.onclick = найти;
      inp.onkeydown = e => { if (e.key === 'Enter') найти(); };
      upd.onclick = async () => {
        try {
          await Pkg.update(); draw();
          let j = await дождисьРаботы();
          st.lists = st.lists || !!j.ok;
          /* второй источник обновляем следом: у Flathub свои списки */
          if (st.flatpak){
            try { await Pkg.update('flatpak'); draw(); j = await дождисьРаботы(); } catch(e){}
          }
          Shell.toast('Программы Linux', j.ok ? 'Списки обновлены' : 'Обновить списки не вышло',
            j.ok ? '✅' : '⚠️', 5000);
          if (поискСтрока.length >= 2) найти(); else draw();
        } catch(e){ Dlg.alert('Обновление списков', String(e.message || e), '⚠️'); }
      };

      if (работа){
        const box = el('div', 'card', '');
        box.style.padding = '14px';
        const молчит = работа.молчит || 0;
        box.innerHTML = `<b>${esc(работа.action === 'remove' ? 'Удаление' : работа.action === 'update' ? 'Обновление списков' : 'Установка')}
          ${esc(работа.name || '')}</b>
          <div class="ins-bar" style="margin-top:10px"><i style="width:${работа.percent || 0}%"></i></div>
          <div class="muted tiny" style="margin-top:6px">${esc(работа.step || '')}${
            молчит > 60 ? ' · молчит ' + Math.round(молчит / 60) + ' мин — возможно, ждёт сеть' : ''}</div>`;
        const stop = el('button', 'btn', '✕ Остановить');
        stop.style.marginTop = '10px';
        stop.onclick = async () => {
          if (!await Dlg.confirm('Остановить работу?',
              'apt будет прерван, а система приведена в порядок.', { icon:'✕', okText:'Остановить', danger:true })) return;
          try { await Pkg.cancel(); работа = null; draw(); }
          catch(e){ Dlg.alert('Программы Linux', String(e.message || e), '⚠️'); }
        };
        box.appendChild(stop);
        body.appendChild(box);
      }

      if (поискСписок === 'обновляю'){
        body.appendChild(el('div', 'empty', 'Обновляю списки пакетов — это делается один раз…'));
      } else if (поискСписок === 'ищу'){
        body.appendChild(el('div', 'empty', 'Ищу в репозиториях…'));
      } else if (поискСписок && поискСписок.ошибка){
        body.appendChild(el('div', 'set-note', esc(поискСписок.ошибка)));
      } else if (Array.isArray(поискСписок)){
        body.appendChild(el('div', 'card-t', 'Найдено'));
        if (!поискСписок.length)
          body.appendChild(el('div', 'empty', !st.lists
            ? 'Списки пакетов ещё не загружены — нажмите «Обновить списки»'
            : (st.flatpak && !st.flathubData)
              ? 'В репозиториях Ubuntu такого нет. Telegram, Spotify и подобное живут на Flathub — ' +
                'подключите его кнопкой выше, и поиск найдёт их'
              : 'Ничего не найдено. У программ бывают свои имена — попробуйте другое написание'));
        поискСписок.forEach(x => {
          const b2 = el('button', 'btn' + (x.installed ? '' : ' pri'),
            x.installed ? 'Удалить' : '⬇ Установить');
          b2.disabled = !!работа;
          b2.onclick = async () => {
            try {
              if (x.installed){
                if (!await Dlg.confirm('Удалить ' + x.name + '?',
                    'Программа будет удалена из системы вместе с ненужными зависимостями.',
                    { icon:'🗑️', okText:'Удалить', danger:true })) return;
                await Pkg.remove(x.name, x.source);
              } else await Pkg.install(x.name, x.source);
              следиЗаРаботой(() => {});
              draw();
            } catch(e){ Dlg.alert('Программы машины', String(e.message || e), '⚠️'); }
          };
          if (x.snap){
            b2.disabled = true;
            b2.textContent = 'через Snap';
          }
          const источник = x.source === 'flatpak' ? 'Flathub' : 'Ubuntu';
          const сведения = [источник,
                            x.installed ? 'установлена ' + x.installed : x.candidate || '',
                            размер(x.size) ? (x.source === 'flatpak' ? 'скачает ' : 'займёт ') + размер(x.size) : '',
                            x.snap ? 'это заглушка: ставится через Snap, а он в системе не работает' : '']
                            .filter(Boolean).join(' · ');
          body.appendChild(row(x.source === 'flatpak' ? '🫙' : '📦',
            (x.title && x.title !== x.name ? x.title + ' · ' + x.name : x.name),
            (x.about || '') + (сведения ? ' · ' + сведения : ''), b2));
        });
      } else {
        body.appendChild(el('div', 'set-note',
          'Введите название программы. В самой системе программ нет — они берутся из ' +
          'репозиториев Ubuntu, тех же, что у обычного линукса, поэтому нужен интернет. ' +
          (st.lists ? '' : 'Списки пакетов ещё не загружены — система обновит их при первом поиске.')));
      }
      win.setSub('программы Linux');
    }

    function draw(){
      if (tab === 'linux'){ drawLinux(); return; }
      body.innerHTML = '';
      if (tab === 'catalog'){
        body.appendChild(el('div', 'st-hero', `<h2 style="margin:0 0 6px">Магазин ${Brand.name}</h2>
          <div style="opacity:.85">Приложения устанавливаются по-настоящему: появляются в Пуске,
          запускаются как системные и удаляются вместе с данными</div>`));
        body.appendChild(el('div', 'card-t', 'Доступно к установке'));
        const g = el('div', 'st-grid');
        CATALOG.forEach(a => g.appendChild(cardFor(a, AppStore.isInstalled(a.id))));
        body.appendChild(g);
        win.setSub('каталог · ' + CATALOG.length);
      }
      else if (tab === 'mine'){
        const custom = AppStore.custom();
        const installed = AppStore.installedIds().map(id => CATALOG.find(x => x.id === id)).filter(Boolean);
        body.appendChild(el('div', 'card-t', 'Установлено из каталога'));
        const g1 = el('div', 'st-grid');
        installed.forEach(a => g1.appendChild(cardFor(a, true)));
        if (!installed.length) g1.appendChild(el('div', 'empty', 'Пока ничего не установлено'));
        body.appendChild(g1);

        body.appendChild(el('div', 'card-t', 'Свои приложения'));
        const g2 = el('div', 'st-grid');
        custom.forEach(p => g2.appendChild(cardFor(
          { id:p.id, name:p.name, glyph:p.glyph || '📦', bg:p.bg || 'linear-gradient(140deg,#94a3b8,#475569)',
            desc:'Установлено из пакета', author:'вы' }, true, true)));
        if (!custom.length) g2.appendChild(el('div', 'empty', 'Своих приложений нет'));
        body.appendChild(g2);

        body.appendChild(el('div', 'card-t', 'Системные (удалить нельзя)'));
        const g3 = el('div', 'st-grid');
        Object.entries(APPS).filter(([id, a]) => !a.custom && !CATALOG.some(c => c.id === id))
          .forEach(([id, a]) => g3.appendChild(cardFor({ ...a, id, removable:false }, true)));
        body.appendChild(g3);
        win.setSub('установлено · ' + (installed.length + custom.length));
      }
      else {
        body.appendChild(el('div', 'card-t', 'Своё приложение'));
        const c = card('');
        c.appendChild(row('📄', 'Что такое пакет',
          'Обычный .json-файл: идентификатор, имя, значок, размеры окна и код на JavaScript в поле code. ' +
          'Код получает объект окна и набор функций системы.', el('span')));

        const tpl = el('button', 'btn pri', '✨ Создать заготовку');
        tpl.onclick = () => {
          const p = AppStore.template();
          const name = p.id + '.app.json';
          FS.write(['Документы'], name, JSON.stringify(p, null, 2));
          WM.open('notepad', { file:{ name, path:['Документы'], body:JSON.stringify(p, null, 2) } });
          Shell.toast('Магазин', 'Заготовка в Документы/' + name, '✨');
        };
        c.appendChild(row('🧩', 'Заготовка в Блокноте', 'Создаст рабочий пример и откроет его для правки', tpl));

        const fromFs = el('button', 'btn', '📂 Из файлов');
        fromFs.onclick = () => WM.open('files', { path:['Документы'], pick:f => {
          try { AppStore.installPkg(JSON.parse(f.body)); tab = 'mine'; draw(); WM.focus(win); }
          catch(e){ Shell.toast('Магазин', 'Не разобрать JSON: ' + e.message, '⚠️'); }
        }});
        c.appendChild(row('💾', 'Установить из системы', 'Выберите .json-пакет в Проводнике', fromFs));

        const fromDisk = el('button', 'btn', '⬆️ С компьютера');
        fromDisk.onclick = () => {
          const f = el('input'); f.type = 'file'; f.accept = '.json,application/json';
          f.onchange = () => {
            const r = new FileReader();
            r.onload = () => { try { AppStore.installPkg(JSON.parse(r.result)); tab = 'mine'; draw(); }
              catch(e){ Shell.toast('Магазин', 'Не разобрать JSON: ' + e.message, '⚠️'); } };
            r.readAsText(f.files[0]);
          };
          f.click();
        };
        c.appendChild(row('🖥', 'Установить с диска', 'Настоящий файл с вашего компьютера', fromDisk));
        body.appendChild(c);

        const w = card('Как это работает');
        w.appendChild(row('⚙️', 'Код выполняется в системе',
          'Пакет исполняется как обычное приложение прототипа: у него есть доступ к окну, файловой системе и уведомлениям. ' +
          'Ставьте только те пакеты, содержимое которых вы видели — это ваш код в вашем браузере.', el('span')));
        const ex = el('pre', 'pkg-code', esc(JSON.stringify(AppStore.template(), null, 2)).slice(0, 900));
        w.appendChild(ex);
        body.appendChild(w);
        win.setSub('разработка');
      }
    }
    draw();
  }
};
