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
      'Выберите до четырёх. Переключение — Alt + Shift или щелчок по индикатору в панели.':'Pick up to four. Switch with Alt + Shift or by clicking the indicator in the taskbar.',
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
    const d = this.T[this.data.lang] || (window.I18N && I18N.МАСТЕР[this.data.lang]);
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

    /* Язык системы — списком: все языки, на которые переведён интерфейс.
       Сейчас их два; новый появится здесь сам, как только его добавят в
       словари I18N. */
    { title:'Язык системы', sub:'Его можно будет сменить в Параметрах в любой момент.',
      fill(area){
        const список = el('div', 'kb-список kb-языки');
        Object.entries(I18N.LANGS).forEach(([v, l]) => {
          const вкл = this.data.lang === v;
          const с = el('button', 'kb-строка' + (вкл ? ' on' : ''),
            `<span class="kb-флаг">${esc(l.code)}</span><b>${esc(l.name)}</b><small>${esc(l.locale)}</small><span class="kb-галка">${вкл ? '✓' : ''}</span>`);
          с.onclick = () => { this.data.lang = v; Snd.click(); this.paint(); };
          список.appendChild(с);
        });
        area.appendChild(список);
      } },

    /* Раскладки — все, что есть в системе (база XKB), с поиском. Список сам
       перерисовывает себя, не трогая шаг: поиск не теряет фокус. */
    { title:'Раскладки клавиатуры', sub:'Выберите до четырёх. Переключение — Alt + Shift или щелчок по индикатору в панели.',
      fill(area){
        Layouts.список(area, {
          получить:() => this.data.layouts,
          задать:l => { this.data.layouts = l; },
          /* Названия раскладок по-русски понятны и украинцу, и казаху;
             остальным — по-английски */
          англ:!['ru', 'uk', 'be', 'kk'].includes(this.data.lang)
        });
        /* Полный список раскладок знает система — подгружаем его и
           дорисовываем, когда придёт */
        if (window.Platform && Platform.mode === 'native' && !this._раскладкиСистемы){
          this._раскладкиСистемы = true;
          Platform.rpc('sys.kb.list').then(д => { Layouts.дополни(д['список']); this.paint(); }).catch(() => {});
        }
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

    /* Раскладки — и себе, и самой системе: оконный сервер получит их сразу
       или со следующего входа */
    KV.set('kb.current', d.layouts.includes('en') ? 'en' : d.layouts[0]);
    Layouts.применить(d.layouts);

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
  /* Приветствие после настройки — «Сборка вихря».

   Своя сцена, а не чужая: знак системы собирается из пыли.
     — на тёмном небе проявляются круговые оси — тонкие орбиты, по ним
       кружат тысячи мельчайших частиц;
     — каждая частица по спирали вдоль своей орбиты стягивается к своей
       точке логотипа и принимает её цвет — точки берутся из самой
       картинки, поэтому вихрь собирается буквально из пыли, оставляя
       светящиеся следы;
     — собранный вихрь вспыхивает, вокруг пролетает комета света;
     — появляется приветствие — только на языке, выбранном в настройке;
     — всё сжимается в точку, и из неё кругом раскрывается рабочий стол.
   Щелчок или любая клавиша — сразу к приветствию и финалу. На слабой
   машине частиц меньше и без следов; при просьбе о меньшем движении —
   без частиц вовсе: знак и слова просто проявляются. */

  /* Слова — на языке системы. Незнакомый язык получает английские. */
  ТЕКСТЫ:{
    ru:{ привет:'Привет', welcome:'Добро пожаловать в GlowerOS', skip:'Щелчок — пропустить', безИмени:'Добро пожаловать',
         система:'Система', готово:'Настройка завершена — добро пожаловать' },
    en:{ привет:'Hello', welcome:'Welcome to GlowerOS', skip:'Click to skip', безИмени:'Welcome',
         система:'System', готово:'Setup complete — welcome' },
    uk:{ привет:'Привіт', welcome:'Ласкаво просимо до GlowerOS', skip:'Клацніть, щоб пропустити', безИмени:'Ласкаво просимо',
         система:'Система', готово:'Налаштування завершено — ласкаво просимо' },
    be:{ привет:'Прывітанне', welcome:'Сардэчна запрашаем у GlowerOS', skip:'Пстрыкніце, каб прапусціць', безИмени:'Сардэчна запрашаем',
         система:'Сістэма', готово:'Наладка завершана — сардэчна запрашаем' },
    kk:{ привет:'Сәлем', welcome:'GlowerOS-қа қош келдіңіз', skip:'Өткізіп жіберу үшін басыңыз', безИмени:'Қош келдіңіз',
         система:'Жүйе', готово:'Баптау аяқталды — қош келдіңіз' },
    de:{ привет:'Hallo', welcome:'Willkommen bei GlowerOS', skip:'Klicken zum Überspringen', безИмени:'Willkommen',
         система:'System', готово:'Einrichtung abgeschlossen — willkommen' },
    fr:{ привет:'Bonjour', welcome:'Bienvenue dans GlowerOS', skip:'Cliquez pour passer', безИмени:'Bienvenue',
         система:'Système', готово:'Configuration terminée — bienvenue' },
    es:{ привет:'Hola', welcome:'Bienvenido a GlowerOS', skip:'Haz clic para omitir', безИмени:'Bienvenido',
         система:'Sistema', готово:'Configuración completada — bienvenido' },
    it:{ привет:'Ciao', welcome:'Benvenuto in GlowerOS', skip:'Fai clic per saltare', безИмени:'Benvenuto',
         система:'Sistema', готово:'Configurazione completata — benvenuto' },
    pt:{ привет:'Olá', welcome:'Bem-vindo ao GlowerOS', skip:'Clique para pular', безИмени:'Bem-vindo',
         система:'Sistema', готово:'Configuração concluída — bem-vindo' },
    pl:{ привет:'Cześć', welcome:'Witamy w GlowerOS', skip:'Kliknij, aby pominąć', безИмени:'Witamy',
         система:'System', готово:'Konfiguracja zakończona — witamy' },
    tr:{ привет:'Merhaba', welcome:'GlowerOS’a hoş geldiniz', skip:'Geçmek için tıklayın', безИмени:'Hoş geldiniz',
         система:'Sistem', готово:'Kurulum tamamlandı — hoş geldiniz' }
  },

  тексты(язык){ return this.ТЕКСТЫ[язык] || this.ТЕКСТЫ.en; },

  play(name){
    const who = name || (Profiles.current() || {}).name || S.userName || '';
    const язык = window.I18N ? I18N.lang() : 'ru';
    const T = this.тексты(язык);
    const тело = document.body.classList;
    const тихо = тело.contains('reduced');
    const слабо = тело.contains('эконом');

    const строка = who ? T.привет + ', ' + who : T.безИмени;
    const буквы = [...строка].map((б, i) =>
      `<span style="--i:${i}">${б === ' ' ? '&nbsp;' : esc(б)}</span>`).join('');

    const ov = el('div', 'welcome');
    ov.setAttribute('lang', язык);
    ov.innerHTML = `
      <canvas class="wl-холст"></canvas>
      <img class="os-logo wl-лого" src="assets/logo.png" alt="" draggable="false">
      <div class="wl-текст">
        <div class="wl-who">${буквы}</div>
        <div class="wl-hi">${esc(T.welcome)}</div>
      </div>
      <div class="wl-skip">${esc(T.skip)}</div>
      <div class="wl-волна"></div>`;
    document.body.appendChild(ov);
    requestAnimationFrame(() => ov.classList.add('on'));

    const сцена = тихо ? null : this.сцена(ov.querySelector('.wl-холст'), { слабо });
    const таймеры = [];
    const потом = (мс, f) => таймеры.push(setTimeout(f, мс));
    let этап = 'сборка';

    /* Вихрь собран: чёткий знак проявляется сквозь пыль, вспышка, комета */
    const собран = () => {
      if (этап !== 'сборка') return;
      этап = 'собран';
      ov.classList.add('собран');
      if (сцена) сцена.вспышка();
      Snd.blip(523, .18, 'sine', .04);
      setTimeout(() => Snd.blip(784, .28, 'sine', .03), 140);
      потом(450, слова);
    };
    /* Приветствие — по буквам */
    const слова = () => {
      if (этап === 'слова' || этап === 'финал') return;
      этап = 'слова';
      ov.classList.add('собран', 'слова');
      setTimeout(() => Snd.blip(1046, .30, 'sine', .022), 300);
      потом(тихо ? 1600 : 2300, финал);
    };
    /* Финал: всё сжимается к центру, и кругом раскрывается стол */
    const финал = () => {
      if (этап === 'финал') return;
      этап = 'финал';
      таймеры.forEach(clearTimeout);
      ov.classList.add('сжатие');
      if (сцена) сцена.сожми();
      setTimeout(() => {
        ov.classList.add('раскрытие');
        document.body.classList.add('стол-въезд');
        $('#desktop').classList.add('on');
        setTimeout(() => {
          ov.classList.add('bye');
          setTimeout(() => { if (сцена) сцена.стоп(); ov.remove(); }, 600);
        }, 800);
        setTimeout(() => document.body.classList.remove('стол-въезд'), 2000);
        Shell.toast(T.система, T.готово, '✨', 4000);
      }, 380);
      removeEventListener('keydown', пропуск, true);
    };

    /* Пропустить: щелчок или клавиша — сразу к словам, повторно — к финалу */
    const пропуск = e => {
      if (e.type === 'keydown'){ e.preventDefault(); e.stopPropagation(); }
      if (этап === 'сборка' || этап === 'собран'){
        таймеры.forEach(clearTimeout);
        if (сцена) сцена.дособери();
        ov.classList.add('собран');
        слова();
      } else финал();
    };
    ov.addEventListener('click', пропуск);
    addEventListener('keydown', пропуск, true);

    if (сцена) сцена.собрано.then(собран);
    else потом(500, собран);
  },

  /* Сцена на холсте: круговые оси, частицы, следы, вспышка и комета.

     Точки логотипа берём из самой картинки. Если картинку прочитать нельзя
     (страница открыта файлом — браузер тогда не даёт читать пиксели),
     вихрь рисуется по формуле: пять закрученных рукавов, как у знака. */
  сцена(холст, { слабо }){
    const к = холст.getContext('2d');
    const d = Math.min(2, devicePixelRatio || 1);
    let w = 0, h = 0, cx = 0, cy = 0;
    const размер = () => {
      w = холст.clientWidth; h = холст.clientHeight;
      холст.width = w * d; холст.height = h * d; к.setTransform(d, 0, 0, d, 0, 0);
      cx = w / 2; cy = h * 0.42;
    };
    размер();
    const ЛОГО = 180;                                   // как .wl-лого в CSS
    const N = слабо ? 900 : 2600;
    const частицы = [];
    let готово;
    const собрано = new Promise(r => готово = r);

    const цели = cb => {
      const img = new Image();
      img.onload = () => {
        try {
          const c = document.createElement('canvas'); c.width = c.height = 120;
          const cc = c.getContext('2d'); cc.drawImage(img, 0, 0, 120, 120);
          const px = cc.getImageData(0, 0, 120, 120).data, точки = [];
          for (let y = 0; y < 120; y++) for (let x = 0; x < 120; x++){
            const i = (y * 120 + x) * 4;
            if (px[i + 3] > 140 && px[i] + px[i + 1] + px[i + 2] > 120)
              точки.push([(x / 120 - 0.5) * ЛОГО, (y / 120 - 0.5) * ЛОГО, px[i], px[i + 1], px[i + 2]]);
          }
          if (точки.length > 200) return cb(точки);
        } catch(e){ /* холст «испорчен» чужой картинкой — рисуем по формуле */ }
        cb(null);
      };
      img.onerror = () => cb(null);
      img.src = 'assets/logo.png';
    };
    const поФормуле = () => {
      const т = [];
      for (let i = 0; i < 4000; i++){
        const рукав = i % 5, u = Math.random();
        const r = 8 + u * (ЛОГО * 0.47);
        const a = рукав * (2 * Math.PI / 5) + u * 2.4 + (Math.random() - 0.5) * 0.35 * (1 - u * 0.5);
        const синий = u > 0.5;
        т.push([Math.cos(a) * r, Math.sin(a) * r, синий ? 70 : 150, синий ? 130 : 90, 255]);
      }
      return т;
    };

    const начало = performance.now();
    let вспышкаС = 0, сжатиеС = 0, стоим = false, дособрать = false;
    const плавно = p => p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2;

    цели(точки => {
      const т = точки || поФормуле();
      for (let i = 0; i < N; i++){
        const [tx, ty, R, G, B] = т[(Math.random() * т.length) | 0];
        const rt = Math.hypot(tx, ty), θt = Math.atan2(ty, tx);
        /* Орбита старта: одна из «осей» — колец, на которых частицы кружат
           до того, как пойти к своей точке */
        const ось = [150, 230, 320, 420, 540][(Math.random() * 5) | 0] + (Math.random() - 0.5) * 26;
        частицы.push({ rt, θt, r0:Math.max(ось, rt + 60), θ0:Math.random() * 6.283,
          витков:1 + Math.random() * 1.2, з:0.25 + Math.random() * 0.9 + (1 - rt / ЛОГО) * 0.3,
          д:1.5 + Math.random() * 0.6, ω:(0.25 + Math.random() * 0.35) * (Math.random() < 0.85 ? 1 : -1),
          ц:`${Math.min(255, R + 40)},${Math.min(255, G + 40)},${Math.min(255, B + 20)}`, р:0.7 + Math.random() * 1.1 });
      }
    });

    const кадр = now => {
      if (стоим || !холст.isConnected) return;
      const t = (now - начало) / 1000;
      /* Следы: прежний кадр не стираем, а гасим — частицы тянут хвосты */
      if (слабо) к.clearRect(0, 0, w, h);
      else {
        к.globalCompositeOperation = 'destination-out';
        к.fillStyle = 'rgba(0,0,0,0.22)';
        к.fillRect(0, 0, w, h);
      }
      к.globalCompositeOperation = 'lighter';

      /* Круговые оси: тонкие кольца, по ним бегут светлые дуги */
      const оси = Math.min(1, t / 0.8) * (сжатиеС ? Math.max(0, 1 - (now - сжатиеС) / 350) : 1)
                * (вспышкаС ? Math.max(0.25, 1 - (now - вспышкаС) / 1500) : 1);
      if (оси > 0.01) [150, 230, 320, 420, 540].forEach((R, i) => {
        к.strokeStyle = `rgba(140,160,255,${(0.035 * оси).toFixed(3)})`;
        к.lineWidth = 1;
        к.beginPath(); к.arc(cx, cy, R, 0, 6.283); к.stroke();
        const a = t * (0.5 - i * 0.07) * (i % 2 ? -1 : 1) + i;
        к.strokeStyle = `rgba(170,190,255,${(0.10 * оси).toFixed(3)})`;
        к.lineWidth = 1.5;
        к.beginPath(); к.arc(cx, cy, R, a, a + 0.7); к.stroke();
      });

      /* Частицы: кружат на своей оси, потом по спирали — к своей точке */
      let дошли = 0;
      const сжатие = сжатиеС ? Math.min(1, (now - сжатиеС) / 380) : 0;
      for (const ч of частицы){
        let p = дособрать ? 1 : Math.max(0, Math.min(1, (t - ч.з) / ч.д));
        const e = плавно(p);
        let r, θ;
        if (p <= 0){ r = ч.r0; θ = ч.θ0 + t * ч.ω; }
        else {
          const старт = ч.θ0 + ч.з * ч.ω;
          r = ч.r0 + (ч.rt - ч.r0) * e;
          θ = ч.θt + (старт - ч.θt + ч.витков * 6.283) * (1 - e);
        }
        if (p >= 1) дошли++;
        if (сжатие) r *= 1 - сжатие * сжатие;
        const x = cx + Math.cos(θ) * r, y = cy + Math.sin(θ) * r;
        const а = Math.min(1, t * 1.5) * (p >= 1 ? (вспышкаС ? Math.max(0.15, 1 - (now - вспышкаС) / 900) : 0.95) : 0.8);
        к.fillStyle = `rgba(${ч.ц},${а.toFixed(3)})`;
        к.fillRect(x - ч.р / 2, y - ч.р / 2, ч.р, ч.р);
      }
      if (частицы.length && дошли >= частицы.length * 0.97 && готово){ const г = готово; готово = null; г(); }

      /* Вспышка: кольцо света расходится, по орбите пролетает комета */
      if (вспышкаС){
        const ф = (now - вспышкаС) / 1000;
        if (ф < 0.9){
          /* Тонкое и прозрачное: следы кадров превращают его в лёгкую
             рябь, а толстое кольцо оставляло стопку серых колец */
          const R = ЛОГО * 0.5 + ф * 420, а = (1 - ф / 0.9) * 0.28;
          const г = к.createRadialGradient(cx, cy, R * 0.94, cx, cy, R);
          г.addColorStop(0, 'rgba(120,150,255,0)'); г.addColorStop(0.7, `rgba(180,200,255,${а})`); г.addColorStop(1, 'rgba(120,150,255,0)');
          к.fillStyle = г; к.beginPath(); к.arc(cx, cy, R, 0, 6.283); к.fill();
        }
        if (ф < 1.4){
          const а = -Math.PI / 2 + ф * 4.6, R = ЛОГО * 0.66;
          for (let j = 0; j < 14; j++){
            const аа = а - j * 0.05, x = cx + Math.cos(аа) * R, y = cy + Math.sin(аа) * R;
            const сила = (1 - j / 14) * Math.min(1, (1.4 - ф) * 2);
            const г = к.createRadialGradient(x, y, 0, x, y, 9 - j * 0.4);
            г.addColorStop(0, `rgba(255,255,255,${сила.toFixed(3)})`); г.addColorStop(1, 'rgba(130,150,255,0)');
            к.fillStyle = г; к.beginPath(); к.arc(x, y, 9 - j * 0.4, 0, 6.283); к.fill();
          }
        }
      }
      к.globalCompositeOperation = 'source-over';
      requestAnimationFrame(кадр);
    };
    requestAnimationFrame(кадр);
    addEventListener('resize', размер);

    /* На случай, если машина не успевает: собрано не позже чем через 4 с */
    setTimeout(() => { if (готово){ const г = готово; готово = null; г(); } }, 4000);

    return {
      собрано,
      вспышка(){ вспышкаС = performance.now(); },
      дособери(){ дособрать = true; if (!вспышкаС) вспышкаС = performance.now(); },
      сожми(){ сжатиеС = performance.now(); },
      стоп(){ стоим = true; removeEventListener('resize', размер); }
    };
  }
};
window.Welcome = Welcome;
