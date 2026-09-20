/* ==========================================================================
   Магазин с настоящей установкой приложений
   Каталог, установка/удаление, свои приложения из .json-пакета
   ========================================================================== */
'use strict';

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
    /* Раньше здесь поднимались ещё и приложения из нашего каталога —
       помидорный таймер, 2048, конвертер и палитра. Каталога больше нет:
       Магазин занят настоящими программами Linux, а не нашими игрушками.
       Те, кто успел их поставить, найдут их на месте только до обновления —
       и это правильно: держать в системе четыре примера ради примеров
       значит выдавать их за то, чем система полезна. */
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
/* ==========================================================================
   МАГАЗИН

   Переделан целиком. Раньше первую половину занимал наш собственный
   каталог: помидорный таймер, 2048, конвертер единиц и палитра. Четыре
   примера, написанные, чтобы показать, что приложения бывают, — и они же
   занимали место настоящих программ, ради которых Магазин и открывают.
   Каталога больше нет.

   Вид взят у тех, кто делает это хорошо: витрина и плитки разделов — как
   в Microsoft Store, страница программы с крупным значком и одной
   понятной кнопкой — как в GNOME Software (её видно в Fedora). Оттуда же
   привычка не заставлять человека ждать молча: пока система ходит в
   репозитории, на месте будущих карточек лежат серые заготовки, а не
   пустота.

   Что здесь честно, а что нет. Подборки по разделам — наши, составлены
   руками: у apt нет ни витрин, ни «выбора редакции». Зато всё остальное
   настоящее: описания, размеры, версии и сама установка идут от машины, и
   если программы в репозиториях нет, Магазин так и скажет.
   ========================================================================== */
