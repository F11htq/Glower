/* ==========================================================================
   Первый запуск: начальная настройка и приветствие

   При первом включении: язык, регион, имя, оформление,
   вход. Разница в одном — здесь не спрашивают согласия на то, чего
   система не делает: ни учётной записи в облаке, ни сбора данных,
   ни рекламного идентификатора. Всё, что вы выберете, останется на
   этой машине.
   ========================================================================== */
'use strict';

const Setup = {
  KEY:'setup.done',

  /* Мастер говорит на выбранном языке сразу, а не после перезагрузки:
     свои строки он держит при себе, поэтому переключение видно на месте. */
  T:{
    en:{
      'Здравствуйте':'Welcome',
      'Настроим систему под вас. Это займёт минуту.':'Let\u2019s set the system up for you. It takes a minute.',
      'Язык системы':'System language',
      'Его можно будет сменить в Параметрах в любой момент.':'You can change it in Settings at any time.',
      'Раскладки клавиатуры':'Keyboard layouts',
      'Переключение — Alt + Shift или щелчок по индикатору в панели.':'Switch with Alt + Shift or by clicking the indicator in the taskbar.',
      'Ваш город':'Your city',
      'Нужен только для погоды. Данные берутся из открытого сервиса Open-Meteo.':'Used only for weather, from the open Open-Meteo service.',
      'Как к вам обращаться':'What should we call you',
      'Имя видно в меню Пуск и при смене пользователя.':'The name appears in Start and when switching users.',
      'Имя пользователя':'User name',
      'Без имени дальше нельзя — так система будет к вам обращаться':'A name is required — this is how the system will address you',
      'Пароль':'Password',
      'Можно оставить пустым — тогда профиль не будет ничего спрашивать.':'Leave it empty and the profile will not ask for anything.',
      'Пароль (не обязательно)':'Password (optional)',
      'Честно о том, что это даёт: этот пароль разделяет профили внутри оболочки. Его хеш хранится на этой машине, сами файлы не шифруются. Запертый экран машины отпирается паролем учётной записи, а не этим.':'Honestly about what this gives you: this password separates profiles inside the shell. Its hash is stored on this machine and the files themselves are not encrypted. A locked screen is unlocked with the account password, not with this one.',
      'Оформление':'Appearance',
      'Тему и цвет тоже можно менять когда угодно.':'Theme and colour can be changed at any time too.',
      'Тёмная':'Dark', 'Светлая':'Light',
      'непрозрачные поверхности':'opaque surfaces',
      'светлые поверхности':'light surfaces',
      'Обои':'Wallpaper',
      'Что система о вас знает':'What the system knows about you',
      'Коротко и без мелкого шрифта.':'Briefly, and without fine print.',
      'Всё остаётся здесь':'Everything stays here',
      'Настройки и файлы хранятся на этой машине. Никакой учётной записи и никакой отправки на сторону.':'Settings and files are kept on this machine. No account, nothing sent anywhere.',
      'Единственный запрос наружу':'The only outside request',
      'Погода: город уходит в открытый сервис Open-Meteo. Откажетесь — погода просто не покажется.':'Weather: the city name goes to the open Open-Meteo service. Decline and the weather simply will not show.',
      'Чего здесь нет':'What is not here',
      'Ни сбора статистики, ни рекламного идентификатора, ни «улучшения качества продукта».':'No analytics, no advertising identifier, no \u201cproduct improvement programme\u201d.',
      'Проверяется':'Verifiable',
      'Система открыта: весь код лежит рядом и читается глазами.':'The system is open: all the code sits next to it and can be read.',
      'Всё готово':'All set',
      'Осталось нажать кнопку.':'One button left.',
      'Язык':'Language', 'Раскладки':'Layouts', 'Город':'City',
      'Пользователь':'User', 'Вход':'Sign-in', 'Тема':'Theme',
      'по паролю':'password', 'свободный':'free', 'не указан':'not set',
      'включена':'enabled', 'выключена':'disabled',
      'Назад':'Back', 'Далее':'Next', 'Начать работу':'Get started',
      'Русская':'Russian', 'Английская':'English', 'Немецкая':'German',
      'Добро пожаловать':'Welcome', 'Готовим рабочий стол…':'Preparing your desktop\u2026'
    }
  },

  t(s){
    const d = this.T[this.data.lang];
    return (d && d[s]) || s;
  },

  needed(){ return !KV.get(this.KEY, false); },
  finish(){ KV.set(this.KEY, true); },

  /* собранные ответы */
  data:{ lang:'ru', layouts:['en', 'ru'], city:'Москва', name:'', emoji:'',
         pass:'', theme:'dark', accent:0, wallpaper:'gora' },

  /* ---------- каркас ---------- */
  open(){
    const ov = el('div', 'setup');
    ov.id = 'setup';
    ov.innerHTML = `
      <div class="setup-bg"></div>
      <div class="setup-box glass">
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

    $('#setup-back', ov).onclick = () => this.назад();
    $('#setup-next', ov).onclick = () => this.вперёд();

    /* Настройка должна проходиться и без мыши: Enter ведёт вперёд, Backspace
       возвращает назад. Без этого система непроходима для того, у кого мышь
       не работает или её просто нет. */
    this._клавиши = e => {
      if (!document.getElementById('setup')) return;
      const поле = document.activeElement;
      const вводит = поле && /^(INPUT|TEXTAREA|SELECT)$/.test(поле.tagName);
      if (e.key === 'Enter'){ e.preventDefault(); e.stopPropagation(); this.вперёд(); }
      else if (e.key === 'Backspace' && !вводит){ e.preventDefault(); this.назад(); }
    };
    document.addEventListener('keydown', this._клавиши, true);
  },

  вперёд(){
    const s = this.STEPS[this.i];
    if (s.check && !s.check.call(this)) return;
    if (this.i < this.STEPS.length - 1){ this.i++; this.paint(); Snd.click(); }
    else this.done();
  },

  назад(){ if (this.i > 0){ this.i--; this.paint(); } },

  paint(){
    const box = $('#setup-body'), steps = $('#setup-steps');
    const s = this.STEPS[this.i];
    steps.innerHTML = this.STEPS.map((x, k) =>
      `<i class="${k === this.i ? 'on' : k < this.i ? 'was' : ''}"></i>`).join('');
    box.innerHTML = `<div class="setup-t">${this.t(s.title)}</div>
      ${s.sub ? `<div class="setup-s">${this.t(s.sub)}</div>` : ''}`;
    const area = el('div', 'setup-area');
    box.appendChild(area);
    s.fill.call(this, area);
    $('#setup-back').style.visibility = this.i ? 'visible' : 'hidden';
    $('#setup-back').textContent = this.t('Назад');
    $('#setup-next').textContent = this.t(this.i === this.STEPS.length - 1 ? 'Начать работу' : 'Далее');
    box.style.animation = 'none';
    requestAnimationFrame(() => box.style.animation = '');
  },

  /* ---------- вспомогательное ---------- */
  cards(area, items, get, set){
    const g = el('div', 'setup-cards');
    items.forEach(it => {
      const c = el('button', 'setup-card' + (get() === it.v ? ' on' : ''));
      c.innerHTML = `${it.big ? `<span class="big">${it.big}</span>` : ''}
        <b>${esc(this.t(it.n))}</b>${it.d ? `<small>${esc(this.t(it.d))}</small>` : ''}`;
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
          c.innerHTML = `<span class="big">${d.flag}</span><b>${esc(this.t(d.name))}</b>
            <small>${this.t(on ? 'включена' : 'выключена')}</small>`;
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
        i.value = this.data.city; i.placeholder = this.data.lang === 'en' ? 'Moscow' : 'Москва';
        i.oninput = () => this.data.city = i.value;
        area.appendChild(i);
      } },

    { title:'Как к вам обращаться', sub:'Имя видно в меню Пуск и при смене пользователя.',
      fill(area){
        const wrap = el('div', 'setup-name');
        const i = el('input', 'inp setup-input');
        i.value = this.data.name; i.placeholder = this.t('Имя пользователя'); i.maxLength = 24;
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
        if (this._err) this._err.textContent = this.t('Без имени дальше нельзя — так система будет к вам обращаться');
        return false;
      } },

    { title:'Пароль', sub:'Можно оставить пустым — тогда профиль не будет ничего спрашивать.',
      fill(area){
        const i = el('input', 'inp setup-input'); i.type = 'password';
        i.placeholder = this.t('Пароль (не обязательно)'); i.value = this.data.pass;
        i.oninput = () => this.data.pass = i.value;
        area.appendChild(i);
        area.appendChild(el('div', 'setup-note', this.t(
          'Честно о том, что это даёт: этот пароль разделяет профили внутри оболочки. ' +
          'Его хеш хранится на этой машине, сами файлы не шифруются. Запертый экран ' +
          'машины отпирается паролем учётной записи, а не этим.')));
      } },

    { title:'Оформление', sub:'Тему и цвет тоже можно менять когда угодно.',
      fill(area){
        this.cards(area, [
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
          r.innerHTML = `<span class="e">${e}</span><div><b>${this.t(t)}</b><small>${this.t(d)}</small></div>`;
          box.appendChild(r);
        });
        area.appendChild(box);
      } },

    { title:'Всё готово', sub:'Осталось нажать кнопку.',
      fill(area){
        const sum = el('div', 'setup-sum');
        const L = Object.entries(Layouts.ALL).filter(([id]) => this.data.layouts.includes(id))
          .map(([, d]) => this.t(d.name)).join(', ');
        [['🗣', 'Язык', I18N.LANGS[this.data.lang].name],
         ['⌨️', 'Раскладки', L],
         ['🏙', 'Город', this.data.city || this.t('не указан')],
         ['👤', 'Пользователь', this.data.name],
         ['🔒', 'Вход', this.t(this.data.pass ? 'по паролю' : 'свободный')],
         ['🎨', 'Тема', this.t({ dark:'Тёмная', light:'Светлая' }[this.data.theme] || 'Тёмная')]
        ].forEach(([e, k, v]) => {
          const r = el('div', 'setup-sr');
          r.innerHTML = `<span class="e">${e}</span><span class="k">${this.t(k)}</span><span class="v">${esc(String(v))}</span>`;
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
    if (this._клавиши){ document.removeEventListener('keydown', this._клавиши, true); this._клавиши = null; }
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
    /* имя выбрано только что — кнопка в Пуске должна показать его сразу,
       а не после перезапуска системы */

    if (Profiles.renderChip) Profiles.renderChip();
    Shell.renderShell();

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
    const T = { hi:'Добро пожаловать', note:'Готовим рабочий стол…' };
    if ((window.I18N ? I18N.lang() : 'ru') === 'en'){ T.hi = 'Welcome'; T.note = 'Preparing your desktop…'; }
    const ov = el('div', 'welcome');
    ov.innerHTML = `
      <div class="wl-ring"><span></span><span></span><span></span></div>
      <img class="os-logo wl-logo" src="assets/logo.png" alt="" draggable="false">
      <div class="wl-hi">${T.hi}</div>
      <div class="wl-who">${esc(who)}</div>
      <div class="wl-note">${T.note}</div>`;
    document.body.appendChild(ov);
    requestAnimationFrame(() => ov.classList.add('on'));

    Snd.blip(523, .18, 'sine', .04);
    setTimeout(() => Snd.blip(659, .18, 'sine', .035), 160);
    setTimeout(() => Snd.blip(784, .30, 'sine', .03), 330);

    setTimeout(() => {
      ov.classList.add('bye');
      setTimeout(() => ov.remove(), 900);
      $('#desktop').classList.add('on');

      Shell.toast('Система', 'Настройка завершена — добро пожаловать', '✨', 4000);
    }, 2600);
  }
};
window.Welcome = Welcome;
