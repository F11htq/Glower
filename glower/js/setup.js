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
      'не указан':'not set',
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
         theme:'dark', accent:0, wallpaper:'gora' },

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

    /* Шага с паролем здесь больше нет. Он появился ради экрана блокировки
       оболочки, которого мы лишились, и с тех пор спрашивал пароль ни для
       чего: экран машины отпирается паролем учётной записи, заданным при
       установке, а профильный нужен лишь тому, кто заведёт второй профиль —
       и задаётся там же, в Параметрах. Спрашивать при первом запуске пароль,
       который человеку негде применить, — значит отнимать время и обещать
       защиту, которой нет. */

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
  /* Приветствие после настройки — «Привет» на языках мира, как у macOS.

   Человек только что закончил настройку: это момент, когда система впервые
   становится его. Поэтому здесь не заставка, а маленькая сцена:
     — на тёмном небе медленно течёт сияние в цветах системы, вверх плывут
       пылинки;
     — первым словом — «Привет» на языке системы: его выписывает «перо
       света» — сначала контуры букв, потом заливка, а по краю письма бежит
       искра;
     — дальше каскад приветствий, всё быстрее: каждое слово рассыпается
       искрами и уступает место следующему;
     — последним крупно выписывается имя и «Добро пожаловать в GlowerOS»;
     — финал: от центра раскрывается круг, по краю идёт волна света, за ней
       рабочий стол.
   Щелчок или любая клавиша — сразу к финалу.

   Языки — только те, чьё письмо есть в шрифтах системы (Noto): слово из
   квадратиков хуже, чем его отсутствие. Китайский, японский и корейский
   потребовали бы шрифта на десятки мегабайт. */
  ЯЗЫКИ:[
    ['Bonjour', 'fr'], ['Hola', 'es'], ['Hallo', 'de'], ['Ciao', 'it'], ['Olá', 'pt'],
    ['Merhaba', 'tr'], ['Γειά σου', 'el'], ['नमस्ते', 'hi'], ['مرحبا', 'ar']
  ],

  play(name){
    const who = name || (Profiles.current() || {}).name || S.userName || '';
    const en = (window.I18N ? I18N.lang() : 'ru') === 'en';
    const T = en ? { hi:'Hello', welcome:'Welcome to GlowerOS', skip:'Click to skip' }
                 : { hi:'Привет', welcome:'Добро пожаловать в GlowerOS', skip:'Щелчок — пропустить' };
    const тихо = document.body.classList.contains('эконом') || document.body.classList.contains('reduced');

    /* Слова по порядку: язык системы первым, второй язык интерфейса
       следом, дальше — мир. Длительности тают: каскад разгоняется. */
    const слова = [[T.hi, en ? 'en' : 'ru'], [en ? 'Привет' : 'Hello', en ? 'ru' : 'en'], ...this.ЯЗЫКИ];
    const длит = тихо ? слова.slice(0, 2).map(() => 900)
                      : слова.map((_, i) => i === 0 ? 1500 : Math.max(380, 820 - i * 55));
    const показать = тихо ? слова.slice(0, 2) : слова;

    const ov = el('div', 'welcome');
    ov.innerHTML = `
      <div class="wl-сияние"><i></i><i></i><i></i></div>
      <canvas class="wl-пыль"></canvas>
      <div class="wl-слово" aria-hidden="true"></div>
      <div class="wl-итог">
        <div class="wl-who">${esc(who || (en ? 'Welcome' : 'Добро пожаловать'))}</div>
        <div class="wl-hi">${esc(T.welcome)}</div>
      </div>
      <div class="wl-skip">${esc(T.skip)}</div>
      <div class="wl-волна"></div>`;
    document.body.appendChild(ov);
    requestAnimationFrame(() => ov.classList.add('on'));

    const пыль = тихо ? null : this.пыль(ov.querySelector('.wl-пыль'));
    const место = ov.querySelector('.wl-слово');

    /* Одно слово «пером света»: SVG-текст, у которого сначала рисуется
       обводка, потом проявляется заливка; маска открывает его слева
       направо (справа налево — для арабского), а по краю бежит искра. */
    const напиши = (текст, язык, мс, крупно) => {
      const ртл = язык === 'ar' || язык === 'he';
      const н = el('div', 'wl-перо' + (ртл ? ' ртл' : '') + (крупно ? ' крупно' : ''));
      н.style.setProperty('--мс', мс + 'ms');
      н.setAttribute('lang', язык);
      н.innerHTML = `<svg viewBox="0 0 1000 220" preserveAspectRatio="xMidYMid meet">
          <defs><linearGradient id="wl-г" x1="0" x2="1" y1="0" y2="0">
            <stop offset="0" stop-color="#9cc2ff"/><stop offset=".5" stop-color="#ffffff"/><stop offset="1" stop-color="#c9a6ff"/>
          </linearGradient></defs>
          <text x="500" y="150" text-anchor="middle">${esc(текст)}</text>
        </svg><span class="wl-искра"></span>`;
      место.appendChild(н);
      requestAnimationFrame(() => н.classList.add('пишу'));
      return н;
    };

    let шаг = 0, таймер = null, конец = false;
    const следующее = () => {
      if (конец) return;
      const прежнее = место.querySelector('.wl-перо:not(.ушло)');
      if (прежнее){
        прежнее.classList.add('ушло');
        if (пыль) пыль.искры(прежнее);
        setTimeout(() => прежнее.remove(), 700);
      }
      if (шаг >= показать.length) return итог();
      const [текст, язык] = показать[шаг];
      /* Новое слово начинаем писать чуть позже, чем ушло прежнее */
      const н = шаг, мс = Math.min(1100, длит[шаг] * 0.8);
      setTimeout(() => { if (!конец) напиши(текст, язык, мс, н === 0); }, прежнее ? 120 : 0);
      таймер = setTimeout(следующее, длит[шаг]);
      шаг++;
    };

    /* Имя — крупно и тем же пером; под ним приветствие системы */
    const итог = () => {
      if (конец) return;
      конец = true;
      clearTimeout(таймер);
      место.querySelectorAll('.wl-перо').forEach(н => н.classList.add('ушло'));
      ov.classList.add('имя');
      Snd.blip(523, .18, 'sine', .04);
      setTimeout(() => Snd.blip(659, .18, 'sine', .035), 160);
      setTimeout(() => Snd.blip(784, .30, 'sine', .03), 330);
      setTimeout(финал, тихо ? 1200 : 2100);
    };

    /* Финал: круг раскрывается от центра, по краю — волна света */
    const финал = () => {
      ov.classList.add('раскрытие');
      Snd.blip(1046, .35, 'sine', .028);
      document.body.classList.add('стол-въезд');
      $('#desktop').classList.add('on');
      setTimeout(() => {
        ov.classList.add('bye');
        setTimeout(() => { if (пыль) пыль.стоп(); ov.remove(); }, 600);
      }, 800);
      setTimeout(() => document.body.classList.remove('стол-въезд'), 2000);
      Shell.toast(en ? 'System' : 'Система',
        en ? 'Setup complete — welcome' : 'Настройка завершена — добро пожаловать', '✨', 4000);
      removeEventListener('keydown', пропуск, true);
    };

    /* Пропустить: щелчок или клавиша — сразу к имени и финалу */
    const пропуск = e => {
      if (e.type === 'keydown'){ e.preventDefault(); e.stopPropagation(); }
      if (!конец) итог();
    };
    ov.addEventListener('click', пропуск);
    addEventListener('keydown', пропуск, true);

    setTimeout(следующее, 450);
  },

  /* Пылинки и искры — на одном холсте. Пылинки плывут вверх и мерцают;
     искры вылетают из уходящего слова и гаснут. Холст сам себя убирает,
     когда приветствие закончилось. */
  пыль(холст){
    const к = холст.getContext('2d');
    if (!к) return null;
    let w = 0, h = 0, стоим = false;
    const размер = () => {
      const d = Math.min(2, devicePixelRatio || 1);
      w = холст.clientWidth; h = холст.clientHeight;
      холст.width = w * d; холст.height = h * d; к.setTransform(d, 0, 0, d, 0, 0);
    };
    размер();
    const пылинки = Array.from({ length:70 }, () => ({
      x:Math.random() * w, y:Math.random() * h, r:0.6 + Math.random() * 1.6,
      v:6 + Math.random() * 18, ф:Math.random() * 6.28 }));
    const искры = [];
    let было = performance.now();
    const кадр = now => {
      if (стоим || !холст.isConnected) return;
      const dt = Math.min(0.05, (now - было) / 1000); было = now;
      к.clearRect(0, 0, w, h);
      к.globalCompositeOperation = 'lighter';
      for (const п of пылинки){
        п.y -= п.v * dt; п.x += Math.sin(now / 1400 + п.ф) * 6 * dt;
        if (п.y < -4){ п.y = h + 4; п.x = Math.random() * w; }
        const а = 0.25 + 0.35 * (0.5 + 0.5 * Math.sin(now / 600 + п.ф));
        к.fillStyle = `rgba(190,210,255,${а.toFixed(3)})`;
        к.beginPath(); к.arc(п.x, п.y, п.r, 0, 6.283); к.fill();
      }
      for (let i = искры.length - 1; i >= 0; i--){
        const и = искры[i];
        и.ж -= dt; if (и.ж <= 0){ искры.splice(i, 1); continue; }
        и.vy += 40 * dt; и.x += и.vx * dt; и.y += и.vy * dt;
        const а = Math.max(0, и.ж / и.ж0);
        const г = к.createRadialGradient(и.x, и.y, 0, и.x, и.y, и.r * 3);
        г.addColorStop(0, `rgba(255,255,255,${а})`);
        г.addColorStop(0.4, `rgba(${и.ц},${(а * 0.6).toFixed(3)})`);
        г.addColorStop(1, 'rgba(0,0,0,0)');
        к.fillStyle = г; к.beginPath(); к.arc(и.x, и.y, и.r * 3, 0, 6.283); к.fill();
      }
      к.globalCompositeOperation = 'source-over';
      requestAnimationFrame(кадр);
    };
    requestAnimationFrame(кадр);
    addEventListener('resize', размер);
    return {
      искры(узел){
        const р = узел.getBoundingClientRect(), о = холст.getBoundingClientRect();
        for (let i = 0; i < 26; i++){
          const у = Math.random() * 6.283, с = 60 + Math.random() * 160;
          искры.push({ x:р.left - о.left + р.width * (0.2 + Math.random() * 0.6),
                       y:р.top - о.top + р.height * (0.35 + Math.random() * 0.3),
                       vx:Math.cos(у) * с, vy:Math.sin(у) * с - 40, r:1.2 + Math.random() * 2,
                       ж0:0.6 + Math.random() * 0.5, ж:0, ц:Math.random() < 0.5 ? '120,160,255' : '190,140,255' });
          искры[искры.length - 1].ж = искры[искры.length - 1].ж0;
        }
      },
      стоп(){ стоим = true; removeEventListener('resize', размер); }
    };
  }
};
window.Welcome = Welcome;