APPS.store = {
  name:'Магазин', glyph:'🛍️', bg:'linear-gradient(140deg,#c4b5fd,#7c3aed)',
  w:980, h:680, single:true,

  render(win, opts){
    const wrap = el('div', 'app st-app'); win.body.appendChild(wrap);
    const side = el('div', 'sidebar st-side');
    const main = el('div', 'st-main');
    const top = el('div', 'st-top');
    const работаМесто = el('div', 'st-job');
    const body = el('div', 'scroll pad st-body');
    main.append(top, работаМесто, body);
    wrap.append(side, main);

    /* ---------- разговор с системой ---------- */
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
      cancel(){ return Platform.rpc('pkg.cancel'); },
      обновления(){ return Platform.rpc('pkg.upgrade.check'); },
      обновить(){ return Platform.rpc('pkg.upgrade.run', {}); }
    };
    const размер = b => !b ? '' : b > 1048576 ? (b / 1048576).toFixed(1) + ' МБ'
                                              : Math.round(b / 1024) + ' КБ';

    /* ---------- что показываем ---------- */
    const РАЗДЕЛЫ = [
      { id:'сеть',    имя:'Интернет',     знак:'🌐', фон:'linear-gradient(140deg,#60a5fa,#1d4ed8)' },
      { id:'работа',  имя:'Работа',       знак:'📄', фон:'linear-gradient(140deg,#fbbf24,#b45309)' },
      { id:'медиа',   имя:'Музыка и кино',знак:'🎬', фон:'linear-gradient(140deg,#f472b6,#be185d)' },
      { id:'графика', имя:'Графика',      знак:'🎨', фон:'linear-gradient(140deg,#34d399,#047857)' },
      { id:'игры',    имя:'Игры',         знак:'🎮', фон:'linear-gradient(140deg,#a78bfa,#6d28d9)' },
      { id:'система', имя:'Инструменты',  знак:'🛠', фон:'linear-gradient(140deg,#94a3b8,#334155)' }
    ];

    /* Подборки составлены руками: в репозиториях нет ни витрин, ни
       разделов в человеческом смысле. Имя пакета пишем такое, под каким
       программа лежит в Ubuntu, иначе поиск по ней ничего не найдёт. */
    const ПОДБОРКИ = {
      'сеть':[
        /* Firefox здесь нет намеренно: он и так стоит в системе, а пакет
           «firefox» в Ubuntu — пустая заглушка, которая тянет snap, и snap
           у нас не работает. Предлагать его значило бы обещать установку,
           которая кончится ничем. */
        ['chromium', 'Chromium', 'Браузер на движке Chrome', '🌐'],
        ['telegram-desktop', 'Telegram', 'Мессенджер', '✈️'],
        ['thunderbird', 'Thunderbird', 'Почта', '📬'],
        ['transmission-gtk', 'Transmission', 'Торренты', '⬇️'],
        ['filezilla', 'FileZilla', 'Файлы по FTP и SFTP', '📡']
      ],
      'работа':[
        ['libreoffice', 'LibreOffice', 'Документы, таблицы, презентации', '📄'],
        ['obsidian', 'Obsidian', 'Заметки связанными страницами', '🗒'],
        ['calibre', 'Calibre', 'Библиотека книг и читалка', '📚'],
        ['keepassxc', 'KeePassXC', 'Хранилище паролей', '🔐'],
        ['scribus', 'Scribus', 'Вёрстка печатных изданий', '📰'],
        ['gnucash', 'GnuCash', 'Домашняя бухгалтерия', '💰']
      ],
      'медиа':[
        ['vlc', 'VLC', 'Проигрыватель, который играет всё', '🎬'],
        ['audacity', 'Audacity', 'Запись и правка звука', '🎙'],
        ['obs-studio', 'OBS Studio', 'Запись экрана и трансляции', '🎥'],
        ['kdenlive', 'Kdenlive', 'Монтаж видео', '🎞'],
        ['rhythmbox', 'Rhythmbox', 'Музыкальный проигрыватель', '🎵'],
        ['handbrake', 'HandBrake', 'Перегон видео между форматами', '🔄']
      ],
      'графика':[
        ['gimp', 'GIMP', 'Редактор изображений', '🎨'],
        ['inkscape', 'Inkscape', 'Векторная графика', '✒️'],
        ['krita', 'Krita', 'Рисование и живопись', '🖌'],
        ['blender', 'Blender', 'Трёхмерная графика и анимация', '🧊'],
        ['darktable', 'darktable', 'Проявка фотографий из RAW', '📷'],
        ['shotwell', 'Shotwell', 'Разбор домашнего фотоархива', '🖼']
      ],
      'игры':[
        ['steam', 'Steam', 'Магазин и запуск игр', '🎮'],
        ['0ad', '0 A.D.', 'Историческая стратегия', '🏛'],
        ['supertuxkart', 'SuperTuxKart', 'Гонки с пингвином', '🏎'],
        ['minetest', 'Minetest', 'Кубический мир и строительство', '⛏'],
        ['gnome-mines', 'Сапёр', 'Та самая игра', '💣'],
        ['aisleriot', 'Пасьянсы', 'Косынка и ещё восемьдесят', '🃏']
      ],
      'система':[
        ['htop', 'htop', 'Диспетчер задач в терминале', '📊'],
        ['gparted', 'GParted', 'Разметка дисков', '💽'],
        ['timeshift', 'Timeshift', 'Снимки системы для отката', '⏪'],
        ['synaptic', 'Synaptic', 'Подробный список всех пакетов', '📦'],
        ['gnome-disk-utility', 'Диски', 'Состояние и проверка дисков', '🩺'],
        ['neofetch', 'neofetch', 'Красивая справка о машине', '💬']
      ]
    };

    /* Витрина: то, с чего начинают на новой машине. */
    const ВИТРИНА = [
      { имя:'Telegram', пакет:'telegram-desktop', знак:'✈️',
        строка:'Мессенджер, который открывается быстрее, чем успеваешь передумать',
        фон:'linear-gradient(120deg,#38bdf8,#1d4ed8)' },
      { имя:'GIMP', пакет:'gimp', знак:'🎨',
        строка:'Всё, что делают с изображениями, — и бесплатно',
        фон:'linear-gradient(120deg,#34d399,#065f46)' },
      { имя:'VLC', пакет:'vlc', знак:'🎬',
        строка:'Играет любое видео. Правда любое',
        фон:'linear-gradient(120deg,#fb923c,#c2410c)' }
    ];

    /* ---------- состояние окна ---------- */
    let экран = (opts && opts['вкладка']) === 'linux' ? 'обзор'
              : (opts && opts['вкладка']) || 'обзор';
    let раздел = null;            // какой раздел открыт
    let программа = null;         // чья страница открыта
    let набрано = '', поискСтрока = '', поискСписок = null;
    let работа = null, началоРаботы = 0;
    let состояние = null;

    const узнайСостояние = async () => {
      if (состояние) return состояние;
      состояние = await Pkg.state().catch(e => ({ reason:String(e.message || e) }));
      return состояние;
    };

    /* ---------- боковой список ---------- */
    const ПУНКТЫ = [
      ['обзор',          '🏠', 'Обзор'],
      ['разделы',        '🗂', 'Разделы'],
      ['установленные',  '📦', 'Установленные'],
      ['обновления',     '⬆️', 'Обновления'],
      ['своё',           '🧑‍💻', 'Своё приложение']
    ];
    const рисуйБок = () => {
      side.innerHTML = '';
      ПУНКТЫ.forEach(([id, зн, имя]) => {
        const б = el('button', 'sb-item' + (экран === id ? ' on' : ''),
          `<span class="e">${зн}</span><span>${esc(имя)}</span>`);
        б.onclick = () => { экран = id; раздел = null; программа = null;
                            набрано = ''; поле.value = ''; draw(); };
        side.appendChild(б);
      });
    };

    /* ---------- поиск ---------- */
    const поле = el('input', 'inp st-find');
    поле.placeholder = '🔎 Найти программу: gimp, telegram, htop…';
    top.appendChild(поле);
    поле.oninput = () => { набрано = поле.value.trim(); draw(); };
    поле.onkeydown = е => {
      if (е.key !== 'Enter') return;
      набрано = поле.value.trim();
      if (набрано.length >= 2) найтиВРепозиториях(набрано); else draw();
    };

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

    async function найтиВРепозиториях(строка){
      const st = await узнайСостояние();
      поискСтрока = строка;
      if (строка.length < 2) return draw();
      if (!st || st.reason || !st.allowed){ поискСписок = null; return draw(); }

      if (работа && работа.action === 'update'){
        поискСписок = 'ждём'; draw();
        const j = await дождисьРаботы();
        st.lists = st.lists || !!j.ok;
      }
      /* В свежей системе списки пакетов пусты — их вычищают при сборке
         образа. Читаем сами: человек не обязан знать, что перед первым
         поиском надо нажать отдельную кнопку. */
      if (!st.lists && !работа){
        поискСписок = 'ждём'; draw();
        try { await Pkg.update(); const j = await дождисьРаботы(); st.lists = !!j.ok; }
        catch(e){ поискСписок = { ошибка:String(e.message || e) }; return draw(); }
      }

      поискСписок = 'ищу'; draw();
      try {
        const r = await Pkg.search(строка);
        const список = r.list.slice(0, 24);
        поискСписок = await Promise.all(список.map(async x => {
          try { return Object.assign(x, await Pkg.info(x.name, x.source)); } catch(e){ return x; }
        }));
      } catch(e){ поискСписок = { ошибка:String(e.message || e) }; }
      draw();
    }

    async function следиЗаРаботой(){
      for (let i = 0; i < 900; i++){
        let j;
        try { j = await Pkg.job(); } catch(e){ break; }
        работа = j.running ? j : null;
        if (работа && !началоРаботы) началоРаботы = Date.now();
        draw();
        if (!j.running){
          Shell.toast('Магазин',
            j.ok ? (j.action === 'install' ? 'Установлено: ' + j.name + ' — ищите в Пуске'
                  : j.action === 'remove' ? 'Удалено: ' + j.name : 'Списки обновлены')
                 : 'Не вышло: ' + (j.error || 'неизвестная причина'),
            j.ok ? '✅' : '⚠️', 8000);
          break;
        }
        await new Promise(r => setTimeout(r, 1200));
      }
      работа = null; началоРаботы = 0;
      if (программа) сведенияОПрограмме(программа.пакет, true);
      draw();
    }

    /* Работа могла начаться не здесь: человек щёлкнул по скачанному .deb,
       установка идёт, а Магазин закрыт. Открыв его, он вправе увидеть ход
       дела, а не пустое окно. */
    setTimeout(() => {
      Pkg.job().then(j => {
        if (j && j.running){ работа = j; началоРаботы = Date.now(); следиЗаРаботой(); }
      }).catch(() => {});
    }, 0);

    /* ---------- мелкие кирпичи ---------- */
    /* Пока система ходит в репозитории, на месте будущих карточек лежат
       серые заготовки. Так делают все хорошие магазины: пустой экран
       читается как поломка, а заготовка — как ожидание. */
    const заготовки = (сколько, класс) => {
      const г = el('div', класс || 'st-grid');
      for (let i = 0; i < сколько; i++){
        const з = el('div', 'st-skel');
        з.style.setProperty('--i', i);
        г.appendChild(з);
      }
      return г;
    };

    const значок = (зн, фон) => {
      const и = el('div', 'app-ico st-ico', зн);
      и.style.background = фон || 'rgba(var(--tint),.14)';
      return и;
    };

    /* Карточка программы: значок, имя, строка про неё. Нажатие открывает
       страницу — как в Microsoft Store, где карточка это вход, а не
       кнопка установки. */
    const карточка = (п, i) => {
      const к = el('button', 'st-card');
      к.style.setProperty('--i', i || 0);
      к.appendChild(значок(п.знак || '📦', п.фон));
      к.appendChild(el('div', 'st-card-t',
        `<b>${esc(п.имя)}</b><span class="tiny muted">${esc(п.про || '')}</span>`));
      к.onclick = () => откройПрограмму(п);
      return к;
    };

    const заголовок = (текст, ещё) => {
      const з = el('div', 'st-head');
      з.appendChild(el('div', 'card-t', текст));
      if (ещё) з.appendChild(ещё);
      return з;
    };

    /* ---------- страница программы ---------- */
    /* Сведения о программе приходят от машины: описание, размер, версия,
       стоит она уже или нет. Подборка знает только имя пакета и одну
       строку от нас — всё остальное спрашиваем. */
    const сведения = new Map();
    async function сведенияОПрограмме(пакет, заново){
      if (!заново && сведения.has(пакет)) return сведения.get(пакет);
      const st = await узнайСостояние();
      if (!st || st.reason || !st.allowed){
        const пусто = { нет:true, почему:(st && st.reason) || 'система не управляет машиной' };
        сведения.set(пакет, пусто);
        return пусто;
      }
      if (!st.lists){
        try { await Pkg.update(); const j = await дождисьРаботы(); st.lists = !!j.ok; } catch(e){}
      }
      let о = null;
      try { о = await Pkg.info(пакет); } catch(e){ о = null; }
      if (!о || (!о.candidate && !о.installed)){
        /* Под тем именем, что у нас записано, программы может не быть:
           в разных выпусках Ubuntu пакеты зовутся по-разному, а часть
           живёт только на Flathub. Тогда ищем по имени и берём похожее. */
        try {
          const r = await Pkg.search(пакет);
          const точное = (r.list || []).find(x => x.name === пакет) || (r.list || [])[0];
          if (точное) о = Object.assign(точное, await Pkg.info(точное.name, точное.source).catch(() => ({})));
        } catch(e){}
      }
      const итог = о && (о.candidate || о.installed) ? о : { нет:true, почему:'в репозиториях такого нет' };
      сведения.set(пакет, итог);
      return итог;
    }

    function откройПрограмму(п){
      программа = п;
      экран = 'программа';
      рисуйБок();
      draw();
      сведенияОПрограмме(п.пакет).then(() => { if (программа === п) draw(); });
    }

    /* ---------- ход установки ---------- */
    /* Виден из любого раздела: человек нажал «Установить», ушёл смотреть
       другое — и всё равно видит, что работа идёт. */
    function рисуйРаботу(){
      работаМесто.innerHTML = '';
      if (!работа) return;
      const box = el('div', 'card st-work');
      const молчит = работа.молчит || 0;
      const сек = Math.round((Date.now() - (началоРаботы || Date.now())) / 1000);
      const время = сек < 60 ? сек + ' с' : Math.floor(сек / 60) + ' мин ' + (сек % 60) + ' с';
      /* Не всякая работа печатает проценты: flatpak при первой установке
         тянет общую основу молча. Чтобы неподвижная полоса не читалась как
         зависание, она в этом случае бежит, а рядом написано, сколько идёт. */
      const естьПроценты = (работа.percent || 0) > 2;
      box.innerHTML = `<b>${esc(работа.action === 'remove' ? 'Удаление'
                            : работа.action === 'update' ? 'Обновление списков' : 'Установка')} ${esc(работа.name || '')}</b>
        <div class="ins-bar${естьПроценты ? '' : ' ins-bar-ждём'}" style="margin-top:10px"><i style="width:${
          естьПроценты ? работа.percent : 100}%"></i></div>
        <div class="muted tiny" style="margin-top:6px">${esc(работа.step || '')} · идёт ${время}${
          молчит > 60 ? ' · молчит ' + Math.round(молчит / 60) + ' мин — возможно, ждёт сеть' : ''}</div>`;
      const stop = el('button', 'btn', '✕ Остановить');
      stop.style.marginTop = '10px';
      stop.onclick = async () => {
        if (!await Dlg.confirm('Остановить работу?',
            'apt будет прерван, а система приведена в порядок.',
            { icon:'✕', okText:'Остановить', danger:true })) return;
        try { await Pkg.cancel(); работа = null; draw(); }
        catch(e){ Dlg.alert('Магазин', String(e.message || e), '⚠️'); }
      };
      box.appendChild(stop);
      работаМесто.appendChild(box);
    }

    /* ---------- установка и удаление ---------- */
    async function поставь(имя, источник){
      try { await Pkg.install(имя, источник); следиЗаРаботой(); draw(); }
      catch(e){ Dlg.alert('Не вышло начать установку', String(e.message || e), '⚠️'); }
    }
    async function убери(имя, источник){
      if (!await Dlg.confirm('Удалить ' + имя + '?',
          'Программа будет удалена вместе с ненужными зависимостями.',
          { icon:'🗑️', okText:'Удалить', danger:true })) return;
      try { await Pkg.remove(имя, источник); следиЗаРаботой(); draw(); }
      catch(e){ Dlg.alert('Не вышло удалить', String(e.message || e), '⚠️'); }
    }

    /* ---------- экраны ---------- */
    function экранОбзор(){
      /* Витрина меняется каждый день: не ради оживления, а чтобы человек,
         открывающий Магазин не в первый раз, видел не одно и то же. */
      const в = ВИТРИНА[new Date().getDate() % ВИТРИНА.length];
      const витрина = el('button', 'st-hero');
      витрина.style.background = в.фон;
      витрина.innerHTML = `<div class="st-hero-l">
          <div class="st-hero-z">${в.знак}</div>
          <div><h2>${esc(в.имя)}</h2><div class="st-hero-p">${esc(в.строка)}</div></div>
        </div><span class="st-hero-b">Подробнее ›</span>`;
      витрина.onclick = () => откройПрограмму({ имя:в.имя, пакет:в.пакет, знак:в.знак, про:в.строка, фон:в.фон });
      body.appendChild(витрина);

      body.appendChild(заголовок('Разделы'));
      const плитки = el('div', 'st-tiles');
      РАЗДЕЛЫ.forEach((р, i) => {
        const т = el('button', 'st-tile');
        т.style.setProperty('--i', i);
        т.style.background = р.фон;
        т.innerHTML = `<span class="st-tile-z">${р.знак}</span><span>${esc(р.имя)}</span>`;
        т.onclick = () => { экран = 'раздел'; раздел = р.id; рисуйБок(); draw(); };
        плитки.appendChild(т);
      });
      body.appendChild(плитки);

      body.appendChild(заголовок('Стоит поставить первым делом'));
      const г = el('div', 'st-grid');
      ['сеть', 'медиа', 'графика'].forEach((кто, i) => {
        const [пакет, имя, про, знак] = ПОДБОРКИ[кто][0];
        г.appendChild(карточка({ пакет, имя, про, знак }, i));
      });
      ['работа', 'система', 'игры'].forEach((кто, i) => {
        const [пакет, имя, про, знак] = ПОДБОРКИ[кто][0];
        г.appendChild(карточка({ пакет, имя, про, знак }, i + 3));
      });
      body.appendChild(г);
      win.setSub('обзор');
    }

    function экранРазделы(){
      body.appendChild(заголовок('Разделы'));
      const плитки = el('div', 'st-tiles');
      РАЗДЕЛЫ.forEach((р, i) => {
        const т = el('button', 'st-tile st-tile-big');
        т.style.setProperty('--i', i);
        т.style.background = р.фон;
        т.innerHTML = `<span class="st-tile-z">${р.знак}</span><span>${esc(р.имя)}</span>
          <span class="tiny" style="opacity:.8">${ПОДБОРКИ[р.id].length} программ</span>`;
        т.onclick = () => { экран = 'раздел'; раздел = р.id; рисуйБок(); draw(); };
        плитки.appendChild(т);
      });
      body.appendChild(плитки);
      win.setSub('разделы');
    }

    function экранРаздел(){
      const р = РАЗДЕЛЫ.find(x => x.id === раздел) || РАЗДЕЛЫ[0];
      const назад = el('button', 'btn st-back', '‹ Разделы');
      назад.onclick = () => { экран = 'разделы'; раздел = null; рисуйБок(); draw(); };
      body.appendChild(назад);
      body.appendChild(заголовок(р.знак + ' ' + р.имя));
      const г = el('div', 'st-grid');
      ПОДБОРКИ[р.id].forEach(([пакет, имя, про, знак], i) =>
        г.appendChild(карточка({ пакет, имя, про, знак }, i)));
      body.appendChild(г);
      win.setSub(р.имя.toLowerCase());
    }

    function экранПрограмма(){
      const п = программа;
      const назад = el('button', 'btn st-back', '‹ Назад');
      назад.onclick = () => {
        программа = null;
        экран = раздел ? 'раздел' : набрано ? 'обзор' : 'обзор';
        рисуйБок(); draw();
      };
      body.appendChild(назад);

      const шапка = el('div', 'st-page');
      шапка.appendChild(значок(п.знак || '📦', п.фон));
      const текст = el('div', 'st-page-t');
      текст.innerHTML = `<h2>${esc(п.имя)}</h2><div class="muted">${esc(п.про || '')}</div>`;
      шапка.appendChild(текст);
      const место = el('div', 'st-page-b');
      шапка.appendChild(место);
      body.appendChild(шапка);

      const о = сведения.get(п.пакет);
      if (!о){
        место.appendChild(el('div', 'st-skel st-skel-b'));
        body.appendChild(el('div', 'st-skel st-skel-l'));
        body.appendChild(el('div', 'st-skel st-skel-l'));
        win.setSub(п.имя.toLowerCase());
        return;
      }
      if (о.нет){
        место.appendChild(el('div', 'set-note', esc(о.почему)));
        body.appendChild(el('div', 'set-note',
          'Возможно, у программы другое имя в этом выпуске Ubuntu — попробуйте поиск сверху.'));
        win.setSub(п.имя.toLowerCase());
        return;
      }

      const стоит = !!о.installed;
      const кн = el('button', 'btn pri st-big-btn', стоит ? '🗑 Удалить' : '⬇ Установить');
      кн.disabled = !!работа;
      кн.onclick = () => стоит ? убери(о.name || п.пакет, о.source) : поставь(о.name || п.пакет, о.source);
      /* Часть пакетов в Ubuntu — пустые заглушки: сам пакет весит сто
         килобайт и тянет snap, а snap в системе не работает. Кнопка,
         которая начинает установку, кончающуюся ничем, хуже отсутствующей:
         человек ждёт, а потом ищет, что он сделал не так. */
      if (о.snap && !стоит){
        кн.disabled = true;
        кн.textContent = 'Недоступно';
      }
      место.appendChild(кн);
      if (стоит) место.appendChild(el('div', 'tiny muted', 'Уже в системе'));

      const свед = el('div', 'st-facts');
      const факт = (имя, что) => {
        if (!что) return;
        const ф = el('div', 'st-fact');
        ф.innerHTML = `<span class="tiny muted">${esc(имя)}</span><b>${esc(что)}</b>`;
        свед.appendChild(ф);
      };
      факт('Источник', о.source === 'flatpak' ? 'Flathub' : 'Ubuntu');
      факт('Версия', о.installed || о.candidate || '');
      факт('Размер', размер(о.size) ? (о.source === 'flatpak' ? 'скачает ' : 'займёт ') + размер(о.size) : '');
      факт('Пакет', о.name || п.пакет);
      body.appendChild(свед);

      if (о.about) body.appendChild(el('div', 'st-about', esc(о.about)));
      if (о.snap) body.appendChild(el('div', 'set-note',
        'Это заглушка: программа ставится через Snap, а он в системе не работает.'));
      win.setSub(п.имя.toLowerCase());
    }

    async function экранУстановленные(){
      body.appendChild(заголовок('Программы машины'));
      const место = el('div', 'st-grid');
      body.appendChild(место);
      место.replaceWith(заготовки(6));
      const свои = AppStore.custom();
      if (свои.length){
        body.appendChild(заголовок('Свои приложения'));
        const г = el('div', 'st-grid');
        свои.forEach((п, i) => {
          const к = карточка({ имя:п.name, про:'Своё приложение', знак:п.glyph || '📦', пакет:null }, i);
          к.onclick = () => WM.open(п.id);
          к.oncontextmenu = е => {
            е.preventDefault();
            Shell.ctx(е.clientX, е.clientY, [{ i:'🗑', t:'Удалить', f:() => { AppStore.uninstall(п.id); draw(); } }]);
          };
          г.appendChild(к);
        });
        body.appendChild(г);
      }
      win.setSub('установленные');

      /* Список программ машины спрашиваем у системы: это те же ярлыки, что
         видит Пуск. Заготовки на их месте лежат, пока идёт ответ. */
      let список = [];
      try { список = ((await Platform.rpc('sys.apps')) || {}).list || []; } catch(e){ список = []; }
      if (экран !== 'установленные') return;
      const г = el('div', 'st-grid');
      список.slice(0, 60).forEach((a, i) => {
        const к = карточка({ имя:a.name || a.id, про:a.comment || 'Программа машины',
                             знак:a.flatpak ? '🫙' : '🐧', пакет:null }, i);
        /* Значок берём настоящий, её собственный: человек узнаёт программу
           по нему, а не по нашему пингвину. */
        if (OS.значокПрограммы){
          const свой = OS.значокПрограммы({ значок:a.icon || '', id:a.id, flatpak:a.flatpak }, 'app-ico st-ico');
          к.replaceChild(свой, к.firstChild);
        }
        к.onclick = () => { OS.запустиПоЯрлыку ? OS.запустиПоЯрлыку(a.id) : null; };
        г.appendChild(к);
      });
      if (!список.length) г.appendChild(el('div', 'empty', 'Система не назвала ни одной программы'));
      const прежние = body.querySelector('.st-grid');
      if (прежние) прежние.replaceWith(г);
    }

    async function экранОбновления(){
      body.appendChild(заголовок('Обновления'));
      const место = el('div', '');
      body.appendChild(место);
      место.appendChild(el('div', 'st-skel st-skel-l'));
      win.setSub('обновления');

      let д = null, беда = '';
      try { д = await Pkg.обновления(); } catch(e){ беда = String(e.message || e); }
      if (экран !== 'обновления') return;
      место.innerHTML = '';
      if (!д){
        место.appendChild(el('div', 'set-note', 'Система не ответила про обновления: ' + esc(беда)));
        return;
      }
      const наш = (д.list || []).find(x => x.name === 'glower');
      if (наш){
        const к = el('div', 'card st-upd');
        к.innerHTML = `<div class="st-upd-l"><div class="st-hero-z">✨</div>
          <div><b>GlowerOS ${esc(наш['станет'])}</b>
          <div class="tiny muted">Сама система: оболочка, панель и внутренности · сейчас ${esc(наш['было'])}</div></div></div>`;
        const б = el('button', 'btn pri', '⬇ Обновить');
        б.disabled = !!работа;
        б.onclick = async () => {
          try { await Pkg.обновить(); следиЗаРаботой(); draw(); }
          catch(e){ Dlg.alert('Не вышло начать', String(e.message || e), '⚠️'); }
        };
        к.appendChild(б);
        место.appendChild(к);
      }
      const прочие = (д.list || []).filter(x => x.name !== 'glower');
      if (!д.list || !д.list.length){
        место.appendChild(el('div', 'empty', 'Всё обновлено'));
      } else if (прочие.length){
        /* Кнопка — в заголовке, а не под списком из сорока строк. Раньше
           до неё надо было прокрутить весь список, чтобы понять, что она
           вообще есть. */
        const все = el('button', 'btn pri', '⬆ Обновить всё');
        все.disabled = !!работа;
        все.onclick = async () => {
          if (!await Dlg.confirm('Обновить систему?',
              'Будет обновлено программ: ' + (д.list || []).length + '. Это займёт время и потребует сети.',
              { okText:'Обновить', icon:'⬆️' })) return;
          try { await Pkg.обновить(); следиЗаРаботой(); draw(); }
          catch(e){ Dlg.alert('Не вышло начать', String(e.message || e), '⚠️'); }
        };
        место.appendChild(заголовок('Программы Ubuntu · ' + прочие.length, все));
        прочие.slice(0, 40).forEach(x =>
          место.appendChild(row('📦', x.name, x['было'] + ' → ' + x['станет'], el('span'))));
        if (прочие.length > 40)
          место.appendChild(el('div', 'set-note',
            'И ещё ' + (прочие.length - 40) + ' — система покажет весь список при установке.'));
      }
      const держим = д['удержано'] || [];
      if (держим.length)
        место.appendChild(row('⏸', 'Отложено системой: ' + держим.join(', '),
          'Этим обновлениям нужны пакеты, которых на машине ещё нет', el('span')));
    }

    function экранСвоё(){
      body.appendChild(заголовок('Своё приложение'));
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
        try { AppStore.installPkg(JSON.parse(f.body)); экран = 'установленные'; рисуйБок(); draw(); WM.focus(win); }
        catch(e){ Shell.toast('Магазин', 'Не разобрать JSON: ' + e.message, '⚠️'); }
      }});
      c.appendChild(row('💾', 'Установить из системы', 'Выберите .json-пакет в Проводнике', fromFs));

      const fromDisk = el('button', 'btn', '⬆️ С компьютера');
      fromDisk.onclick = () => {
        const f = el('input'); f.type = 'file'; f.accept = '.json,application/json';
        f.onchange = () => {
          const r = new FileReader();
          r.onload = () => { try { AppStore.installPkg(JSON.parse(r.result)); экран = 'установленные'; рисуйБок(); draw(); }
            catch(e){ Shell.toast('Магазин', 'Не разобрать JSON: ' + e.message, '⚠️'); } };
          r.readAsText(f.files[0]);
        };
        f.click();
      };
      c.appendChild(row('🖥', 'Установить с диска', 'Настоящий файл с вашего компьютера', fromDisk));
      body.appendChild(c);

      const w = card('Как это работает');
      w.appendChild(row('⚙️', 'Код выполняется в системе',
        'Пакет исполняется как обычное приложение оболочки: у него есть доступ к окну, файловой системе и уведомлениям. ' +
        'Ставьте только те пакеты, содержимое которых вы видели.', el('span')));
      w.appendChild(el('pre', 'pkg-code', esc(JSON.stringify(AppStore.template(), null, 2)).slice(0, 900)));
      body.appendChild(w);
      win.setSub('своё приложение');
    }

    function экранПоиск(){
      body.appendChild(заголовок('Найдено по запросу «' + набрано + '»'));
      if (поискСписок === 'ждём'){
        body.appendChild(el('div', 'set-note', 'Читаю списки пакетов — это делается один раз…'));
        body.appendChild(заготовки(4, 'st-grid'));
        return;
      }
      if (поискСписок === 'ищу'){ body.appendChild(заготовки(4, 'st-grid')); return; }
      if (поискСписок && поискСписок.ошибка){
        body.appendChild(el('div', 'set-note', esc(поискСписок.ошибка)));
        return;
      }
      if (!Array.isArray(поискСписок) || поискСтрока !== набрано){
        body.appendChild(el('div', 'set-note', набрано.length < 2
          ? 'Наберите хотя бы две буквы и нажмите Enter'
          : 'Нажмите Enter, чтобы поискать это в репозиториях Ubuntu и Flathub'));
        return;
      }
      if (!поискСписок.length){
        body.appendChild(el('div', 'empty',
          'Ничего не нашлось. У программ бывают свои имена — попробуйте другое написание'));
        return;
      }
      const г = el('div', 'st-grid');
      поискСписок.forEach((x, i) => {
        const про = [x.about || '', x.source === 'flatpak' ? 'Flathub' : 'Ubuntu',
                     x.installed ? 'уже стоит' : '', размер(x.size)].filter(Boolean).join(' · ');
        сведения.set(x.name, x);
        г.appendChild(карточка({ имя:x.title && x.title !== x.name ? x.title : x.name,
                                 пакет:x.name, про, знак:x.source === 'flatpak' ? '🫙' : '📦' }, i));
      });
      body.appendChild(г);
    }

    /* ---------- отрисовка ---------- */
    function draw(){
      рисуйРаботу();
      body.innerHTML = '';
      /* Смена экрана — с движением: так видно, что это новая страница, а
         не переписанная старая. Анимация одна на всё содержимое, поэтому
         стоит она дёшево даже на слабой машине. */
      body.classList.remove('st-вошло');
      void body.offsetWidth;
      body.classList.add('st-вошло');

      if (набрано.length >= 1){ экранПоиск(); win.setSub('поиск'); return; }
      if (экран === 'программа' && программа){ экранПрограмма(); return; }
      if (экран === 'разделы'){ экранРазделы(); return; }
      if (экран === 'раздел'){ экранРаздел(); return; }
      if (экран === 'установленные'){ экранУстановленные(); return; }
      if (экран === 'обновления'){ экранОбновления(); return; }
      if (экран === 'своё'){ экранСвоё(); return; }
      экранОбзор();
    }

    рисуйБок();
    draw();
  }
};

/* Повторное открытие Магазина с нужной вкладкой.

   Окно у Магазина одно на всю систему (single), и второй раз WM его не
   создаёт, а поднимает существующее. Без этого человек, поставивший
   программу при уже открытом Магазине, видел ту вкладку, на которой он
   его оставил, — а ход установки идёт на другой. */
APPS.store.onReopen = function(win, opts){
  if (!opts || !opts['вкладка']) return;
  win.body.replaceChildren();
  APPS.store.render(win, opts);
};
