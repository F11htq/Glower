/* ==========================================================================
   Первый запуск: начальная настройка и приветствие

   Как в Windows при первом включении: язык, регион, имя, оформление,
   вход. Разница в одном — здесь не спрашивают согласия на то, чего
   система не делает: ни учётной записи в облаке, ни сбора данных,
   ни рекламного идентификатора. Всё, что вы выберете, останется на
   этой машине.
   ========================================================================== */
'use strict';

const Setup = {
  KEY:'setup.done',

  needed(){ return !KV.get(this.KEY, false); },
  finish(){ KV.set(this.KEY, true); },

  /* собранные ответы */
  data:{ lang:'ru', layouts:['en', 'ru'], city:'Москва', name:'', emoji:'',
         pass:'', theme:'glass', accent:0, wallpaper:'bloom' },

  /* ---------- каркас ---------- */
  open(){
    const ov = el('div', 'setup');
    ov.id = 'setup';
    ov.innerHTML = `
      <div class="setup-bg"></div>
      <div class="setup-box glass lg">
        <div class="setup-head">
          <img class="os-logo sm" src="assets/logo.png" alt="" draggable="false">
          <div class="setup-steps" id="setup-steps"></div>
        </div>
        <div class="setup-body" id="setup-body"></div>
        <div class="setup-foot">
          <button class="btn" id="setup-back">Назад</button>
          <div class="grow"></div>
          <button class="btn pri" id="setup-next">Далее</button>
        </div>
      </div>`;
    document.body.appendChild(ov);
    requestAnimationFrame(() => ov.classList.add('on'));

    this.i = 0;
    this.paint();

    $('#setup-back', ov).onclick = () => { if (this.i > 0){ this.i--; this.paint(); } };
    $('#setup-next', ov).onclick = () => {
      const s = this.STEPS[this.i];
      if (s.check && !s.check.call(this)) return;
      if (this.i < this.STEPS.length - 1){ this.i++; this.paint(); Snd.click(); }
      else this.done();
    };
  },

  paint(){
    const box = $('#setup-body'), steps = $('#setup-steps');
    const s = this.STEPS[this.i];
    steps.innerHTML = this.STEPS.map((x, k) =>
      `<i class="${k === this.i ? 'on' : k < this.i ? 'was' : ''}"></i>`).join('');
    box.innerHTML = `<div class="setup-t">${s.title}</div>
      ${s.sub ? `<div class="setup-s">${s.sub}</div>` : ''}`;
    const area = el('div', 'setup-area');
    box.appendChild(area);
    s.fill.call(this, area);
    $('#setup-back').style.visibility = this.i ? 'visible' : 'hidden';
    $('#setup-next').textContent = this.i === this.STEPS.length - 1 ? 'Начать работу' : 'Далее';
    box.style.animation = 'none';
    requestAnimationFrame(() => box.style.animation = '');
  },

  /* ---------- вспомогательное ---------- */
  cards(area, items, get, set){
    const g = el('div', 'setup-cards');
    items.forEach(it => {
      const c = el('button', 'setup-card' + (get() === it.v ? ' on' : ''));
      c.innerHTML = `${it.big ? `<span class="big">${it.big}</span>` : ''}
        <b>${esc(it.n)}</b>${it.d ? `<small>${esc(it.d)}</small>` : ''}`;
      if (it.css) c.style.backgroundImage = it.css;
      c.onclick = () => { set(it.v); Snd.click(); this.paint(); };
      g.appendChild(c);
    });
    area.appendChild(g);
  },

  /* ---------- шаги ---------- */
  STEPS:[
    { title:'Здравствуйте', sub:'Настроим систему под вас. Это займёт минуту.',
      fill(area){
        const hi = el('div', 'setup-hello');
        hi.innerHTML = `<img class="os-logo" src="assets/logo.png" alt="" draggable="false">
          <div class="setup-brand">${esc(Brand.name)}</div>
          <div class="setup-ver">${esc(Brand.versionLine())}</div>`;
        area.appendChild(hi);
      } },

    { title:'Язык системы', sub:'Его можно будет сменить в Параметрах в любой момент.',
      fill(area){
        this.cards(area, Object.entries(I18N.LANGS).map(([v, l]) => ({ v, n:l.name, big:l.code })),
          () => this.data.lang, v => this.data.lang = v);
      } },

    { title:'Раскладки клавиатуры', sub:'Переключение — Alt + Shift или щелчок по индикатору в панели.',
      fill(area){
        const g = el('div', 'setup-cards');
        Object.entries(Layouts.ALL).forEach(([id, d]) => {
          const on = this.data.layouts.includes(id);
          const c = el('button', 'setup-card' + (on ? ' on' : ''));
          c.innerHTML = `<span class="big">${d.flag}</span><b>${esc(d.name)}</b>
            <small>${on ? 'включена' : 'выключена'}</small>`;
          c.onclick = () => {
            const l = this.data.layouts;
            if (l.includes(id)){ if (l.length > 1) this.data.layouts = l.filter(x => x !== id); }
            else l.push(id);
            Snd.click(); this.paint();
          };
          g.appendChild(c);
        });
        area.appendChild(g);
      } },

    { title:'Ваш город', sub:'Нужен только для погоды. Данные берутся из открытого сервиса Open-Meteo.',
      fill(area){
        const i = el('input', 'inp setup-input');
        i.value = this.data.city; i.placeholder = 'Москва';
        i.oninput = () => this.data.city = i.value;
        area.appendChild(i);
      } },

    { title:'Как к вам обращаться', sub:'Имя видно в меню Пуск и на экране блокировки.',
      fill(area){
        const wrap = el('div', 'setup-name');
        const i = el('input', 'inp setup-input');
        i.value = this.data.name; i.placeholder = 'Имя пользователя'; i.maxLength = 24;
        const e2 = el('input', 'inp setup-emoji');
        e2.value = this.data.emoji; e2.placeholder = '🙂'; e2.maxLength = 2;
        i.oninput = () => { this.data.name = i.value; err.textContent = ''; };
        e2.oninput = () => this.data.emoji = e2.value;
        const err = el('div', 'setup-err', '');
        wrap.append(e2, i);
        area.append(wrap, err);
        setTimeout(() => i.focus(), 200);
        this._err = err;
      },
      check(){
        if (this.data.name.trim()) return true;
        if (this._err) this._err.textContent = 'Без имени дальше нельзя — так система будет к вам обращаться';
        return false;
      } },

    { title:'Пароль', sub:'Можно оставить пустым — тогда вход будет свободным.',
      fill(area){
        const i = el('input', 'inp setup-input'); i.type = 'password';
        i.placeholder = 'Пароль (не обязательно)'; i.value = this.data.pass;
        i.oninput = () => this.data.pass = i.value;
        area.appendChild(i);
        area.appendChild(el('div', 'setup-note',
          'Честно о том, что это даёт: пароль запирает экран блокировки внутри системы. ' +
          'Его хеш хранится на этой машине, сами файлы не шифруются. Это защита от чужого ' +
          'взгляда, а не от того, у кого есть доступ к компьютеру.'));
      } },

    { title:'Оформление', sub:'Тему и цвет тоже можно менять когда угодно.',
      fill(area){
        this.cards(area, [
          { v:'glass', n:'Прозрачная', d:'стекло и размытие', big:'◍' },
          { v:'dark',  n:'Тёмная',     d:'непрозрачные поверхности', big:'●' },
          { v:'light', n:'Светлая',    d:'светлые поверхности', big:'○' }
        ], () => this.data.theme, v => this.data.theme = v);

        const acc = el('div', 'setup-accents');
        ACCENTS.forEach((a, k) => {
          const b = el('button', 'setup-acc' + (this.data.accent === k ? ' on' : ''));
          b.style.background = `linear-gradient(135deg,${a.a},${a.b})`;
          b.title = a.n;
          b.onclick = () => { this.data.accent = k; Snd.click(); this.paint(); };
          acc.appendChild(b);
        });
        area.appendChild(acc);
      } },

    { title:'Обои', sub:'',
      fill(area){
        const g = el('div', 'setup-walls');
        WALLPAPERS.forEach(w => {
          const b = el('button', 'setup-wall' + (this.data.wallpaper === w.id ? ' on' : ''));
          b.style.backgroundImage = w.css; b.title = w.name;
          b.onclick = () => { this.data.wallpaper = w.id; Snd.click(); this.preview(); this.paint(); };
          g.appendChild(b);
        });
        area.appendChild(g);
      } },

    { title:'Что система о вас знает', sub:'Коротко и без мелкого шрифта.',
      fill(area){
        const list = [
          ['💾', 'Всё остаётся здесь', 'Настройки и файлы хранятся на этой машине. Никакой учётной записи и никакой отправки на сторону.'],
          ['🌤', 'Единственный запрос наружу', 'Погода: город уходит в открытый сервис Open-Meteo. Откажетесь — погода просто не покажется.'],
          ['🚫', 'Чего здесь нет', 'Ни сбора статистики, ни рекламного идентификатора, ни «улучшения качества продукта».'],
          ['🔍', 'Проверяется', 'Система открыта: весь код лежит рядом и читается глазами.']
        ];
        const box = el('div', 'setup-privacy');
        list.forEach(([e, t, d]) => {
          const r = el('div', 'setup-priv');
          r.innerHTML = `<span class="e">${e}</span><div><b>${t}</b><small>${d}</small></div>`;
          box.appendChild(r);
        });
        area.appendChild(box);
      } },

    { title:'Всё готово', sub:'Осталось нажать кнопку.',
      fill(area){
        const sum = el('div', 'setup-sum');
        const L = Object.entries(Layouts.ALL).filter(([id]) => this.data.layouts.includes(id))
          .map(([, d]) => d.name).join(', ');
        [['🗣', 'Язык', I18N.LANGS[this.data.lang].name],
         ['⌨️', 'Раскладки', L],
         ['🏙', 'Город', this.data.city || 'не указан'],
         ['👤', 'Пользователь', this.data.name],
         ['🔒', 'Вход', this.data.pass ? 'по паролю' : 'свободный'],
         ['🎨', 'Тема', { glass:'Прозрачная', dark:'Тёмная', light:'Светлая' }[this.data.theme]]
        ].forEach(([e, k, v]) => {
          const r = el('div', 'setup-sr');
          r.innerHTML = `<span class="e">${e}</span><span class="k">${k}</span><span class="v">${esc(String(v))}</span>`;
          sum.appendChild(r);
        });
        area.appendChild(sum);
      } }
  ],

  /* обои показываем сразу, чтобы выбор было видно */
  preview(){
    const w = WALLPAPERS.find(x => x.id === this.data.wallpaper);
    if (w) $('#wallpaper').style.backgroundImage = w.css;
  },

  /* ---------- применение ---------- */
  async done(){
    const d = this.data;

    Store.set('theme', d.theme);
    Store.set('accent', d.accent);
    Store.set('wallpaper', d.wallpaper);
    Store.set('city', d.city || 'Москва');
    Store.set('userName', d.name.trim());

    KV.set('kb.enabled', d.layouts);
    KV.set('kb.current', d.layouts.includes('en') ? 'en' : d.layouts[0]);

    const me = Profiles.current();
    if (me){
      const l = Profiles.list();
      const p = l.find(x => x.id === me.id);
      if (p){ p.name = d.name.trim(); p.emoji = d.emoji || d.name.trim()[0] || '🙂'; Profiles.save(l); }
      if (d.pass) await Profiles.setPassword(me.id, d.pass);
    }

    this.finish();
    const ov = $('#setup');
    ov.classList.remove('on');
    setTimeout(() => ov.remove(), 500);

    /* язык меняем последним: он перезагружает страницу */
    if (d.lang !== I18N.lang()){
      KV.set('welcome.pending', true);
      setTimeout(() => I18N.set(d.lang), 400);
      return;
    }
    Welcome.play();
  }
};
window.Setup = Setup;

/* ==========================================================================
   Приветствие: экран между настройкой и рабочим столом
   ========================================================================== */
const Welcome = {
  play(name){
    const who = name || (Profiles.current() || {}).name || S.userName || '';
    const ov = el('div', 'welcome');
    ov.innerHTML = `
      <div class="wl-ring"><span></span><span></span><span></span></div>
      <img class="os-logo wl-logo" src="assets/logo.png" alt="" draggable="false">
      <div class="wl-hi">Добро пожаловать</div>
      <div class="wl-who">${esc(who)}</div>
      <div class="wl-note">Готовим рабочий стол…</div>`;
    document.body.appendChild(ov);
    requestAnimationFrame(() => ov.classList.add('on'));

    Snd.blip(523, .18, 'sine', .04);
    setTimeout(() => Snd.blip(659, .18, 'sine', .035), 160);
    setTimeout(() => Snd.blip(784, .30, 'sine', .03), 330);

    setTimeout(() => {
      ov.classList.add('bye');
      setTimeout(() => ov.remove(), 900);
      $('#desktop').classList.add('on');
      Shell.lock(false);
      Shell.toast('Система', 'Настройка завершена — добро пожаловать', '✨', 4000);
    }, 2600);
  }
};
window.Welcome = Welcome;
