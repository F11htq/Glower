/* ==========================================================================
   Оболочка: док, Пуск, центр управления, виджеты, поиск, Task View,
   уведомления, контекстные меню, обращение к системному замку
   ========================================================================== */
'use strict';

const Shell = {
  nowPlaying:null,
  wallTimer:null,

  init(){
    this.renderShell();
    this.bindTray();
    this.bindStart();
    this.bindSpot();
    this.привяжиОбзор();
    this.bindDesktop();
    this.bindKeys();
    this.clock();
    setInterval(() => this.clock(), 1000);
    this.wallShuffle();
    this.autoTheme();
    this.hotCorners();
    this.dockMagnify();
  },

  renderShell(){
    this.renderDock();
    this.renderStart();
    this.renderIcons();
    this.renderDeskWidgets();
    this.renderCC();
    this.renderWidgets();
    this.renderTaskview();
    $('#tbw-city').textContent = S.city;
  },

  /* ================= ДОК ================= */
  renderDock(){
    const box = $('#dock-items');
    box.innerHTML = '';
    S.dockApps.filter(id => APPS[id]).forEach(id => {
      const a = APPS[id];
      const b = el('button', 'dock-item');
      b.dataset.app = id;
      b.dataset.tip = a.name;
      b.appendChild(appIcon(a));
      b.onclick = () => this.launch(id, b);
      b.oncontextmenu = e => {
        e.preventDefault();
        const open = WM.wins.filter(w => w.appId === id);
        this.ctx(e.clientX, e.clientY, [
          { i:a.glyph, t:'Открыть', f:() => this.launch(id) },
          ...(open.length ? [{ i:'✖', t:'Закрыть все окна', f:() => open.forEach(w => WM.close(w)) }] : []),
          'hr',
          { i:'📌', t:'Убрать из дока', f:() => { S.dockApps = S.dockApps.filter(x => x !== id); Store.save(); this.renderDock(); } }
        ]);
      };
      box.appendChild(b);
    });
    this.syncDock();
    this.dockMagnify();
  },

  syncDock(){
    const running = {};
    WM.wins.forEach(w => { running[w.appId] = running[w.appId] || []; running[w.appId].push(w); });
    const active = WM.wins.find(w => w.node.classList.contains('focus') && !w.minimized);

    $$('#dock-items .dock-item').forEach(b => {
      const id = b.dataset.app;
      b.classList.toggle('run', !!running[id]);
      b.classList.toggle('active', !!(active && active.appId === id));
    });

    // запущенные, но не закреплённые
    const box = $('#dock-running');
    const extra = Object.keys(running).filter(id => !S.dockApps.includes(id));
    $('#dock-sep-run').hidden = !extra.length;
    box.innerHTML = '';
    extra.forEach(id => {
      const a = APPS[id];
      if (!a) return;                       // приложение удалено, окна ещё закрываются
      const b = el('button', 'dock-item run' + (active && active.appId === id ? ' active' : ''));
      b.dataset.app = id; b.dataset.tip = a.name;
      b.appendChild(appIcon(a));
      b.onclick = () => this.launch(id, b);
      box.appendChild(b);
    });
    this.dockMagnify();
    this.renderTaskview();
  },

  launch(id, btn){
    const wins = WM.wins.filter(w => w.appId === id && w.desk === WM.desk);
    if (!wins.length){
      if (btn) { btn.classList.add('bounce'); setTimeout(() => btn.classList.remove('bounce'), 700); }
      WM.open(id);
    } else {
      const focused = wins.find(w => w.node.classList.contains('focus') && !w.minimized);
      if (focused && wins.length === 1) WM.minimize(focused);
      else { const t = wins.find(w => w.minimized) || wins[0]; if (t.minimized) WM.restore(t); else WM.focus(t); }
    }
    this.closePanels();
  },

  dockIconRect(appId){
    const b = $(`#dock .dock-item[data-app="${appId}"]`);
    return b ? b.getBoundingClientRect() : null;
  },

  /* увеличение значков как в macOS */
  /* Наведение на значок панели: как в обычных системах — подсветка, и всё.
     Раньше значки разъезжались и подпрыгивали, будто это витрина, а не
     рабочий инструмент; за курсором было тяжело уследить, и попасть по
     нужному значку в движении получалось не всегда. */
  dockMagnify(){ /* оставлено пустым намеренно: увеличения больше нет */ },

  /* ================= ПУСК ================= */
  renderStart(){
    const g = $('#start-pinned');
    g.innerHTML = '';
    S.pinned.filter(id => APPS[id]).forEach((id, i) => {
      const a = APPS[id];
      const b = el('button', 'sg');
      b.style.setProperty('--i', i);
      b.appendChild(appIcon(a));
      b.appendChild(el('div', 'n', a.name));
      b.onclick = () => { WM.open(id); this.closePanels(); };
      b.oncontextmenu = e => { e.preventDefault(); this.ctx(e.clientX, e.clientY, [
        { i:'📌', t:'Открепить', f:() => { S.pinned = S.pinned.filter(x => x !== id); Store.save(); this.renderStart(); } },
        { i:'⬇️', t:'Добавить в док', f:() => { if (!S.dockApps.includes(id)){ S.dockApps.push(id); Store.save(); this.renderDock(); } } },
        { i:'🖥', t:(S.deskApps || []).includes(id) ? 'Убрать с рабочего стола' : 'Вынести на рабочий стол',
          f:() => {
            const было = (S.deskApps || []).includes(id);
            S.deskApps = было ? S.deskApps.filter(x => x !== id) : [...(S.deskApps || []), id];
            Store.save(); this.renderIcons();
            this.toast('Рабочий стол', было ? 'Убрано со стола' : 'Вынесено на стол', '🖥'); } }
      ]); };
      g.appendChild(b);
    });

    const r = $('#start-reco');
    r.innerHTML = '';
    const recent = KV.get('recent', ['notepad','settings','browser','paint']);
    recent.slice(0, 6).forEach(id => {
      if (!APPS[id]) return;
      const a = APPS[id];
      const b = el('button', 'rc');
      b.appendChild(appIcon(a));
      b.appendChild(el('div', 't', `${esc(a.name)}<small>Недавно использовалось</small>`));
      b.onclick = () => { WM.open(id); this.closePanels(); };
      r.appendChild(b);
    });
  },

  bindStart(){
    $('#start-btn').onclick = e => { e.stopPropagation(); this.toggleStart(); };
    $('#start-all').onclick = () => { $('#start-input').value = ''; this.allApps(!this.allMode); };
    $('#start-user').onclick = () => { WM.open('settings', { section:'acc' }); this.closePanels(); };
    $$('#start [data-app]').forEach(b => b.onclick = () => { WM.open(b.dataset.app); this.closePanels(); });
    $('#power-btn').onclick = () => { this.closePanels(); $('#power-overlay').classList.add('on'); };
    $('#start-input').addEventListener('input', e => this.startSearch(e.target.value));
    $('#start-input').addEventListener('keydown', e => {
      if (e.key === 'Escape') this.closePanels();
      if (e.key === 'Enter'){ const f = $('#start-results .sres'); if (f) f.click(); }
    });
    $('#start').addEventListener('click', e => e.stopPropagation());

    $$('#power-overlay [data-act]').forEach(b => b.onclick = () => this.power(b.dataset.act));
    $('#power-overlay').onclick = e => { if (e.target.id === 'power-overlay') $('#power-overlay').classList.remove('on'); };
  },

  toggleStart(v){
    const on = v != null ? v : !document.body.classList.contains('start-open');
    this.closePanels(true);
    document.body.classList.toggle('start-open', on);
    if (on){
      $('#start-input').value = ''; this.allApps(false);
      setTimeout(() => $('#start-input').focus(), 180);
      Snd.blip(880, .06, 'sine', .03);
    }
  },

  /* ---- «Все приложения»: список по алфавиту ---- */
  allApps(on){
    this.allMode = !!on;
    const res = $('#start-results'), body = $('#start-body'), btn = $('#start-all');
    btn.textContent = on ? '‹ Назад' : 'Все приложения ›';
    res.innerHTML = '';
    if (!on){ res.hidden = true; body.hidden = false; return; }

    body.hidden = true; res.hidden = false;

    // собственный заголовок: кнопка в «Закреплённых» скрыта вместе со списком
    const head = el('div', 'all-head');
    const back = el('button', 'mini-btn', '‹ Назад');
    back.onclick = () => this.allApps(false);
    head.append(back, el('span', '', 'Все приложения'));
    res.appendChild(head);

    const items = Object.entries(APPS).sort((a, b) => a[1].name.localeCompare(b[1].name, 'ru'));
    let letter = '';
    items.forEach(([id, a], i) => {
      const L = (a.name[0] || '#').toUpperCase();
      if (L !== letter){ letter = L; res.appendChild(el('div', 'all-letter', L)); }
      const b = el('button', 'all-row');
      b.style.setProperty('--i', i);
      b.appendChild(appIcon(a));
      const t = el('div', 't', esc(a.name));
      if (S.pinned.includes(id)) t.appendChild(el('span', 'all-pin', '📌'));
      b.appendChild(t);
      b.onclick = () => { WM.open(id); this.closePanels(); };
      b.oncontextmenu = e => {
        e.preventDefault();
        const pinned = S.pinned.includes(id), inDock = S.dockApps.includes(id);
        this.ctx(e.clientX, e.clientY, [
          { i:a.glyph, t:'Открыть', f:() => { WM.open(id); this.closePanels(); } },
          'hr',
          { i:'📌', t:pinned ? 'Открепить от Пуска' : 'Закрепить в Пуске', f:() => {
              S.pinned = pinned ? S.pinned.filter(x => x !== id) : [...S.pinned, id];
              Store.save(); this.renderStart(); this.allApps(true); } },
          { i:'⬇️', t:inDock ? 'Убрать из дока' : 'Добавить в док', f:() => {
              S.dockApps = inDock ? S.dockApps.filter(x => x !== id) : [...S.dockApps, id];
              Store.save(); this.renderDock(); } },
          { i:'🖥', t:(S.deskApps || []).includes(id) ? 'Убрать с рабочего стола' : 'Вынести на рабочий стол',
            f:() => {
              const было = (S.deskApps || []).includes(id);
              S.deskApps = было ? S.deskApps.filter(x => x !== id) : [...(S.deskApps || []), id];
              Store.save(); this.renderIcons(); this.allApps(true);
              this.toast('Рабочий стол', было ? 'Убрано со стола' : 'Вынесено на стол', '🖥'); } }
        ]);
      };
      res.appendChild(b);
    });
    res.appendChild(el('div', 'all-count', `Всего приложений: ${items.length}`));
    res.scrollTop = 0;
  },

  startSearch(q){
    const res = $('#start-results'), body = $('#start-body');
    q = q.trim().toLowerCase();
    if (!q){ if (this.allMode) return this.allApps(true); res.hidden = true; body.hidden = false; return; }
    res.hidden = false; body.hidden = true;
    res.innerHTML = '';
    const items = this.searchAll(q);
    if (!items.length){ res.appendChild(el('div', 'empty', 'Ничего не найдено')); return; }
    items.forEach(it => res.appendChild(this.resNode(it)));
  },

  searchAll(q){
    const out = [];
    Object.entries(APPS).forEach(([id, a]) => {
      if (a.name.toLowerCase().includes(q) || id.includes(q))
        out.push({ ico:a, t:a.name, s:'Приложение', k:'Enter', run:() => WM.open(id) });
    });
    const walk = (node, path) => {
      Object.values(node.children || {}).forEach(c => {
        if (c.name.toLowerCase().includes(q))
          out.push({ emo:c.type === 'dir' ? '📁' : '📄', t:c.name, s:'/' + path.join('/'), k:'Файл',
            run:() => c.type === 'dir' ? WM.open('files', { path:[...path, c.name] })
                                       : WM.open('notepad', { file:{ name:c.name, path:path.slice(), body:c.body } }) });
        if (c.type === 'dir') walk(c, [...path, c.name]);
      });
    };
    walk(FS.root, []);
    [['Обои','person'],['Цвет акцента','person'],['Тема','person'],['Форма поверхностей','person'],
     ['Анимации','motion'],['Звук','sound'],['Яркость','display'],['Wi-Fi','net'],['Bluetooth','bt'],
     ['Уведомления','notif'],['Обновления','update'],['О системе','about'],['Док','dock']]
      .forEach(([n, sec]) => { if (n.toLowerCase().includes(q))
        out.push({ emo:'⚙️', t:n, s:'Параметры', k:'Параметр', run:() => WM.open('settings', { section:sec }) }); });

    if (/^[\d\s+\-*/.,()]+$/.test(q) && /[\d]/.test(q)){
      try { const v = Function('"use strict";return (' + q.replace(/,/g, '.') + ')')();
        if (typeof v === 'number' && isFinite(v)) out.unshift({ emo:'🧮', t:String(+v.toFixed(8)), s:q + ' =', k:'Калькулятор', run:() => WM.open('calc') });
      } catch(e){}
    }
    return out.slice(0, 12);
  },

  resNode(it){
    const n = el('div', 'sres');
    if (it.ico) n.appendChild(appIcon(it.ico));
    else { const d = el('div', 'app-ico', it.emo); d.style.background = 'rgba(255,255,255,.16)'; n.appendChild(d); }
    n.appendChild(el('div', 't', `${esc(it.t)}<small>${esc(it.s)}</small>`));
    n.appendChild(el('div', 'k', it.k || ''));
    n.onclick = () => { it.run(); this.closePanels(); };
    return n;
  },

  /* ================= SPOTLIGHT ================= */
  bindSpot(){
    const ov = $('#spot-overlay'), inp = $('#spot-input'), res = $('#spot-res');
    $('#tb-search').onclick = () => this.spot(true);
    ov.onclick = e => { if (e.target === ov) this.spot(false); };
    inp.addEventListener('input', () => {
      const q = inp.value.trim().toLowerCase();
      res.innerHTML = '';
      if (!q) return;
      const items = this.searchAll(q);
      if (!items.length){ res.appendChild(el('div', 'empty', 'Ничего не найдено')); return; }
      items.forEach((it, i) => { const n = this.resNode(it); if (!i) n.classList.add('on'); res.appendChild(n); });
    });
    inp.addEventListener('keydown', e => {
      const list = $$('.sres', res);
      let i = list.findIndex(n => n.classList.contains('on'));
      if (e.key === 'ArrowDown'){ e.preventDefault(); i = Math.min(list.length - 1, i + 1); }
      else if (e.key === 'ArrowUp'){ e.preventDefault(); i = Math.max(0, i - 1); }
      else if (e.key === 'Enter'){ if (list[i]) list[i].click(); return; }
      else if (e.key === 'Escape'){ this.spot(false); return; }
      else return;
      list.forEach(n => n.classList.remove('on'));
      if (list[i]) list[i].classList.add('on');
    });
  },
  spot(on){
    this.closePanels(true);
    $('#spot-overlay').classList.toggle('on', on);
    if (on){ $('#spot-input').value = ''; $('#spot-res').innerHTML = ''; setTimeout(() => $('#spot-input').focus(), 120); }
  },

  /* ================= ЦЕНТР УПРАВЛЕНИЯ ================= */
  renderCC(){
    const cc = $('#cc');
    cc.innerHTML = `<div class="cc-grid"></div>
      <div class="cc-head">Экран и звук</div>
      <div class="tile wide" id="cc-bright"></div>
      <div class="tile wide" id="cc-vol"></div>
      <div class="cc-head">Сейчас играет</div>
      <div class="tile wide" id="cc-media"></div>
      <div class="cc-head">Быстро</div>
      <div class="cc-grid" id="cc-more"></div>`;
    const grid = $('.cc-grid', cc);

    const T = (icon, title, sub, get, set) => {
      const t = el('div', 'tile btn' + (get() ? ' on' : ''));
      t.innerHTML = `<div class="tile-ico">${icon}</div><div><div class="tt">${title}</div><span class="ts">${sub(get())}</span></div>`;
      t.onclick = () => { const v = !get(); set(v); t.classList.toggle('on', v); $('.ts', t).textContent = sub(v); };
      return t;
    };
    // сеть и устройства показывают настоящее состояние и ведут в Параметры
    const netTile = el('div', 'tile btn');
    const paintNet = () => {
      const on = navigator.onLine;
      netTile.classList.toggle('on', on);
      netTile.innerHTML = `<div class="tile-ico">${on ? '📶' : '📴'}</div>
        <div><div class="tt">Сеть</div><span class="ts">${on ? 'подключено' : 'нет подключения'}</span></div>`;
    };
    paintNet();
    addEventListener('online', paintNet); addEventListener('offline', paintNet);
    netTile.onclick = () => { WM.open('settings', { section:'net' }); this.closePanels(); };

    const devTile = el('div', 'tile btn');
    devTile.innerHTML = `<div class="tile-ico">🎧</div><div><div class="tt">Устройства</div>
      <span class="ts">камеры, звук, батарея</span></div>`;
    devTile.onclick = () => { WM.open('settings', { section:'bt' }); this.closePanels(); };

    grid.append(
      netTile, devTile,
      T('🌙', 'Не беспокоить', v => v ? 'Вкл' : 'Выкл', () => S.dnd, v => Store.set('dnd', v)),
      T('🌗', 'Тема', v => v ? (S.theme === 'dark' ? 'Тёмная' : 'Прозрачная') : 'Светлая',
        () => S.theme !== 'light',
        v => { if (v) Store.set('theme', KV.get('darkVariant', 'glass'));
               else { KV.set('darkVariant', S.theme); Store.set('theme', 'light'); } })
    );
    $$('.tile', grid).forEach((t, i) => t.style.setProperty('--i', i));

    const br = $('#cc-bright');
    br.innerHTML = `<div class="tt">Яркость</div>`;
    const brs = slider(() => S.brightness, v => Store.set('brightness', v), 30, 100, 1, v => v + '%');
    brs.classList.add('sl-row'); $('input', brs).style.width = '100%';
    br.appendChild(brs);

    const vo = $('#cc-vol');
    vo.innerHTML = `<div class="tt">Громкость</div>`;
    const vos = slider(() => S.volume, v => { Store.set('volume', v); }, 0, 100, 1, v => v + '%');
    vos.classList.add('sl-row'); $('input', vos).style.width = '100%';
    vo.appendChild(vos);

    const more = $('#cc-more');
    more.append(
      T('🌡', 'Ночной свет', v => v ? 'Вкл' : 'Выкл', () => S.nightLight, v => Store.set('nightLight', v)),
      T('🔍', 'Увеличение дока', v => v ? 'Вкл' : 'Выкл', () => S.dockMagOn, v => Store.set('dockMagOn', v)),
      T('👻', 'Автоскрытие дока', v => v ? 'Вкл' : 'Выкл', () => S.dockAutohide, v => Store.set('dockAutohide', v))
    );
    const settingsBtn = el('div', 'tile btn wide');
    settingsBtn.innerHTML = `<div class="tile-ico">⚙️</div><div><div class="tt">Все параметры</div><span class="ts">Открыть Параметры</span></div>`;
    settingsBtn.onclick = () => { WM.open('settings'); this.closePanels(); };
    more.appendChild(settingsBtn);
    this.updateCC();
  },

  updateCC(){
    const m = $('#cc-media'); if (!m) return;
    const np = this.nowPlaying;
    m.innerHTML = `<div class="cc-media">
      <div class="cc-art">${np ? np.e : '🎵'}</div>
      <div><div class="tt">${np ? esc(np.t) : 'Ничего не воспроизводится'}</div>
      <span class="ts">${np ? esc(np.a) : 'Откройте Музыку'}</span></div>
      <div class="cc-mbtns">
        <button data-a="prev">⏮</button><button data-a="pp">${np ? '⏸' : '▶'}</button><button data-a="next">⏭</button>
      </div></div>`;
    $$('.cc-mbtns button', m).forEach(b => b.onclick = () => {
      const w = WM.wins.find(x => x.appId === 'music');
      if (!w){ WM.open('music'); return; }
      const sel = { prev:'[data-a="prev"]', pp:'[data-a="pp"]', next:'[data-a="next"]' }[b.dataset.a];
      const btn = w.body.querySelector('.mu-ctl ' + sel); if (btn) btn.click();
    });
  },

  /* ================= ВИДЖЕТЫ ================= */
  weather(){
    const seed = (S.city || '').length + new Date().getDate();
    const temps = [21, 18, 24, 15, 27, 12, 19, 23];
    const icons = ['☀️','⛅','🌧','❄️','🌤','🌩','🌫','🌥'];
    const t = temps[seed % temps.length];
    return { t, ico:icons[seed % icons.length],
      desc:['Ясно','Переменная облачность','Дождь','Снег','Малооблачно','Гроза','Туман','Облачно'][seed % 8],
      days:[1,2,3,4,5].map(i => ({ d:['Пн','Вт','Ср','Чт','Пт','Сб','Вс'][(new Date().getDay() - 1 + i + 7) % 7],
        i:icons[(seed + i) % icons.length], t:t + ((seed + i) % 7) - 3 })) };
  },

  widgetCards(){
    const w = this.weather(), now = new Date();
    const events = KV.get('cal.events', {});
    const evList = Object.entries(events).slice(0, 3);
    const todo = KV.get('todo', []).filter(t => !t.d).slice(0, 3);
    return [
      { t:'Погода · ' + S.city, html:`<div class="row"><div style="font-size:38px">${w.ico}</div>
          <div><div class="w-big">${w.t > 0 ? '+' : ''}${w.t}°</div><div class="tiny muted">${w.desc}</div></div></div>
          <div class="forecast">${w.days.map(d => `<div>${d.d}<span>${d.i}</span>${d.t > 0 ? '+' : ''}${d.t}°</div>`).join('')}</div>` },
      { t:'Календарь', html:`<div class="w-big">${now.getDate()}</div>
          <div class="tiny muted">${now.toLocaleDateString('ru-RU', { weekday:'long', month:'long' })}</div>
          ${evList.length ? evList.map(([k, v]) => { const e2 = window.Reminders ? Reminders.norm(v) : { t:v, time:'' };
            return `<div class="w-row"><span>${esc(e2.t)}</span><span class="muted">${e2.time || k.split('-')[2]}</span></div>`; }).join('')
                          : '<div class="w-row muted">Событий нет</div>'}` },
      { t:'Задачи', html:todo.length ? todo.map(t => `<div class="w-row"><span>○ ${esc(t.t)}</span></div>`).join('')
                                      : '<div class="w-row muted">Всё сделано 🎉</div>' },
      { t:'Система', html:`<div class="w-row"><span>Окон открыто</span><b>${WM.wins.length}</b></div>
          <div class="w-row"><span>Рабочий стол</span><b>${WM.desk + 1} / ${WM.desks}</b></div>
          <div class="w-row"><span>Тема</span><b>${{ dark:'Тёмная', light:'Светлая' }[S.theme] || 'Прозрачная'}</b></div>
          <div class="w-row"><span>Размытие</span><b>${S.blur}px</b></div>` }
    ];
  },

  renderWidgets(){
    const box = $('#widgets');
    box.innerHTML = `<div class="w-t" style="font-size:14px;opacity:1;margin-bottom:10px">Виджеты</div>`;
    this.widgetCards().forEach((c, i) => {
      const n = el('div', 'w-card', `<div class="w-t">${c.t}</div>${c.html}`);
      n.style.setProperty('--i', i);
      box.appendChild(n);
    });
  },

  renderDeskWidgets(){
    const box = $('#desk-widgets');
    if (!box) return;
    box.innerHTML = '';
    if (!S.showDeskWidgets) return;
    const now = new Date();
    const w = this.weather();
    const cards = [
      `<div class="w-t">Часы</div><div class="w-big" id="dw-clock">${pad2(now.getHours())}:${pad2(now.getMinutes())}</div>
       <div class="tiny muted">${now.toLocaleDateString('ru-RU', { weekday:'long', day:'numeric', month:'long' })}</div>`,
      `<div class="w-t">Погода · ${esc(S.city)}</div><div class="row"><div style="font-size:30px">${w.ico}</div>
       <div><div class="w-big">${w.t > 0 ? '+' : ''}${w.t}°</div><div class="tiny muted">${w.desc}</div></div></div>`
    ];
    cards.forEach((h, i) => { const n = el('div', 'w-card glass', h); n.style.setProperty('--i', i); box.appendChild(n); });
  },

  /* ================= ИКОНКИ РАБОЧЕГО СТОЛА ================= */
  renderIcons(){
    const box = $('#desktop-icons');
    box.innerHTML = '';
    const items = [
      { g:'💻', n:'Этот компьютер', f:() => WM.open('files', { path:[] }) },
      { g:'🗑️', n:'Корзина', f:() => this.toast('Корзина', 'Корзина пуста', '🗑️') }
    ];

    /* Программы, вынесенные на стол, рисует не эта отрисовка, а та, что в
       realism.js: она заменяет эту целиком — со своей раскладкой значков,
       перетаскиванием и памятью мест. Держать здесь вторую копию значит
       писать код, который никогда не выполнится: я так и сделал, и заметил
       только по упавшей проверке. */
    const desk = FS.node(['Рабочий стол']);
    Object.values(desk ? desk.children : {}).forEach(f => items.push({
      g: f.type === 'dir' ? '📁' : f.img ? '🖼️' : '📄', n:f.name, node:f,
      f: () => f.type === 'dir' ? WM.open('files', { path:['Рабочий стол', f.name] })
             : f.img ? WM.open('photos', { img:f.img, name:f.name })
             : WM.open('notepad', { file:{ name:f.name, path:['Рабочий стол'], body:f.body } })
    }));

    items.forEach(it => {
      const n = el('div', 'di', `<div class="glyph">${it.g}</div><div class="lbl">${esc(it.n)}</div>`);
      /* У программы машины значок настоящий, её собственный: человек должен
         узнавать её на столе так же, как в любой другой системе. */
      n.onclick = e => { e.stopPropagation(); $$('.di', box).forEach(x => x.classList.remove('sel')); n.classList.add('sel'); };
      n.ondblclick = it.f;
      n.oncontextmenu = e => {
        e.preventDefault(); e.stopPropagation();
        this.ctx(e.clientX, e.clientY, [
          { i:'📂', t:'Открыть', f:it.f },
          ...(it.node ? [{ i:'✏️', t:'Переименовать', f: async () => {
            const nn = await Dlg.prompt('Переименовать', 'Новое имя', it.n, '✏️');
            if (nn){ FS.rename(['Рабочий стол'], it.n, nn); this.renderIcons(); } } },
                         { i:'🗑️', t:'Удалить', f:() => { FS.rm(['Рабочий стол'], it.n); this.renderIcons(); } }] : [])
        ]);
      };
      box.appendChild(n);
    });
  },

  /* ================= TASK VIEW ================= */
  renderTaskview(){
    const desks = $('#tv-desks'), grid = $('#tv-grid');
    if (!desks) return;
    desks.innerHTML = '';
    for (let i = 0; i < WM.desks; i++){
      const d = el('div', 'tv-desk' + (i === WM.desk ? ' on' : ''), 'Рабочий стол ' + (i + 1));
      d.style.backgroundImage = (WALLPAPERS.find(w => w.id === S.wallpaper) || WALLPAPERS[0]).css;
      d.onclick = () => { WM.gotoDesk(i); this.renderTaskview(); };
      desks.appendChild(d);
    }
    grid.innerHTML = '';
    const list = WM.wins.filter(w => w.desk === WM.desk);
    if (!list.length){ grid.appendChild(el('div', 'empty', 'Нет открытых окон')); return; }
    list.forEach((w, i) => {
      const n = el('div', 'tv-item');
      n.style.setProperty('--i', i);
      n.innerHTML = `<div class="tv-head"><span>${w.app.glyph}</span><span>${esc(w.app.name)}</span><button class="tv-close">×</button></div>
        <div class="tv-prev"></div>`;
      const prev = $('.tv-prev', n);
      prev.style.background = w.app.bg;
      prev.style.opacity = '.5';
      prev.innerHTML = `<div style="font-size:52px;text-align:center;padding-top:26px;opacity:.8">${w.app.glyph}</div>`;
      $('.tv-close', n).onclick = e => { e.stopPropagation(); WM.close(w); setTimeout(() => this.renderTaskview(), 260); };
      n.onclick = () => { this.taskview(false); if (w.minimized) WM.restore(w); else WM.focus(w); };
      grid.appendChild(n);
    });
  },
  /* ================= ОБЗОР =================
     Меню программ во весь экран, как в GNOME: наверху рабочие столы, ниже
     поиск, открытые окна и все программы разом.

     Почему здесь, у стола, а не в панели, где меню было раньше: обои
     сквозь обзор видны только тому, кто на них и нарисован. Панель —
     отдельная поверхность, и «прозрачный фон» у неё означал бы серую
     пустоту. Заодно здесь под рукой всё нужное: и наши программы, и
     программы машины, и окна, и рабочие столы. */
  рисуйОбзор(){
    const столы = $('#об-столы'), окна = $('#об-окна'), сетка = $('#об-сетка');
    if (!столы) return;

    /* Размытые обои под обзором — те же самые, что на столе, включая
       выбранную человеком картинку: иначе за стеклом оказались бы чужие
       обои, и подмена была бы видна сразу. */
    const фон = $('#об-фон');
    if (фон){
      const о = WALLPAPERS.find(w => w.id === S.wallpaper) || WALLPAPERS[0];
      фон.style.backgroundImage = (S.wallpaper === 'custom' && window.__customWall)
        ? `url(${window.__customWall})` : о.css;
    }

    столы.innerHTML = '';
    for (let i = 0; i < WM.desks; i++){
      const сколько = WM.wins.filter(w => w.desk === i).length;
      const д = el('div', 'об-стол' + (i === WM.desk ? ' on' : ''));
      д.style.backgroundImage = (WALLPAPERS.find(w => w.id === S.wallpaper) || WALLPAPERS[0]).css;
      д.appendChild(el('div', 'номер', (i + 1) + (сколько ? ' · ' + сколько : '')));
      д.onclick = () => { WM.gotoDesk(i); this.рисуйОбзор(); };

      /* Пустой стол можно убрать — иначе заведённые столы только копятся:
         кнопка есть, а обратного пути нет. Со столом, на котором лежат
         окна, так нельзя: он унёс бы их с собой в никуда. */
      if (!сколько && WM.desks > 1){
        const х = el('button', 'об-стол-убрать', '×');
        х.title = 'Убрать этот рабочий стол';
        х.onclick = e => { e.stopPropagation(); if (WM.убериСтол(i)) this.рисуйОбзор(); };
        д.appendChild(х);
      }

      /* Перетаскивание программы на стол: она там и откроется.
      
         Так это работает в GNOME, и так это единственный способ открыть
         программу не там, где стоишь: иначе надо сперва уйти на нужный
         стол, открыть, и вернуться — три действия вместо одного. */
      д.ondragover = e => { e.preventDefault(); д.classList.add('примет'); };
      д.ondragleave = () => д.classList.remove('примет');
      д.ondrop = e => {
        e.preventDefault();
        д.classList.remove('примет');
        let груз = {};
        try { груз = JSON.parse(e.dataTransfer.getData('text/plain') || '{}'); } catch(err){}
        if (груз.окно != null){
          const w = WM.wins.find(x => x.node && x.node.id === груз.окно);
          if (w) WM.наСтол(w, i);
          this.рисуйОбзор();
          return;
        }
        if (!груз.id) return;
        this.обзор(false);
        if (груз['вид'] === 'машина'){
          /* Программа машины открывается своим окном, и оконный сервер
             про наши столы ничего не знает: положить её на соседний стол
             нам нечем. Честно уходим туда сами и открываем там. */
          if (i !== WM.desk) WM.gotoDesk(i);
          if (window.OS && OS.запустиПоЯрлыку) OS.запустиПоЯрлыку(груз.id);
          return;
        }
        /* Открываем сами, а не через launch: тот умеет ещё и свёртывать
           уже открытое — для щелчка по значку это верно, а для броска на
           стол означало бы, что программа то откроется, то спрячется. */
        const окно = WM.open(груз.id);
        if (окно && i !== WM.desk) WM.наСтол(окно, i);
      };
      столы.appendChild(д);
    }

    /* Кнопка нового стола — последней в ряду, как в GNOME. */
    if (WM.desks < 8){
      const плюс = el('button', 'об-стол об-стол-плюс', '+');
      плюс.title = 'Новый рабочий стол';
      плюс.onclick = () => { WM.добавьСтол(); this.рисуйОбзор(); };
      /* На неё тоже можно бросить программу: стол заведётся и программа
         откроется сразу на нём. */
      плюс.ondragover = e => { e.preventDefault(); плюс.classList.add('примет'); };
      плюс.ondragleave = () => плюс.classList.remove('примет');
      плюс.ondrop = e => {
        e.preventDefault();
        плюс.classList.remove('примет');
        const куда = WM.добавьСтол();
        this.рисуйОбзор();
        const стол = столы.children[куда];
        if (стол && стол.ondrop) стол.ondrop(e);
      };
      столы.appendChild(плюс);
    }

    окна.innerHTML = '';
    WM.wins.filter(w => w.desk === WM.desk).forEach(w => {
      const к = el('div', 'об-окно');
      к.appendChild(appIcon(w.app));
      к.appendChild(el('span', '', w.titleEl ? w.titleEl.textContent : w.app.name));
      к.onclick = () => { this.обзор(false); if (w.minimized) WM.restore(w); else WM.focus(w); };
      /* Открытое окно тоже перетаскивается на другой стол — переехать
         должно уметь и то, что уже открыто, а не только новое. */
      к.draggable = true;
      к.ondragstart = e => e.dataTransfer.setData('text/plain',
        JSON.stringify({ 'окно':w.node.id }));
      окна.appendChild(к);
    });

    const что = ($('#об-искать') && $('#об-искать').value || '').trim().toLowerCase();
    /* Список — тот же, что уходит панели: наши программы и настоящие
       программы машины вместе, настоящая главнее одноимённой нарисованной. */
    const все = (window.Поверхности && Поверхности.программы)
      ? Поверхности.программы()
      : Object.keys(APPS).map(id => ({ id, 'имя':APPS[id].name || id, 'вид':'приложение' }));
    const видно = все.filter(п => !что || (п['имя'] || '').toLowerCase().includes(что));

    сетка.innerHTML = '';
    /* Сетка листается, и место, где её оставили, живёт в узле: обзор один и
       тот же, он не создаётся заново. Поэтому в следующий раз он открывался
       прокрученным — у первого ряда были видны только подписи, а значки
       уезжали за верхний край. Каждый раз начинаем сверху. */
    сетка.scrollTop = 0;
    if (!видно.length){
      сетка.appendChild(el('div', 'об-пусто', 'Ничего не нашлось'));
      return;
    }
    видно.forEach((п, i) => {
      const б = el('button', 'об-прог');
      б.style.setProperty('--i', i);
      if (п['вид'] === 'машина' && window.OS && OS.значокПрограммы)
        б.appendChild(OS.значокПрограммы({ id:п.id, name:п['имя'], значок:п['значок'] || '',
                                           flatpak:п.flatpak }, 'app-ico'));
      else if (APPS[п.id]) б.appendChild(appIcon(APPS[п.id]));
      else {
        const з = el('div', 'app-ico', п['знак'] || '▢');
        з.style.background = п['фон'] || 'rgba(255,255,255,.12)';
        б.appendChild(з);
      }
      б.appendChild(el('span', 'имя', п['имя'] || п.id));
      б.draggable = true;
      б.ondragstart = e => e.dataTransfer.setData('text/plain',
        JSON.stringify({ id:п.id, 'вид':п['вид'] || 'приложение' }));
      б.onclick = () => {
        this.обзор(false);
        if (п['вид'] === 'машина' && window.OS && OS.запустиПоЯрлыку) OS.запустиПоЯрлыку(п.id);
        else this.launch(п.id);
      };
      сетка.appendChild(б);
    });
  },

  привяжиОбзор(){
    const узел = $('#обзор'), поле = $('#об-искать');
    if (!узел || !поле) return;
    /* Щелчок по пустому месту закрывает — как в GNOME. По значку или по
       рабочему столу наверху не закрывает: там у человека дело. */
    узел.onclick = e => { if (e.target === узел) this.обзор(false); };
    поле.addEventListener('input', () => this.рисуйОбзор());
    поле.addEventListener('keydown', e => {
      if (e.key === 'Escape'){ this.обзор(false); return; }
      /* Ввод открывает первое найденное: искать и тянуться к мыши — работа
         на два движения там, где хватает одного. */
      if (e.key === 'Enter'){
        const первый = узел.querySelector('.об-прог');
        if (первый) первый.click();
      }
    });
  },

  обзор(on){
    const узел = $('#обзор');
    if (!узел) return;
    const v = on != null ? on : !узел.classList.contains('on');
    if (v){
      this.closePanels(true);
      const поле = $('#об-искать');
      if (поле) поле.value = '';
      this.рисуйОбзор();
    }
    узел.classList.toggle('on', v);
    document.body.classList.toggle('обзор-открыт', v);
    /* Клавиатуру забираем только когда открыто: пустая строка поиска,
       молча съедающая нажатия, — худшее, что может делать меню. */
    if (v){ const поле = $('#об-искать'); if (поле) setTimeout(() => поле.focus(), 80); }
  },

  taskview(on){
    const v = on != null ? on : !$('#taskview').classList.contains('on');
    if (v) this.renderTaskview();
    $('#taskview').classList.toggle('on', v);
    document.body.classList.toggle('blurred', v);
  },

  /* ================= ПАНЕЛИ ================= */
  bindTray(){
    $('#tray-cc').onclick = e => { e.stopPropagation(); this.panel('#cc'); this.updateCC(); };
    $('#tray-widgets').onclick = e => { e.stopPropagation(); this.renderWidgets(); this.panel('#widgets'); };
    $('#tray-clock').onclick = e => { e.stopPropagation(); this.renderWidgets(); this.panel('#widgets'); };
    $('#tray-bat').onclick = () => this.toast('Батарея', 'Заряд 87% · осталось ~4 ч 20 мин', '🔋');
    $('#tb-weather').onclick = e => { e.stopPropagation(); this.renderWidgets(); this.panel('#widgets'); };
    $('#cc').onclick = e => e.stopPropagation();
    $('#widgets').onclick = e => e.stopPropagation();
  },
  panel(sel){
    const on = !$(sel).classList.contains('on');
    this.closePanels(true);
    $(sel).classList.toggle('on', on);
    document.body.classList.toggle('panel-open', on && sel === '#cc');
  },
  closePanels(keepStart){
    $('#cc').classList.remove('on');
    $('#widgets').classList.remove('on');
    $('#ctx').classList.remove('on');
    document.body.classList.remove('panel-open');
    if (!keepStart){
      document.body.classList.remove('start-open');
      $('#spot-overlay').classList.remove('on');
    }
  },

  /* ================= КОНТЕКСТНОЕ МЕНЮ ================= */
  ctx(x, y, items){
    const c = $('#ctx');
    c.innerHTML = '';
    items.forEach(it => {
      if (it === 'hr'){ c.appendChild(el('hr')); return; }
      if (it.head){ c.appendChild(el('div', 'ctx-head', esc(it.t))); return; }
      const b = el('button', '', `<span class="em">${it.i || ''}</span><span>${esc(it.t)}</span>${it.k ? `<span class="k">${it.k}</span>` : ''}`);
      b.onclick = () => { c.classList.remove('on'); it.f && it.f(); };
      c.appendChild(b);
    });
    c.classList.add('on');
    const r = c.getBoundingClientRect();
    c.style.left = Math.min(x, innerWidth - r.width - 10) + 'px';
    c.style.top = Math.min(y, innerHeight - r.height - 10) + 'px';
  },

  bindDesktop(){
    const d = $('#desktop');
    d.addEventListener('click', e => {
      /* Если по этому щелчку меню перестроилось, старый элемент уже выброшен
         из документа — и проверка «щёлкнули мимо» ошибочно закрывала только
         что открытое меню. Отсюда пропадали подменю. */
      if (!document.contains(e.target)) return;
      if (!e.target.closest('.win, .dock, .start, #cc, #widgets, .topbar, .ctx')) this.closePanels();
      if (!e.target.closest('.di')) $$('.di').forEach(x => x.classList.remove('sel'));
    });
    d.addEventListener('contextmenu', e => {
      if (e.target.closest('.win, .dock, .start, .di, #cc, #widgets')) return;
      e.preventDefault();
      this.ctx(e.clientX, e.clientY, [
        { i:'🔄', t:'Обновить', f:() => this.renderShell() },
        { i:'📄', t:'Создать текстовый файл', f: async () => {
            const n = await Dlg.prompt('Создать файл', 'Имя нового файла', 'Новый.txt', '📄');
            if (n){ FS.write(['Рабочий стол'], FS.uniqueName(['Рабочий стол'], n), ''); this.renderIcons(); } } },
        { i:'📁', t:'Создать папку', f: async () => {
            const n = await Dlg.prompt('Создать папку', 'Имя новой папки', 'Новая папка', '📁');
            if (n){ FS.mkdir(['Рабочий стол'], FS.uniqueName(['Рабочий стол'], n)); this.renderIcons(); } } },
        'hr',
        { i:'▦', t:'Разложить окна', f:() => WM.tile(), k:'' },
        { i:'🗂', t:'Каскад окон', f:() => WM.cascade() },
        { i:'🪟', t:'Просмотр задач', f:() => this.taskview(true), k:'Win+Tab' },
        'hr',
        { i:'🖼️', t:'Сменить обои', f:() => WM.open('settings', { section:'person' }) },
        { i:'⚙️', t:'Параметры', f:() => WM.open('settings') }
      ]);
    });
    // автоскрытие дока
    addEventListener('mousemove', e => {
      if (!S.dockAutohide) return;
      const near = e.clientY > innerHeight - 60;
      document.body.classList.toggle('dock-peek', near);
      document.body.classList.toggle('dock-hidden', !near);
    });
  },

  hotCorners(){
    let fired = false;
    addEventListener('mousemove', e => {
      if (!S.hotcorners) return;
      const corner = e.clientX > innerWidth - 6 && e.clientY < 6;
      if (corner && !fired){ fired = true; this.taskview(true); }
      if (!corner) fired = false;
    });
  },

  /* ================= УВЕДОМЛЕНИЯ ================= */
  /* Последним доводом можно передать дело: тогда нажатие на сообщение его
     выполняет — так открывается воткнутая флешка одним щелчком. */
  toast(title, text, icon = '🔔', ms = 4200, дело = null){
    if (S.dnd) return;
    const t = el('div', 'toast');
    t.innerHTML = `<div class="app-ico" style="background:linear-gradient(140deg,var(--accent),var(--accent-2))">${icon}</div>
      <div class="tx"><b>${esc(title)}</b>${esc(text)}</div>`;
    if (дело) t.style.cursor = 'pointer';
    t.onclick = () => { if (дело){ try { дело(); } catch(e){} } close(); };
    $('#toasts').appendChild(t);
    Snd.note();
    const close = () => { t.classList.add('out'); setTimeout(() => t.remove(), 320); };
    setTimeout(close, ms);
  },

  /* ================= ЧАСЫ / ФОН / ТЕМА ================= */
  clock(){
    /* raw — настоящий момент, его форматируют функции локали (они сами знают
       про выбранный пояс); n — тот же момент, сдвинутый так, чтобы getHours()
       показывал часы выбранного пояса. */
    const raw = new Date();
    const n = zoned(raw);
    let h = n.getHours();
    const suf = S.clock24 ? '' : (h >= 12 ? ' PM' : ' AM');
    if (!S.clock24) h = h % 12 || 12;
    const time = `${S.clock24 ? pad2(h) : h}:${pad2(n.getMinutes())}${S.showSeconds ? ':' + pad2(n.getSeconds()) : ''}${suf}`;
    const date = raw.toLocaleDateString('ru-RU', { day:'numeric', month:'short' });
    const tt = $('#tray-time'), td = $('#tray-date');
    if (tt) tt.textContent = time;
    if (td) td.textContent = date;
    /* Виджет часов на столе рисуется с классом, а искали его по
       идентификатору — и он не тикал вовсе: показывал минуту, в которую его
       нарисовали, и замирал. Обновляем оба написания. */
    const цифры = `${pad2(n.getHours())}:${pad2(n.getMinutes())}`;
    const dw = $('#dw-clock'); if (dw) dw.textContent = цифры;
    $$('.dw-clock').forEach(э => { э.textContent = цифры; });
    const tb = $('#tbw-temp'); if (tb){ const w = this.weather(); tb.textContent = (w.t > 0 ? '+' : '') + w.t + '°'; $('#tb-weather .tbw-icon').textContent = w.ico; }
  },

  wallShuffle(){
    clearInterval(this.wallTimer);
    if (!S.wallShuffle) return;
    this.wallTimer = setInterval(() => {
      const i = WALLPAPERS.findIndex(w => w.id === S.wallpaper);
      const next = WALLPAPERS[(i + 1) % WALLPAPERS.length];
      const nx = $('#wallpaper-next');
      nx.style.backgroundImage = next.css;
      nx.style.opacity = '1';
      setTimeout(() => { Store.set('wallpaper', next.id); nx.style.opacity = '0'; }, 700);
    }, 30000);
  },

  autoTheme(){
    clearInterval(this._themeIv);
    if (!S.autoTheme) return;
    const upd = () => { const h = zoned().getHours();
      Store.set('theme', (h >= 8 && h < 20) ? 'light' : 'dark'); };
    upd(); this._themeIv = setInterval(upd, 60000);
  },

  /* Блик за курсором убран вместе со стеклом: поверхности плотные, и водить
     по ним светом больше нечем. Имя оставлено — его зовут при запуске. */
  bindGlassPointer(){},

  /* ================= КЛАВИАТУРА ================= */
  bindKeys(){
    addEventListener('keydown', e => {
      const typing = /INPUT|TEXTAREA/.test(document.activeElement.tagName);

      if (e.key === 'Meta' && !e.repeat && !typing){ e.preventDefault(); this.toggleStart(); return; }
      if (e.metaKey || e.ctrlKey){
        if (e.key === ' '){ e.preventDefault(); this.spot(true); return; }
        if (e.key.toLowerCase() === 'w' && !typing){ const t = WM.top(); if (t){ e.preventDefault(); WM.close(t); } return; }
        if (e.key === 'Tab' && e.metaKey){ e.preventDefault(); this.taskview(); return; }
        if (e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')){
          e.preventDefault();
          WM.gotoDesk(clamp(WM.desk + (e.key === 'ArrowRight' ? 1 : -1), 0, WM.desks - 1));
          return;
        }
        if (e.key.toLowerCase() === 'd' && !typing){ e.preventDefault(); WM.minimizeAll(); return; }
        const t = WM.top();
        if (t && ['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key) && e.metaKey){
          e.preventDefault();
          if (e.key === 'ArrowUp') WM.snap(t, 'max');
          else if (e.key === 'ArrowDown'){ if (t.maximized) WM.toggleMax(t); else WM.minimize(t); }
          else WM.snap(t, e.key === 'ArrowLeft' ? 'left' : 'right');
          return;
        }
      }
      if (e.key === 'Escape'){
        if ($('#обзор') && $('#обзор').classList.contains('on')) { this.обзор(false); return; }
        if ($('#taskview').classList.contains('on')) { this.taskview(false); return; }
        if ($('#power-overlay').classList.contains('on')) { $('#power-overlay').classList.remove('on'); return; }
        this.closePanels();
      }
      if (e.altKey && e.key === 'Tab'){
        e.preventDefault();
        const list = WM.wins.filter(w => !w.minimized && w.desk === WM.desk);
        if (list.length > 1){ const t = WM.top(); const i = list.indexOf(t); WM.focus(list[(i + 1) % list.length]); }
      }
    });
    addEventListener('keyup', e => {
      if (e.key === 'Meta') e.preventDefault();
    });
  },

  /* ================= ПИТАНИЕ ================= */
  power(act){
    $('#power-overlay').classList.remove('on');
    if (act === 'cancel') return;
    if (act === 'lock'){ this.lock(); return; }
    if (act === 'sleep'){
      const f = el('div', 'shutdown-fade', '<div style="text-align:center;opacity:.7">Спящий режим…<br><small>Нажмите, чтобы разбудить</small></div>');
      document.body.appendChild(f);
      f.onclick = () => { f.remove(); };
      return;
    }
    const f = el('div', 'shutdown-fade');
    f.innerHTML = act === 'restart'
      ? '<div style="text-align:center"><div class="boot-ring"><svg viewBox="0 0 50 50"><circle cx="25" cy="25" r="20"/></svg></div><div style="margin-top:14px;opacity:.7">Перезагрузка…</div></div>'
      : '<div style="opacity:.6">Завершение работы…</div>';
    document.body.appendChild(f);
    setTimeout(() => location.reload(), act === 'restart' ? 1800 : 1500);
  },

  /* Настоящая блокировка экрана.

     Прежний замок был картинкой внутри оболочки: он закрывал собой страницу
     и спрашивал пароль у самой оболочки. Защищал он ровно ничего — под ним
     продолжали работать чужие окна, а уйти из-под него можно было
     переключением консоли. Хуже того, он появлялся при входе вторым
     вопросом после настоящего.

     Теперь блокировкой занимается система: экран гасит отдельная программа,
     которая держит его до правильного пароля и спрашивает этот пароль у
     PAM. Программы при этом продолжают работать — именно этим блокировка и
     отличается от выхода из системы. */
  async lock(){
    this.closePanels();
    if (!window.Platform || Platform.mode !== 'native'){
      Shell.toast('Блокировка', 'Здесь нечего блокировать: оболочка работает без системы', '🔒');
      return;
    }
    try { await Platform.rpc('sys.power', { action:'lock' }); }
    catch(e){ Dlg.alert('Заблокировать не вышло', String(e.message || e), '🔒'); }
  }
};

/* Оболочка объявлена через const, а const на window не попадает. Пока всё
   жило одной страницей, это никому не мешало: соседние файлы видят её и так.
   Но стоило появиться коду, который осторожно проверяет «window.Shell есть?»,
   как проверка стала молча отвечать «нет» — и подмены не ставились вовсе,
   без единой ошибки в журнале. Поэтому объявляем явно, как это давно делает
   Platform. */
window.Shell = Shell;
