/* ==========================================================================
   Язык интерфейса

   Русский — исходный язык, английский собран словарём по точному совпадению
   строки. Всё, чего в словаре нет, остаётся по-русски — и Параметры об этом
   честно сообщают, а не делают вид, что перевод полон.
   ========================================================================== */
'use strict';

const I18N = {
  LANGS:{ ru:{ name:'Русский', code:'RU', locale:'ru-RU' },
          en:{ name:'English', code:'EN', locale:'en-GB' } },

  lang(){ return KV.get('lang', 'ru'); },
  locale(){ return this.LANGS[this.lang()].locale; },

  DICT:{ en:{
    /* оболочка */
    'Пуск':'Start', 'Поиск':'Search', 'Поиск в Windows':'Search', 'Поиск в Glower OS':'Search',
    'Поиск приложений, файлов и параметров':'Search apps, files and settings',
    'Все приложения ›':'All apps ›', '‹ Назад':'‹ Back', 'Все приложения':'All apps',
    'Закреплённые':'Pinned', 'Недавние':'Recent', 'Рекомендуем':'Recommended',
    'Виджеты':'Widgets', 'Центр управления':'Control centre', 'Батарея':'Battery',
    'Уведомления':'Notifications', 'Очистить все':'Clear all', 'Новых уведомлений нет':'No new notifications',
    'Свернуть все окна':'Show desktop', 'Корзина':'Recycle Bin', 'Этот компьютер':'This PC',
    'Рабочий стол':'Desktop', 'Документы':'Documents', 'Изображения':'Pictures',
    'Музыка':'Music', 'Загрузки':'Downloads', 'Быстрый доступ':'Quick access', 'Система':'System',
    'Просмотр задач':'Task view', 'Нет открытых окон':'No open windows', 'Всего приложений':'Apps total',
    /* питание и вход */
    'Завершение работы':'Shut down', 'Сменить пользователя':'Switch user', 'Заблокировать':'Lock',
    'Спящий режим':'Sleep', 'Перезагрузка':'Restart', 'Выключение':'Shut down', 'Отмена':'Cancel',
    'Войти':'Sign in', 'Пароль':'Password', 'Неверный пароль':'Wrong password',
    'Другие пользователи':'Other users', 'нажмите куда угодно или Enter':'press anywhere or Enter',
    /* приложения */
    'Блокнот':'Notepad', 'Проводник':'File Explorer', 'Параметры':'Settings', 'Браузер':'Browser',
    'Калькулятор':'Calculator', 'Терминал':'Terminal', 'Фотографии':'Photos', 'Календарь':'Calendar',
    'Часы':'Clock', 'Магазин':'Store', 'Задачи':'Tasks', 'Диспетчер задач':'Task Manager',
    'Свойства':'Properties', 'Помидор':'Pomodoro', 'Конвертер':'Converter', 'Палитра':'Palette',
    /* общие действия */
    'Открыть':'Open', 'Открыть с помощью':'Open with', 'Закрыть':'Close', 'Свернуть':'Minimise',
    'Развернуть':'Maximise', 'Восстановить':'Restore', 'Удалить':'Delete', 'Переименовать':'Rename',
    'Копировать':'Copy', 'Вырезать':'Cut', 'Вставить':'Paste', 'Обновить':'Refresh', 'Сохранить':'Save',
    'Создать папку':'New folder', 'Создать файл':'New file', 'Создать текстовый файл':'New text file',
    'Папка':'Folder', 'Файл':'File', 'Имя':'Name', 'Скачать':'Download', 'Найти':'Find', 'Новый':'New',
    'Печать…':'Print…', 'Свернуть всё':'Minimise all', 'Разложить окна':'Tile windows',
    'Каскад окон':'Cascade windows', 'Выровнять значки':'Align icons', 'Добавить виджет…':'Add widget…',
    'Убрать виджет':'Remove widget', 'Ярлык на рабочий стол':'Create desktop shortcut',
    'Создать ярлык на столе':'Create desktop shortcut', 'Убрать из дока':'Unpin from dock',
    'Добавить в док':'Pin to dock', 'Закрепить в Пуске':'Pin to Start', 'Открепить от Пуска':'Unpin from Start',
    'Открепить':'Unpin', 'Закрыть все окна':'Close all windows', 'Показать в Проводнике':'Show in Explorer',
    'Очистить список':'Clear list', 'Новая вкладка':'New tab', 'Закрыть вкладку':'Close tab',
    'Открыть в новом окне':'Open in new window', 'Показать папку':'Open containing folder',
    /* столбцы и статусы */
    'Дата изменения':'Date modified', 'Тип':'Type', 'Размер':'Size', 'Элементов':'Items',
    'Выбрано':'Selected', 'Папка пуста':'This folder is empty', 'Ничего не найдено':'Nothing found',
    'Плитка':'Tiles', 'Таблица':'Details', 'По имени':'By name', 'По размеру':'By size',
    'По дате':'By date', 'По типу':'By type', 'Корзина пуста':'Recycle Bin is empty',
    'Восстановить всё':'Restore all', 'Очистить корзину':'Empty Recycle Bin',
    /* диалоги */
    'Файл уже существует':'File already exists', 'Заменить файл в папке назначения':'Replace the file in the destination',
    'Сохранить оба файла — новый получит номер':'Keep both files — the new one gets a number',
    'Пропустить этот файл':'Skip this file', 'Применить ко всем оставшимся':'Do this for all remaining',
    'Удалить в корзину':'Move to Recycle Bin', 'Очистить':'Clear', 'Всегда':'Always',
    'Только сейчас':'Just once',
    /* параметры */
    'Главная':'Home', 'Персонализация':'Personalisation', 'Рабочий стол и док':'Desktop and dock',
    'Движение и анимации':'Motion', 'Звук':'Sound', 'Дисплей':'Display', 'Сеть и Интернет':'Network',
    'Устройства и датчики':'Devices', 'Уведомления и фокус':'Notifications', 'Приложения':'Apps',
    'Учётные записи':'Accounts', 'Время и язык':'Time and language', 'Спец. возможности':'Accessibility',
    'Конфиденциальность':'Privacy', 'Сборка':'Build', 'О системе':'About',
    'Найти параметр':'Find a setting', 'Тема':'Theme', 'Прозрачная':'Transparent', 'Тёмная':'Dark',
    'Светлая':'Light', 'Режим':'Mode', 'Цвет акцента':'Accent colour', 'Свой цвет':'Custom colour',
    'Фон рабочего стола':'Desktop background', 'Слайд-шоу':'Slideshow', 'Своё изображение':'Your own image',
    'Материал интерфейса — Liquid Glass':'Interface material — Liquid Glass', 'Материал':'Material',
    'Прозрачность':'Transparency', 'Размытие фона':'Background blur', 'Плотность стекла':'Glass density',
    'Яркость кромки':'Edge highlight', 'Насыщенность':'Saturation', 'Форма':'Shape', 'Пресеты':'Presets',
    'Громкость':'Volume', 'Яркость':'Brightness', 'Ночной свет':'Night light',
    'Виджеты рабочего стола':'Desktop widgets', 'Показывать виджеты':'Show widgets',
    'Автозапуск':'Startup', 'Резервная копия системы':'System backup', 'Экспорт всей системы':'Export the whole system',
    'Восстановить из файла':'Restore from a file', 'Хранилище':'Storage', 'Сброс системы':'Reset the system',
    'Язык интерфейса':'Interface language', 'Раскладки клавиатуры':'Keyboard layouts',
    'Часовой пояс':'Time zone', 'Город для погоды':'City for weather', 'Дата и время':'Date and time',
    'Язык и регион':'Language and region', 'Подключение к системе':'System connection',
    'Автономный режим':'Standalone mode', 'Подключено к системе':'Connected to the system',
    'настоящий диск':'real disk', 'браузер':'browser', 'Синхронизация':'Sync',
    /* прочее */
    'Погода':'Weather', 'Сегодня':'Today', 'Событий нет':'No events', 'Всё сделано 🎉':'All done 🎉',
    'Заметка':'Note', 'Нажмите и пишите…':'Click and type…', 'Новая задача…':'New task…',
    'Раскладка клавиатуры':'Keyboard layout', 'Параметры языка':'Language settings',
    'Русская':'Russian', 'Английская':'English', 'Немецкая':'German',
    /* виджеты и мелкие подписи */
    'Недавно использовалось':'Recently used', 'Окон открыто':'Windows open',
    'Тема':'Theme', 'Размытие':'Blur', 'Прозрачная':'Transparent', 'Тёмная':'Dark', 'Светлая':'Light',
    'демо':'demo', 'Ясно':'Clear', 'Переменная облачность':'Partly cloudy', 'Дождь':'Rain',
    'Снег':'Snow', 'Малооблачно':'Mostly clear', 'Гроза':'Thunderstorm', 'Туман':'Fog',
    'Облачно':'Cloudy', 'Заряд':'Charge', 'Сеть':'Network', 'Громкость':'Volume', 'Яркость':'Brightness',
    'Пн':'Mon', 'Вт':'Tue', 'Ср':'Wed', 'Чт':'Thu', 'Пт':'Fri', 'Сб':'Sat', 'Вс':'Sun'
  }},

  t(s){
    const d = this.DICT[this.lang()];
    if (!d) return s;
    if (d[s]) return d[s];
    /* составные подписи вида «Погода · Москва · демо» переводим по частям */
    if (s.includes(' · ')){
      const parts = s.split(' · ');
      if (parts.some(p => d[p.trim()])) return parts.map(p => d[p.trim()] || p).join(' · ');
    }
    return s;
  },

  /* перевод уже отрисованного интерфейса */
  apply(root = document.body){
    if (this.lang() === 'ru') return;
    const skip = 'textarea, input, .term, .np-area, .dw-note, [contenteditable]';
    const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(n){
        if (!n.nodeValue.trim()) return NodeFilter.FILTER_REJECT;
        if (n.parentElement && n.parentElement.closest(skip)) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      }
    });
    const nodes = [];
    while (walk.nextNode()) nodes.push(walk.currentNode);
    nodes.forEach(n => {
      const raw = n.nodeValue.trim();
      const tr = this.t(raw);
      if (tr !== raw) n.nodeValue = n.nodeValue.replace(raw, tr);
    });
    $$('[data-tip], [placeholder], [title]', root).forEach(e => {
      ['data-tip', 'placeholder', 'title'].forEach(a => {
        const v = e.getAttribute(a);
        if (v){ const tr = this.t(v.trim()); if (tr !== v.trim()) e.setAttribute(a, tr); }
      });
    });
  },

  set(lang){
    if (!this.LANGS[lang]) return;
    KV.set('lang', lang);
    document.documentElement.lang = lang;
    location.reload();                    // перерисовываем систему целиком
  },

  /* сколько строк вообще переведено */
  coverage(){ return Object.keys(this.DICT.en).length; }
};
window.I18N = I18N;

/* даты и время следуют за языком интерфейса */
(function localeDates(){
  const D = Date.prototype;
  const wrap = name => {
    const orig = D[name];
    D[name] = function(loc, opt){ return orig.call(this, loc === 'ru-RU' ? I18N.locale() : loc, opt); };
  };
  ['toLocaleDateString', 'toLocaleTimeString', 'toLocaleString'].forEach(wrap);
})();

/* живой перевод: всё, что дорисовывается, переводится сразу */
(function liveTranslate(){
  document.documentElement.lang = I18N.lang();
  if (I18N.lang() === 'ru') return;
  I18N.apply();
  let queued = false;
  new MutationObserver(() => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => { queued = false; I18N.apply(); });
  }).observe(document.body, { childList:true, subtree:true, characterData:true });
})();
