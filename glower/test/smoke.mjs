#!/usr/bin/env node
/* ==========================================================================
   Смоук-тесты прототипа: гоняют настоящий Chromium по ключевым сценариям.

   Запуск:  node test/smoke.mjs
   Chromium ищется автоматически; можно указать свой:  CHROME=/путь/к/chrome
   ========================================================================== */
import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const URL_APP = pathToFileURL(resolve(root, 'index.html')).href;
const require = createRequire(import.meta.url);

/* ---------- поиск playwright и браузера ---------- */
let chromium;
for (const id of ['playwright', 'playwright-core', '/opt/node22/lib/node_modules/playwright']){
  try { ({ chromium } = await import(id === 'playwright' || id === 'playwright-core'
        ? require.resolve(id) && id : pathToFileURL(id + '/index.mjs').href)); break; } catch(e){}
}
if (!chromium){
  console.error('Нужен playwright: npm i -D playwright');
  process.exit(2);
}
const CANDIDATES = [process.env.CHROME,
  '/opt/pw-browsers/chromium-1194/chrome-linux/chrome',
  '/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome'].filter(Boolean);
const exe = CANDIDATES.find(p => existsSync(p));

/* ---------- крошечный раннер ---------- */
let passed = 0, failed = 0;
const results = [];
const check = (name, cond, detail = '') => {
  if (cond){ passed++; results.push(`  ✅ ${name}`); }
  else { failed++; results.push(`  ❌ ${name}${detail ? ' — ' + detail : ''}`); }
};

const browser = await chromium.launch(exe ? { executablePath:exe, args:['--no-sandbox'] } : { args:['--no-sandbox'] });
const page = await browser.newPage({ viewport:{ width:1440, height:900 } });
const jsErrors = [];
page.on('pageerror', e => jsErrors.push(e.message));
page.on('console', m => { if (m.type() === 'error' && !/ERR_|net::/.test(m.text())) jsErrors.push(m.text()); });
page.on('dialog', d => d.accept());

/* обычные проверки идут по уже настроенной системе — мастер первого запуска
   проверяется отдельно, в самом конце */
await page.addInitScript(() => {
  try { localStorage.setItem('glower.setup.done', 'true'); } catch(e){}
});

const boot = async () => {
  await page.goto(URL_APP);
  await page.waitForTimeout(2400);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(900);
};

try {
  /* --- загрузка --- */
  await boot();
  check('система загружается и разблокируется',
    await page.evaluate(() => $('#desktop').classList.contains('on')));
  check('приветственных баннеров нет', (await page.$$('.toast')).length === 0);

  /* --- рабочий стол --- */
  await page.dblclick('.di[data-name="Заметка.txt"]');
  await page.waitForTimeout(700);
  check('двойной клик по значку открывает файл',
    await page.evaluate(() => WM.wins.some(w => w.appId === 'notepad')));
  check('верхняя панель прячется при открытом окне',
    await page.evaluate(() => document.body.classList.contains('chrome-hidden')));

  /* --- «Все приложения» в Пуске --- */
  await page.evaluate(() => Shell.toggleStart(true));
  await page.waitForTimeout(500);
  await page.click('#start-all');
  await page.waitForTimeout(500);
  const all = await page.evaluate(() => ({
    rows: document.querySelectorAll('#start-results .all-row').length,
    apps: Object.keys(APPS).length,
    letters: document.querySelectorAll('#start-results .all-letter').length,
    btn: document.querySelector('#start-all').textContent
  }));
  check('«Все приложения» показывает весь список',
    all.rows === all.apps && all.rows > 0, `строк ${all.rows} при ${all.apps} приложениях`);
  check('список сгруппирован по буквам', all.letters > 1);
  check('в списке есть своя кнопка «Назад»',
    await page.evaluate(() => !!document.querySelector('#start-results .all-head .mini-btn')));
  await page.click('#start-results .all-head .mini-btn'); await page.waitForTimeout(400);
  check('кнопка «Назад» возвращает к закреплённым',
    await page.evaluate(() => !document.querySelector('#start-body').hidden));
  await page.evaluate(() => Shell.closePanels());
  await page.waitForTimeout(300);

  /* --- панель задач --- */
  await page.evaluate(() => WM.toggleMax(WM.top()));
  await page.waitForTimeout(800);
  check('док становится панелью задач', await page.evaluate(() => document.body.classList.contains('taskbar')));
  check('развёрнутое окно не заходит под панель',
    await page.evaluate(() => Math.round(WM.top().node.getBoundingClientRect().height) < innerHeight - 40));
  const щель = await page.evaluate(() => {
    const о = WM.top().node.getBoundingClientRect();
    const п = document.querySelector('.dock-wrap').getBoundingClientRect();
    return Math.round(п.top - о.bottom);
  });
  check('между развёрнутым окном и панелью нет щели', Math.abs(щель) <= 1, щель + 'px');

  /* --- приложения открываются --- */
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(400);
  const apps = ['files','term','paint','photos','music','calendar','clock','browser','todo','taskmgr','store','settings','calc','trash'];
  for (const id of apps){ await page.evaluate(a => WM.open(a), id); await page.waitForTimeout(220); }
  await page.waitForTimeout(500);
  check(`все ${apps.length} приложений открываются`,
    await page.evaluate(n => WM.wins.length === n, apps.length),
    await page.evaluate(() => WM.wins.length) + ' окон');

  /* --- калькулятор --- */
  const calc = await page.evaluate(() => {
    const w = WM.wins.find(x => x.appId === 'calc');
    const k = [...w.body.querySelectorAll('.calc-keys button')];
    k.find(b => b.textContent === '7').click(); k.find(b => b.textContent === '×').click();
    k.find(b => b.textContent === '8').click(); k.find(b => b.textContent === '=').click();
    return w.body.querySelector('.calc-val').textContent;
  });
  check('калькулятор считает 7×8=56', calc === '56', 'получено ' + calc);

  /* --- терминал --- */
  await page.evaluate(() => { const w = WM.wins.find(x => x.appId === 'term');
    WM.focus(w); w.body.querySelector('.term-in input').focus(); });
  for (const cmd of ['ls', 'expr 6*7', 'sysinfo']){
    await page.keyboard.type(cmd); await page.keyboard.press('Enter'); await page.waitForTimeout(160);
  }
  const term = await page.evaluate(() => WM.wins.find(x => x.appId === 'term').body.innerText);
  const brand = await page.evaluate(() => Brand.name);
  check('терминал выполняет команды', /^42$/m.test(term) && term.includes(brand),
    'бренд «' + brand + '» в выводе: ' + term.includes(brand));

  /* --- файловая система --- */
  const fsOk = await page.evaluate(() => {
    FS.write(['Документы'], 'тест.txt', 'проверка');
    const written = FS.node(['Документы', 'тест.txt']);
    FS.rm(['Документы'], 'тест.txt');
    const inTrash = KV.get('trash', []).some(t => t.name === 'тест.txt');
    return !!written && !FS.node(['Документы', 'тест.txt']) && inTrash;
  });
  check('файл создаётся, удаляется и попадает в корзину', fsOk);

  /* --- магазин --- */
  /* Каталог наших приложений из Магазина убран целиком, и установки «из
     каталога» больше нет: ставить оттуда нечего. Свои пакеты — остались,
     это единственное, что Магазин ставит сам, без помощи машины. */
  const store = await page.evaluate(() => {
    AppStore.installPkg({ id:'smoke-app', name:'Смоук', glyph:'🧪',
      code:"win.body.appendChild(api.el('div','pad','ok'));" });
    return { custom:!!APPS['smoke-app'], каталога:typeof AppStore.install };
  });
  check('свой пакет устанавливается', store.custom);
  check('установки из каталога больше нет', store.каталога === 'undefined', store.каталога);
  await page.evaluate(() => AppStore.uninstall('smoke-app'));
  await page.waitForTimeout(500);
  check('приложение удаляется', await page.evaluate(() => !APPS['smoke-app']));

  /* --- системные диалоги вместо браузерных --- */
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(400);
  await page.evaluate(() => WM.open('files', { path:['Документы'] }));
  await page.waitForTimeout(700);
  await page.click('[data-a="nf"]');
  await page.waitForTimeout(500);
  check('создание папки открывает системный диалог, а не браузерный',
    await page.evaluate(() => !!document.querySelector('.dlg-ov.on .dlg')));
  await page.fill('.dlg input', 'Из диалога');
  await page.click('.dlg-foot .btn.pri');
  await page.waitForTimeout(500);
  check('диалог создаёт папку',
    await page.evaluate(() => !!FS.node(['Документы', 'Из диалога'])));
  check('диалог закрылся', (await page.$$('.dlg-ov')).length === 0);

  /* --- центр уведомлений --- */
  await page.evaluate(() => { Notif.clear(); Shell.toast('Проверка', 'Запись в журнале', '🧪'); });
  await page.waitForTimeout(400);
  check('уведомление попадает в журнал',
    await page.evaluate(() => Notif.list().length === 1));
  await page.evaluate(() => Shell.panel('#widgets'));
  await page.waitForTimeout(500);
  check('журнал виден в панели',
    await page.evaluate(() => !!document.querySelector('#notif-box .nc-item')));
  await page.evaluate(() => { Notif.clear(); Shell.closePanels(); });
  await page.waitForTimeout(300);

  /* --- индикатор громкости --- */
  await page.evaluate(() => Store.set('volume', 30));
  await page.waitForTimeout(300);
  check('громкость показывает индикатор',
    await page.evaluate(() => document.querySelector('.osd.on') !== null));

  /* --- восстановление сеанса --- */
  await page.evaluate(() => { WM.wins.forEach(w => WM.close(w)); });
  await page.waitForTimeout(400);
  await page.evaluate(() => { WM.open('calc'); WM.open('clock'); Session.save(); });
  await page.waitForTimeout(700);
  await page.reload();
  /* Под нагрузкой экран блокировки появляется позже, и одиночное нажатие
     Enter уходило в пустоту: рабочий стол не открывался, а восстановление
     сеанса начинается только после него. Жмём, пока стол не откроется. */
  for (let i = 0; i < 40; i++){
    const ready = await page.evaluate(() => document.querySelector('#desktop').classList.contains('on'));
    if (ready) break;
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);
  }
  /* восстановление идёт по таймеру, а под нагрузкой он растягивается —
     ждём результата, а не фиксированной паузы */
  const restored = await page.evaluate(async () => {
    /* под нагрузкой восстановление занимает секунды, а не мгновение;
       ждём до минуты, иначе проверка мигает без всякой поломки */
    for (let i = 0; i < 240; i++){
      if (['calc','clock'].every(a => WM.wins.some(w => w.appId === a))) return true;
      await new Promise(r => setTimeout(r, 250));
    }
    return false;
  });
  /* Эта проверка изредка мигала, и по одной строке «провалено» понять было
     нечего: то ли стол не открылся, то ли сеанс не сохранился, то ли просто
     не успел. Пишем в пояснение всё три величины сразу — при следующем
     мигании догадываться не придётся. */
  check('окна восстанавливаются после перезагрузки', restored,
    await page.evaluate(() => {
      let сохранено = '—';
      try {
        /* ключ складывается так же, как в KV: «glower.» + имя набора */
        const сырое = localStorage.getItem('glower.' + (window.__ns || '') + Session.KEY);
        сохранено = (JSON.parse(сырое || '[]') || []).map(z => z.app).join(',') || '(пусто)';
      } catch(e){ сохранено = 'нечитаемо'; }
      return 'открыты: ' + (WM.wins.map(w => w.appId).join(',') || '(нет)')
        + ' · сохранено: ' + сохранено
        + ' · стол ' + (document.querySelector('#desktop').classList.contains('on') ? 'открыт' : 'НЕ ОТКРЫТ')
        + ' · помнить сеанс: ' + S.restoreSession
        + ' · восстановлено: ' + (window.Session && Session.восстановлено)
        + ' · есть APPS: ' + ['calc','clock'].filter(a => APPS[a]).join(',')
        + ' · журнал: ' + (KV.get('session.журнал', []) || [])
            .map(з => з.п + '=' + з.с).join(' ');
    }));
  await page.evaluate(() => { WM.wins.forEach(w => WM.close(w)); Session.save(); });
  await page.waitForTimeout(400);

  /* --- ассоциации, свойства, меню, диспетчер --- */
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(400);
  check('ассоциации по расширению',
    await page.evaluate(() => Assoc.appFor({ name:'a.png' }) === 'photos'
      && Assoc.appFor({ name:'b.txt' }) === 'notepad'
      && Assoc.appFor({ name:'c.html' }) === 'browser'));
  await page.evaluate(() => Assoc.set('txt', 'browser'));
  check('ассоциацию можно переназначить',
    await page.evaluate(() => Assoc.appFor({ name:'b.txt' }) === 'browser'));
  await page.evaluate(() => { const m = KV.get('assoc', {}); delete m.txt; KV.set('assoc', m); });

  await page.evaluate(() => WM.open('props', { node:FS.node(['Документы','Идеи.txt']), path:['Документы'] }));
  await page.waitForTimeout(600);
  check('свойства открываются отдельным окном',
    await page.evaluate(() => { const w = WM.wins.find(x => x.appId === 'props');
      return !!w && /Расположение/.test(w.body.innerText) && /Изменён/.test(w.body.innerText); }));

  await page.evaluate(() => WM.open('notepad'));
  await page.waitForTimeout(600);
  check('в Блокноте есть меню Файл/Правка/Вид',
    await page.evaluate(() => { const w = WM.wins.find(x => x.appId === 'notepad');
      return [...w.body.querySelectorAll('.menu-b')].map(b => b.textContent).join(',') === 'Файл,Правка,Вид'; }));

  await page.evaluate(() => WM.open('taskmgr'));
  await page.waitForTimeout(1800);
  check('диспетчер показывает настоящие метрики',
    await page.evaluate(() => { const w = WM.wins.find(x => x.appId === 'taskmgr');
      return /FPS/.test(w.body.innerText) && /Элементов DOM/.test(w.body.innerText); }));

  /* --- недавние документы, списки переходов, ярлыки --- */
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(400);
  await page.evaluate(() => { Recent.clear();
    Assoc.open(FS.node(['Документы','Идеи.txt']), ['Документы']); });
  await page.waitForTimeout(600);
  check('открытый файл попадает в недавние',
    await page.evaluate(() => Recent.list()[0] && Recent.list()[0].name === 'Идеи.txt'));
  await page.evaluate(() => Shell.toggleStart(true));
  await page.waitForTimeout(500);
  check('«Недавние» в Пуске показывают файл',
    await page.evaluate(() => /Идеи\.txt/.test(document.querySelector('#start-reco').textContent)));
  await page.evaluate(() => Shell.closePanels());

  await page.evaluate(() => Link.toDesktop({ app:'calc' }, 'Калькулятор'));
  await page.waitForTimeout(500);
  check('ярлык появляется на рабочем столе',
    await page.evaluate(() => !!document.querySelector('.di.is-link')));
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(300);
  await page.dblclick('.di.is-link');
  await page.waitForTimeout(700);
  check('ярлык запускает приложение',
    await page.evaluate(() => WM.wins.some(w => w.appId === 'calc')));

  /* --- «Свернуть всё» --- */
  check('в панели есть кнопка «Свернуть всё»', (await page.$$('.show-desktop')).length === 1);
  await page.click('.show-desktop'); await page.waitForTimeout(600);
  check('кнопка сворачивает окна',
    await page.evaluate(() => WM.wins.every(w => w.minimized)));
  await page.click('.show-desktop'); await page.waitForTimeout(600);
  check('повторное нажатие возвращает окна',
    await page.evaluate(() => WM.wins.every(w => !w.minimized)));

  /* --- конфликт при вставке и переименование на месте --- */
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(400);
  await page.evaluate(() => {
    FS.write(['Документы'], 'дубль.txt', 'исходный');
    FS.write(['Загрузки'], 'дубль.txt', 'новый');
    WM.open('files', { path:['Загрузки'] });
  });
  await page.waitForTimeout(800);
  await page.click('.fe-it[data-name="дубль.txt"], .fe-tr[data-name="дубль.txt"]');
  await page.click('[data-a="copy"]');
  await page.waitForTimeout(300);
  await page.evaluate(() => { const w = WM.wins.find(x => x.appId === 'files');
    w.body.querySelectorAll('.sb-item')[2].click(); });        // Документы
  await page.waitForTimeout(600);
  await page.click('[data-a="paste"]');
  await page.waitForTimeout(600);
  check('вставка поверх существующего спрашивает, что делать',
    await page.evaluate(() => !!document.querySelector('.conflict')));
  await page.click('.conflict-b:nth-child(2)');                // «оставить оба»
  await page.waitForTimeout(600);
  check('«оставить оба» создаёт копию с номером',
    await page.evaluate(() => !!FS.node(['Документы', 'дубль (1).txt'])
      && FS.node(['Документы', 'дубль.txt']).body === 'исходный'));

  await page.click('.fe-it[data-name="дубль (1).txt"], .fe-tr[data-name="дубль (1).txt"]');
  await page.keyboard.press('F2');
  await page.waitForTimeout(400);
  check('F2 переименовывает прямо на значке',
    await page.evaluate(() => !!document.querySelector('.inline-edit')));
  await page.fill('.inline-edit', 'переименован.txt');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(600);
  check('новое имя сохраняется',
    await page.evaluate(() => !!FS.node(['Документы', 'переименован.txt'])));
  await page.evaluate(() => { FS.rm(['Документы'], 'переименован.txt', true);
    FS.rm(['Документы'], 'дубль.txt', true); FS.rm(['Загрузки'], 'дубль.txt', true); });

  /* --- вкладки, адресная строка, поиск везде --- */
  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(400);
  await page.evaluate(() => WM.open('files', { path:['Документы'] }));
  await page.waitForTimeout(800);
  check('у Проводника есть вкладки', (await page.$$('.fe-tab')).length === 1);
  await page.click('.fe-tab-add'); await page.waitForTimeout(700);
  check('вкладка добавляется', (await page.$$('.fe-tab')).length === 2);
  await page.click('.fe-tab:first-child'); await page.waitForTimeout(600);
  check('переключение вкладок работает',
    await page.evaluate(() => document.querySelector('.fe-tab').classList.contains('on')));
  await page.click('.fe-tab:nth-child(2) .x'); await page.waitForTimeout(600);
  check('вкладка закрывается', (await page.$$('.fe-tab')).length === 1);

  await page.click('.fe-path-edit'); await page.waitForTimeout(400);
  check('адресная строка редактируется', (await page.$$('.fe-path-inp')).length === 1);
  await page.fill('.fe-path-inp', '/Изображения');
  await page.keyboard.press('Enter'); await page.waitForTimeout(700);
  check('переход по введённому пути',
    await page.evaluate(() => String(WM.wins.find(w => w.appId === 'files').data.path) === 'Изображения'));

  await page.click('.fe-all'); await page.waitForTimeout(300);
  await page.fill('.fe-find', 'readme');
  await page.waitForTimeout(700);
  check('поиск по всему компьютеру находит вложенный файл',
    await page.evaluate(() => /readme\.md/.test(document.querySelector('.scroll').textContent)
      && /Проекты/.test(document.querySelector('.scroll').textContent)));

  /* --- бренд, языки, раскладки --- */
  check('система называется собственным именем',
    await page.evaluate(() => Brand.name === 'GlowerOS' && document.title === 'GlowerOS'
      && !document.body.innerText.includes('Windows 12')));

  check('в панели поиска нет чужого имени системы',
    await page.evaluate(() => {
      const t = document.querySelector('#tb-search').textContent;
      return t.includes('GlowerOS') && !/Windows/.test(t);
    }));

  check('фирменный логотип загружен и виден',
    await page.evaluate(() => {
      const imgs = [...document.querySelectorAll('img.os-logo')];
      return imgs.length >= 1 && imgs.every(i => i.complete && i.naturalWidth > 0);
    }));

  check('в Параметрах нет настройки полноэкранного режима',
    await page.evaluate(async () => {
      WM.open('settings', { section:'display' });
      await new Promise(r => setTimeout(r, 700));
      const w = WM.wins.find(w => w.appId === 'settings');
      return !w.body.innerText.includes('Полноэкранный');
    }));

  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(300);

  /* --- переключатели, которые раньше ничего не делали --- */
  check('выключённые уведомления не показывают баннер, но пишутся в центр',
    await page.evaluate(async () => {
      KV.set('notif', false);
      const before = Notif.list().length;
      Shell.toast('Проверка', 'баннера быть не должно', '🔔');
      await new Promise(r => setTimeout(r, 250));
      const noBanner = document.querySelectorAll('.toast').length === 0;
      const logged = Notif.list().length === before + 1;
      KV.set('notif', true);
      return noBanner && logged;
    }));

  check('включённые уведомления снова показывают баннер',
    await page.evaluate(async () => {
      Shell.toast('Проверка', 'баннер должен быть', '🔔');
      await new Promise(r => setTimeout(r, 250));
      const ok = document.querySelectorAll('.toast').length > 0;
      document.querySelectorAll('.toast').forEach(t => t.remove());
      return ok;
    }));

  check('моно-звук действительно сводит выход в один канал',
    await page.evaluate(() => {
      A11Y.applyMono(true);
      const c = Snd.ac();
      const bus = A11Y.bus(c);
      const ok = bus.channelCount === 1 && bus.channelCountMode === 'explicit';
      A11Y.applyMono(false);
      return ok;
    }));

  check('залипание клавиш держит модификатор до следующей клавиши',
    await page.evaluate(async () => {
      KV.set('sticky', true);
      dispatchEvent(new KeyboardEvent('keydown', { key:'Control', bubbles:true }));
      const latched = !!A11Y.latched.Control && !!document.querySelector('#sticky-osd');
      let sawCtrl = false;
      const h = e => { if (e.key === 'k' && e.ctrlKey) sawCtrl = true; };
      addEventListener('keydown', h);
      document.body.dispatchEvent(new KeyboardEvent('keydown', { key:'k', bubbles:true }));
      await new Promise(r => setTimeout(r, 60));
      removeEventListener('keydown', h);
      const cleared = !A11Y.latched.Control && !document.querySelector('#sticky-osd');
      KV.set('sticky', false);
      return latched && sawCtrl && cleared;
    }));

  check('приложения по умолчанию действительно меняют ассоциацию',
    await page.evaluate(async () => {
      KV.set('assoc', {});
      const was = Assoc.appFor({ name:'заметка.txt' });
      const m = KV.get('assoc', {}); m.txt = 'term'; KV.set('assoc', m);
      const now = Assoc.appFor({ name:'заметка.txt' });
      KV.set('assoc', {});
      return was === 'notepad' && now === 'term' && Assoc.appFor({ name:'заметка.txt' }) === 'notepad';
    }));

  /* --- то, что срабатывает само --- */
  check('будильник срабатывает в назначенную минуту',
    await page.evaluate(async () => {
      KV.set('alarms', []);
      const now = new Date();
      Alarms.add({ h:now.getHours(), m:now.getMinutes(), label:'Проверка', days:[] });
      let fired = false;
      const orig = Dlg.alert.bind(Dlg);
      Dlg.alert = (t) => { if (/Будильник/.test(t)) fired = true; return Promise.resolve(true); };
      Alarms.tick();
      await new Promise(r => setTimeout(r, 150));
      Dlg.alert = orig;
      const off = Alarms.list()[0].on === false;      // разовый выключается сам
      KV.set('alarms', []);
      return fired && off;
    }));

  check('будильник не срабатывает дважды за одну минуту',
    await page.evaluate(async () => {
      KV.set('alarms', []);
      const now = new Date();
      Alarms.add({ h:now.getHours(), m:now.getMinutes(), label:'Раз в минуту', days:[0,1,2,3,4,5,6] });
      let count = 0;
      const orig = Dlg.alert.bind(Dlg);
      Dlg.alert = () => { count++; return Promise.resolve(true); };
      Alarms.tick(); Alarms.tick(); Alarms.tick();
      await new Promise(r => setTimeout(r, 150));
      Dlg.alert = orig;
      KV.set('alarms', []);
      return count === 1;
    }));

  check('напоминание календаря приходит в указанное время',
    await page.evaluate(async () => {
      const now = new Date();
      const key = `${now.getFullYear()}-${now.getMonth()}-${now.getDate()}`;
      const hm = String(now.getHours()).padStart(2,'0') + ':' + String(now.getMinutes()).padStart(2,'0');
      KV.set('cal.events', { [key]:{ t:'Встреча', time:hm } });
      KV.set('cal.fired', {});
      let seen = '';
      const orig = Shell.toast.bind(Shell);
      Shell.toast = (title, text) => { if (title === 'Календарь') seen = text; };
      Reminders.tick(); Reminders.tick();
      Shell.toast = orig;
      KV.set('cal.events', {});
      return seen.includes('Встреча') && seen.includes(hm);
    }));

  check('автоблокировка срабатывает после бездействия и молчит при активности',
    await page.evaluate(async () => {
      // замок теперь системный: бездействие зовёт Shell.lock(), а тот —
      // отдельную программу через агента. Подменяем именно его.
      const orig = Shell.lock.bind(Shell);
      let locked = 0;
      Shell.lock = () => { locked++; };
      KV.set('autoLock', 1);

      AutoLock.last = Date.now() - 120000;   // две минуты без ввода
      AutoLock.tick();
      const afterIdle = locked;

      AutoLock.touch();                       // пользователь пошевелился
      AutoLock.tick();
      const afterTouch = locked;

      KV.set('autoLock', 0);
      AutoLock.last = Date.now() - 120000;
      AutoLock.tick();                        // выключенная настройка ничего не делает
      const afterOff = locked;

      Shell.lock = orig;
      AutoLock.touch();
      return afterIdle === 1 && afterTouch === 1 && afterOff === 1;
    }));

  check('поиск находит слово внутри файла, а не только в имени',
    await page.evaluate(() => {
      FS.write(['Документы'], 'протокол.txt', 'секретное слово барсук внутри файла');
      const res = Search.scan('барсук');
      const f = res.find(r => r.node.name === 'протокол.txt');
      FS.rm(['Документы'], 'протокол.txt', true);
      return !!f && !!f.hit && f.hit.includes('барсук');
    }));

  /* --- Проводник: один конвейер вместо цепочки обёрток --- */
  check('дополнения Проводника выстроены в очередь, а не обёрнуты друг в друга',
    await page.evaluate(() => {
      const names = APPS.files.parts.map(p => p.name);
      return typeof APPS.files.paint === 'function' && names.length === 3
        && names[0] === 'диалоги' && names[2] === 'вкладки и адресная строка';
    }));

  check('сбой одного дополнения не ломает Проводник целиком',
    await page.evaluate(async () => {
      const ce = console.error;
      console.error = () => {};                      // ошибку ждём намеренно
      APPS.files.use('поломанное', () => { throw new Error('специально'); });
      WM.open('files', { path:['Документы'] });
      await new Promise(r => setTimeout(r, 700));
      const w = WM.wins.find(x => x.appId === 'files');
      const alive = !!w && !!w.body.querySelector('.fe-tabs') && !!w.body.querySelector('.sidebar');
      APPS.files.parts.pop();
      console.error = ce;
      WM.close(w);
      await new Promise(r => setTimeout(r, 300));
      return alive;
    }));

  /* --- замок --- */
  /* Своего экрана блокировки у оболочки больше нет: замок системный, его
     показывает отдельная программа поверх всего. Проверяем то, что осталось
     на нашей стороне — что оболочка зовёт систему, а без системы честно
     говорит об этом и ничего не прячет. */
  check('замок без системы не прячет рабочий стол, а объясняет, почему',
    await page.evaluate(async () => {
      const было = document.body.className;
      let сказано = '';
      const orig = Shell.toast.bind(Shell);
      Shell.toast = (title, text) => { сказано = String(text || ''); };
      await Shell.lock();
      Shell.toast = orig;
      await new Promise(r => setTimeout(r, 200));
      const целостность = document.body.className === было
        && !document.body.classList.contains('locked')
        && !document.getElementById('lock');
      return целостность && сказано.length > 0;
    }));

  /* --- переключение окон --- */
  check('Alt + ` переключает окна системы',
    await page.evaluate(async () => {
      WM.open('notepad'); await new Promise(r => setTimeout(r, 400));
      WM.open('calc');    await new Promise(r => setTimeout(r, 400));
      const first = WM.top().appId;
      dispatchEvent(new KeyboardEvent('keydown', { key:'`', code:'Backquote', altKey:true, bubbles:true }));
      await new Promise(r => setTimeout(r, 250));
      const second = WM.top().appId;
      const osd = !!document.querySelector('.switcher');
      WM.wins.forEach(w => WM.close(w));
      await new Promise(r => setTimeout(r, 300));
      return first !== second && osd;
    }));

  check('Параметры честно объясняют, какие сочетания забирает браузер',
    await page.evaluate(async () => {
      WM.open('settings', { section:'keys' });
      await new Promise(r => setTimeout(r, 700));
      const t = WM.wins.find(w => w.appId === 'settings').body.innerText;
      WM.wins.forEach(w => WM.close(w));
      await new Promise(r => setTimeout(r, 250));
      return t.includes('Alt + Tab') && t.includes('Alt + `') && /забира/.test(t);
    }));

  check('раскладки спрятаны под стрелку и раскрываются по щелчку',
    await page.evaluate(async () => {
      KV.set('exp.kb', false);
      WM.open('settings', { section:'time' });
      await new Promise(r => setTimeout(r, 700));
      const w = WM.wins.find(x => x.appId === 'settings');
      const exp = w.body.querySelector('.set-exp');
      const closed = exp && !exp.classList.contains('open')
        && exp.querySelector('.exp-body').getBoundingClientRect().height < 4;
      exp.querySelector('.set-row').click();
      await new Promise(r => setTimeout(r, 600));
      const opened = exp.classList.contains('open')
        && exp.querySelectorAll('.exp-body .switch').length === Object.keys(Layouts.ALL).length
        && exp.querySelector('.exp-body').getBoundingClientRect().height > 60;
      exp.querySelector('.set-row').click();
      await new Promise(r => setTimeout(r, 500));
      const closedAgain = !exp.classList.contains('open');
      WM.wins.forEach(x => WM.close(x));
      await new Promise(r => setTimeout(r, 300));
      return closed && opened && closedAgain;
    }));

  check('система не показывает окон самого браузера',
    await page.evaluate(async () => {
      /* если что-то позовёт prompt/confirm/alert — засчитаем как провал */
      let native = 0;
      ['prompt', 'confirm', 'alert'].forEach(k => { window[k] = () => { native++; return null; }; });

      const seen = [];
      const openDlg = async fn => {
        fn();
        await new Promise(r => setTimeout(r, 350));
        const d = document.querySelector('.dlg-ov');
        seen.push(!!d);
        if (d){ const c = d.querySelector('.dlg-foot .btn'); if (c) c.click(); }
        await new Promise(r => setTimeout(r, 350));
      };

      WM.open('files', { path:['Документы'] });
      await new Promise(r => setTimeout(r, 800));
      const w = WM.wins.find(x => x.appId === 'files');
      const bar2 = w.body.querySelectorAll('.toolbar')[1];

      await openDlg(() => bar2.querySelector('[data-a="nf"]').click());
      await openDlg(() => bar2.querySelector('[data-a="nt"]').click());
      WM.close(w);
      await new Promise(r => setTimeout(r, 300));

      WM.open('paint');
      await new Promise(r => setTimeout(r, 700));
      const pw = WM.wins.find(x => x.appId === 'paint');
      await openDlg(() => [...pw.body.querySelectorAll('.btn')].find(b => b.textContent.includes('В файлы')).click());
      WM.wins.forEach(x => WM.close(x));
      await new Promise(r => setTimeout(r, 300));

      return native === 0 && seen.length === 3 && seen.every(Boolean);
    }));

  check('меню «Добавить виджет» открывается и виджет появляется на столе',
    await page.evaluate(async () => {
      S.deskWidgets = []; Store.save(); Shell.renderDeskWidgets();
      /* правый щелчок по пустому месту рабочего стола */
      const d = document.querySelector('#desktop');
      d.dispatchEvent(new MouseEvent('contextmenu', { bubbles:true, clientX:640, clientY:500 }));
      await new Promise(r => setTimeout(r, 300));
      const add = [...document.querySelectorAll('#ctx button')].find(b => /Добавить виджет/.test(b.textContent));
      if (!add) return false;
      add.click();
      await new Promise(r => setTimeout(r, 400));
      const menu = document.querySelector('#ctx');
      const listed = menu.classList.contains('on') && menu.querySelectorAll('button').length >= 4;
      if (!listed) return false;
      menu.querySelector('button').click();          // берём первый виджет
      await new Promise(r => setTimeout(r, 400));
      const placed = S.deskWidgets.length === 1 && document.querySelectorAll('#desk-widgets .dw').length === 1;
      S.deskWidgets = []; Store.save(); Shell.renderDeskWidgets();
      return placed;
    }));

  check('индикатор раскладки есть в панели', (await page.$$('#kb-badge')).length === 1);
  check('по умолчанию латиница — набор не подменяется',
    await page.evaluate(() => Layouts.current() === 'en' && document.querySelector('#kb-badge').textContent === 'ENG'));

  await page.evaluate(() => WM.wins.forEach(w => WM.close(w)));
  await page.waitForTimeout(300);
  await page.evaluate(() => { Layouts.set('ru'); WM.open('notepad'); });
  await page.waitForTimeout(800);
  check('переключение раскладки меняет индикатор',
    await page.evaluate(() => document.querySelector('#kb-badge').textContent === 'РУС'));
  await page.click('.np-area');
  await page.keyboard.type('ghbdtn');          // на ЙЦУКЕН это «привет»
  await page.waitForTimeout(300);
  check('русская раскладка даёт кириллицу по расположению клавиш',
    await page.evaluate(() => document.querySelector('.np-area').value === 'привет'),
    await page.evaluate(() => document.querySelector('.np-area').value));
  await page.evaluate(() => Layouts.set('en'));
  await page.keyboard.type('ok');
  await page.waitForTimeout(300);
  check('латинская раскладка ничего не меняет',
    await page.evaluate(() => document.querySelector('.np-area').value.endsWith('ok')));

  /* --- английский интерфейс --- */
  await page.evaluate(() => { KV.set('lang', 'en'); });
  await page.reload();
  await page.waitForTimeout(2600); await page.keyboard.press('Enter'); await page.waitForTimeout(1200);
  await page.evaluate(() => Shell.toggleStart(true));
  await page.waitForTimeout(600);
  const enUI = await page.evaluate(() => ({
    start: document.querySelector('#start-all').textContent,
    search: document.querySelector('#start-input').placeholder,
    pinned: document.querySelector('.start-head span').textContent
  }));
  check('интерфейс переключается на английский',
    enUI.start.includes('All apps') && /Search/.test(enUI.search) && enUI.pinned === 'Pinned',
    JSON.stringify(enUI));
  await page.evaluate(() => { KV.set('lang', 'ru'); });
  await page.reload();
  await page.waitForTimeout(2600); await page.keyboard.press('Enter'); await page.waitForTimeout(900);
  check('возврат на русский', await page.evaluate(() => document.querySelector('#start-input')
    .placeholder.includes('Поиск')));

  /* --- в меню питания нет эмодзи --- */
  check('в меню питания нет эмодзи',
    await page.evaluate(() => ![...document.querySelectorAll('.power-actions button')]
      .some(b => /\p{Extended_Pictographic}/u.test(b.textContent))));
  check('свечение вокруг курсора убрано',
    await page.evaluate(() => !getComputedStyle(document.querySelector('.dock'), '::before').background.includes('radial-gradient')));

  /* --- темы --- */
  for (const t of ['dark', 'light', 'glass']){
    await page.evaluate(x => Store.set('theme', x), t);
    await page.waitForTimeout(250);
  }
  check('темы переключаются без ошибок', await page.evaluate(() => S.theme === 'glass'));
  await page.evaluate(() => { if (!WM.wins.length) WM.open('calc'); });
  await page.waitForTimeout(500);
  const darkOpaque = await page.evaluate(async () => {
    Store.set('theme', 'dark'); await new Promise(r => setTimeout(r, 250));
    const bg = getComputedStyle(document.querySelector('.win')).backgroundColor;
    Store.set('theme', 'glass');
    return !/rgba\(.*0\.\d/.test(bg);
  });
  check('в тёмной теме окна непрозрачные', darkOpaque);

  /* --- профили --- */
  await page.evaluate(async () => {
    await Profiles.setPassword('default', 'pass123');
    FS.write(['Документы'], 'личное.txt', 'секрет');
    await Profiles.add('Гость', 'Г', '');
  });
  await page.reload(); await page.waitForTimeout(2600);

  /* Своего экрана входа у оболочки больше нет — машину отпирает система.
     Пароль профиля остался при одном: он спрашивается, когда переходишь в
     чужой профиль, и именно это здесь и проверяем. */
  check('пароль профиля отклоняет чужого',
    await page.evaluate(async () => !(await Profiles.проверьПрофиль('default', 'wrong'))));
  check('пароль профиля пускает своего',
    await page.evaluate(async () => await Profiles.проверьПрофиль('default', 'pass123')));

  const guest = await page.evaluate(() => Profiles.list().find(p => p.name === 'Гость').id);
  await page.evaluate(id => Profiles.switchTo(id), guest);
  await page.waitForTimeout(2700);
  check('данные профилей изолированы',
    await page.evaluate(() => S.userName === 'Гость' && !FS.node(['Документы', 'личное.txt'])));

  /* --- журнал ошибок --- */
  /* --- начальная настройка при первом запуске --- */
  await page.evaluate(() => localStorage.clear());
  await page.addInitScript(() => { try { localStorage.removeItem('glower.setup.done'); } catch(e){} });
  await page.goto(URL_APP);
  await page.waitForTimeout(2900);

  check('при первом запуске система встречает настройкой, а не рабочим столом',
    await page.evaluate(() => !!document.querySelector('#setup')
      && document.querySelector('#setup').classList.contains('on')
      && !document.querySelector('#desktop').classList.contains('on')));

  /* на медленной машине скрипты выполняются дольше загрузочного экрана —
     именно на этом настройка однажды потерялась в виртуалке */
  check('настройка не теряется на медленной машине',
    await (async () => {
      const slow = await browser.newPage({ viewport:{ width:1280, height:800 } });
      const cdp = await slow.context().newCDPSession(slow);
      await cdp.send('Emulation.setCPUThrottlingRate', { rate:20 });
      await slow.goto(URL_APP);
      await slow.waitForTimeout(9000);
      const ok = await slow.evaluate(() => !!document.querySelector('#setup')
        && !document.querySelector('#desktop').classList.contains('on'));
      await slow.close();
      return ok;
    })());

  check('шаги переключаются вперёд и назад',
    await page.evaluate(async () => {
      const step = () => document.querySelector('#setup-steps .on');
      const idx = () => [...document.querySelectorAll('#setup-steps i')].indexOf(step());
      const was = idx();
      document.querySelector('#setup-next').click();
      await new Promise(r => setTimeout(r, 300));
      const fwd = idx();
      document.querySelector('#setup-back').click();
      await new Promise(r => setTimeout(r, 300));
      return was === 0 && fwd === 1 && idx() === 0;
    }));

  check('выбор английского меняет язык мастера сразу, а не после перезагрузки',
    await page.evaluate(async () => {
      Setup.i = 1; Setup.paint();                       // шаг «Язык системы»
      await new Promise(r => setTimeout(r, 300));
      const ru = document.querySelector('.setup-t').textContent;
      [...document.querySelectorAll('.setup-card')].find(c => /English/.test(c.textContent)).click();
      await new Promise(r => setTimeout(r, 350));
      const en = document.querySelector('.setup-t').textContent;
      const btn = document.querySelector('#setup-next').textContent;
      Setup.i = 4; Setup.paint();
      await new Promise(r => setTimeout(r, 300));
      const step = document.querySelector('.setup-t').textContent;

      [...document.querySelectorAll('.setup-card')];    // возвращаемся к русскому
      Setup.data.lang = 'ru'; Setup.i = 1; Setup.paint();
      await new Promise(r => setTimeout(r, 300));
      const back = document.querySelector('.setup-t').textContent;

      return ru === 'Язык системы' && en === 'System language' && btn === 'Next'
        && step === 'What should we call you' && back === 'Язык системы';
    }));

  check('без имени дальше не пускает',
    await page.evaluate(async () => {
      Setup.i = 4; Setup.paint();                       // шаг «как к вам обращаться»
      await new Promise(r => setTimeout(r, 300));
      document.querySelector('#setup-next').click();
      await new Promise(r => setTimeout(r, 300));
      const stuck = Setup.i === 4 && /имени/.test(document.querySelector('.setup-err').textContent);
      const inp = document.querySelector('.setup-input');
      inp.value = 'Проверка';
      inp.dispatchEvent(new Event('input', { bubbles:true }));
      document.querySelector('#setup-next').click();
      await new Promise(r => setTimeout(r, 350));
      return stuck && Setup.i === 5;
    }));

  check('выбор в мастере действительно применяется к системе',
    await page.evaluate(async () => {
      Setup.data.theme = 'dark';
      Setup.data.accent = 5;
      Setup.data.wallpaper = 'mint';
      Setup.data.city = 'Казань';
      Setup.data.layouts = ['en', 'de'];
      Setup.i = Setup.STEPS.length - 1; Setup.paint();
      await new Promise(r => setTimeout(r, 300));
      document.querySelector('#setup-next').click();
      await new Promise(r => setTimeout(r, 900));
      return S.theme === 'dark' && S.accent === 5 && S.wallpaper === 'mint'
        && S.city === 'Казань' && S.userName === 'Проверка'
        && JSON.stringify(KV.get('kb.enabled', [])) === JSON.stringify(['en', 'de'])
        && KV.get('setup.done', false) === true;
    }));

  check('после настройки показывается приветствие с именем',
    await page.evaluate(() => {
      const w = document.querySelector('.welcome');
      return !!w && /Проверка/.test(w.textContent) && /Добро пожаловать/.test(w.textContent);
    }));

  check('приветствие уходит и открывает рабочий стол',
    await page.evaluate(async () => {
      await new Promise(r => setTimeout(r, 3400));
      return !document.querySelector('.welcome')
        && document.querySelector('#desktop').classList.contains('on');
    }));

  check('второй запуск проходит без настройки',
    await page.evaluate(() => KV.get('setup.done', false) === true));

  check('на слабой машине система сама снимает нагрузку',
    await page.evaluate(async () => {
      KV.set('perf.userChoice', false);
      KV.set('perf.lite', false);
      const real = Perf.measure.bind(Perf);
      Perf.measure = async () => 9;                 // как будто девять кадров в секунду
      await Perf.check();
      const lite = KV.get('perf.lite', false) && S.blur === 0 && S.reduceMotion === true;
      Perf.lite(false);
      KV.set('perf.lite', false);
      Perf.measure = real;
      return lite && S.blur === 34;
    }));

  check('выбор пользователя система не перебивает',
    await page.evaluate(async () => {
      KV.set('perf.userChoice', true);              // человек сам решил
      KV.set('perf.lite', false);
      const real = Perf.measure.bind(Perf);
      Perf.measure = async () => 5;
      await Perf.check();
      const untouched = KV.get('perf.lite', false) === false && S.blur === 34;
      Perf.measure = real;
      KV.set('perf.userChoice', false);
      return untouched;
    }));

  /* --- панель задач не наезжает сама на себя --- */
  /* панель проверяем на нескольких ширинах: на узких экранах она ломалась */
  for (const size of [{ width:1440, height:900 }, { width:1280, height:720 },
                      { width:1024, height:640 }, { width:860, height:600 }]){
    await page.setViewportSize(size);
    await page.waitForTimeout(500);
    const r = await page.evaluate(async () => {
      ['notepad','files','settings','music','photos','calc','mail','store','clock'].forEach(id => {
        try { WM.open(id); } catch(e){}
      });
      const w = WM.wins[0];
      if (w && !w.maximized) WM.toggleMax(w);
      await new Promise(r2 => setTimeout(r2, 600));
      const dock = document.querySelector('#dock');
      const tray = document.querySelector('#tb-tray').getBoundingClientRect();
      const items = [...document.querySelectorAll('#dock .dock-item, .tb-dock-search')];
      const over = items.filter(n => {
        const b = n.getBoundingClientRect();
        return b.right > tray.left + 1 && b.left < tray.right - 1 &&
               b.bottom > tray.top + 1 && b.top < tray.bottom - 1;
      });
      const d = dock.getBoundingClientRect();
      return { over:over.length, items:items.length,
        wider:Math.round(d.width) > innerWidth + 1,
        outside:items.some(n => { const b = n.getBoundingClientRect();
          return b.left < -1 || b.right > innerWidth + 1; }) };
    });
    check(`панель не ломается при ширине ${size.width}`,
      r.over === 0 && !r.wider && !r.outside && r.items > 5, JSON.stringify(r));
  }
  await page.setViewportSize({ width:1440, height:900 });
  await page.waitForTimeout(400);

  /* указатель открытого окна и подсказка когда-то делили один элемент:
     из-за этого под значком висела белая клякса, а наведение показывало
     обрывки. Проверяем, что теперь это разные элементы и ничего не режется. */
  check('указатель открытого окна не спорит с подсказкой',
    await page.evaluate(async () => {
      ['notepad','files','music'].forEach(id => { try { WM.open(id); } catch(e){} });
      await new Promise(r => setTimeout(r, 400));
      const it = document.querySelector('#dock .dock-item.run');
      if (!it) return false;
      const before = getComputedStyle(it, '::before');
      const after = getComputedStyle(it, '::after');
      const узкий = parseFloat(before.width) <= 20;          // указатель — точка или полоска
      const подсказка = after.content.includes('Блокнот') || after.content.includes('attr') ||
                        after.content !== 'none';            // подсказка живёт отдельно
      const целый = getComputedStyle(it.closest('.dock-items') || it).overflow !== 'hidden';
      return узкий && подсказка && целый;
    }));

  check('в развёрнутом окне значки панели не залезают на трей',
    await page.evaluate(async () => {
      ['notepad', 'files', 'settings', 'music', 'photos', 'calc', 'mail', 'store'].forEach(id => {
        try { WM.open(id); } catch(e){}
      });
      const w = WM.wins[0];
      if (!w.maximized) WM.toggleMax(w);
      await new Promise(r => setTimeout(r, 700));
      const tray = document.querySelector('#tb-tray').getBoundingClientRect();
      const items = [...document.querySelectorAll('#dock .dock-item, .tb-dock-search')];
      const over = items.filter(n => {
        const r = n.getBoundingClientRect();
        return r.right > tray.left + 1 && r.left < tray.right - 1 &&
               r.bottom > tray.top + 1 && r.top < tray.bottom - 1;
      });
      return { over:over.length, items:items.length };
    }).then(r => r.over === 0 && r.items > 5));

  /* --- имя из мастера первого запуска доходит до Пуска --- */
  check('в Пуске стоит имя, выбранное при настройке',
    await page.evaluate(async () => {
      Store.set('userName', 'Тестовый');
      const l = Profiles.list();
      l[0].name = 'Тестовый'; l[0].emoji = 'Т';
      Profiles.save(l);
      Profiles.renderChip();
      await new Promise(r => setTimeout(r, 200));
      const chip = document.querySelector('#start-user').textContent;
      return chip.includes('Тестовый');
    }));

  /* --- часовой пояс --- */
  check('часовой пояс переключается и двигает часы',
    await page.evaluate(async () => {
      /* Сравнивать «стало как было» по строке нельзя: между двумя
         замерами может смениться минута, и проверка падает на ровном
         месте, ничего не поймав. Сверяем с часами машины, с допуском в
         минуту. */
      const минуты = т => { const [ч, м] = т.split(':').map(Number);
        return Number.isFinite(ч) ? ч * 60 + м : null; };
      Store.set('tz', 'Asia/Kamchatka');
      Shell.clock();
      const kam = document.querySelector('#tray-time').textContent;
      Store.set('tz', 'Europe/London');
      Shell.clock();
      const lon = document.querySelector('#tray-time').textContent;
      Store.set('tz', '');
      Shell.clock();
      const back = минуты(document.querySelector('#tray-time').textContent);
      const здесь = new Date();
      const надо = здесь.getHours() * 60 + здесь.getMinutes();
      return kam !== lon && back !== null && Math.abs(back - надо) <= 1;
    }));

  check('в настройках есть список часовых поясов',
    await page.evaluate(async () => {
      WM.open('settings', { section:'time' });
      await new Promise(r => setTimeout(r, 800));
      const w = WM.wins.find(x => x.appId === 'settings');
      const s2 = [...w.body.querySelectorAll('select')]
        .find(x => [...x.options].some(o => /Europe\/Moscow/.test(o.value)));
      if (!s2) return false;
      s2.value = 'Asia/Tokyo';
      s2.dispatchEvent(new Event('change'));
      await new Promise(r => setTimeout(r, 300));
      const ok = S.tz === 'Asia/Tokyo';
      Store.set('tz', '');
      return ok;
    }));

  /* настройка первого запуска проходится с клавиатуры: без мыши тоже система */
  const клавиатурой = await (async () => {
    const p4 = await browser.newPage({ viewport:{ width:1280, height:800 } });
    try {
      await p4.goto(URL_APP);
      await p4.waitForFunction(() => !!document.getElementById('setup'), null, { timeout:20000 }).catch(() => {});
      const было = await p4.evaluate(() => (window.Setup && Setup.i) || 0);
      await p4.keyboard.press('Enter');
      await p4.waitForTimeout(600);
      const стало = await p4.evaluate(() => (window.Setup && Setup.i) || 0);
      await p4.keyboard.press('Backspace');
      await p4.waitForTimeout(600);
      const назад = await p4.evaluate(() => (window.Setup && Setup.i) || 0);
      return { было, стало, назад };
    } finally { await p4.close(); }
  })();
  const темаВИтогах = await (async () => {
    const p5 = await browser.newPage({ viewport:{ width:1280, height:800 } });
    try {
      await p5.goto(URL_APP);
      await p5.waitForFunction(() => !!window.Setup, null, { timeout:20000 }).catch(() => {});
      return await p5.evaluate(() => Setup.data && Setup.data.theme);
    } finally { await p5.close(); }
  })();
  check('настройка предлагает тёмную тему, а не убранную прозрачную',
    темаВИтогах === 'dark', String(темаВИтогах));

  check('настройка идёт вперёд по Enter и назад по Backspace',
    клавиатурой.стало === клавиатурой.было + 1 && клавиатурой.назад === клавиатурой.было,
    JSON.stringify(клавиатурой));

  /* установочная среда: только мастер, ничего лишнего вокруг */
  const среда = await (async () => {
    const p2 = await browser.newPage({ viewport:{ width:1280, height:800 } });
    try {
      await p2.goto(URL_APP + '?install=1');
      await p2.waitForFunction(() => document.querySelector('#desktop').classList.contains('on'),
        null, { timeout:20000 }).catch(() => {});
      return await p2.evaluate(() => ({
        настройка:!!document.querySelector('#setup.on'),
        заблокировано:!!document.querySelector('#lock'),   // замка в странице быть не должно вовсе
        рабочий:document.querySelector('#desktop').classList.contains('on'),
        средаУстановки:document.body.classList.contains('setup-env'),
        докВиден:getComputedStyle(document.querySelector('.dock-wrap')).display !== 'none',
        тема:document.documentElement.dataset.theme
      }));
    } finally { await p2.close(); }
  })();
  check('в установочной среде нет ни входа, ни настройки',
    среда.настройка === false && среда.заблокировано === false && среда.рабочий === true,
    JSON.stringify(среда));
  check('в установочной среде убран рабочий стол',
    среда.средаУстановки === true && среда.докВиден === false, JSON.stringify(среда));
  check('окно установки показывается на тёмном', среда.тема === 'dark', String(среда.тема));

  check('восстановление опознаётся отдельно от установки',
    await (async () => {
      const p3 = await browser.newPage({ viewport:{ width:1280, height:800 } });
      try {
        await p3.goto(URL_APP + '?install=1&repair=1');
        await p3.waitForTimeout(1500);
        return await p3.evaluate(() => Install.repairing() === true &&
          /[?&]install=1/.test(location.search));
      } finally { await p3.close(); }
    })());

  /* --- Ножницы --- */
  /* Снять настоящий экран здесь нечем: браузер в проверке не имеет ни
     оконного сервера, ни системы. Поэтому проверяем всё, что наше, —
     разметку поверх готового кадра и сохранение. Сам кадр подсовываем
     маленькой картинкой, как его отдал бы агент. */
  const ножницы = await page.evaluate(async () => {
    WM.open('snip');
    await new Promise(r => setTimeout(r, 500));
    const win = WM.wins.find(w => w.appId === 'snip');
    if (!win) return { есть:false };

    const пустоВидно = getComputedStyle(win.body.querySelector('.snip-empty')).display !== 'none';
    const холстСпрятан = win.body.querySelector('.snip-canvas').style.display === 'none';

    /* Картинка 2×2, какую вернул бы агент */
    const кадр = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFUlEQVR42mNk+M9Qz0BFwDiqkL4KAeUlA/1BpFSNAAAAAElFTkSuQmCC';
    await win.data.новыйКадр(кадр);

    const холст = win.body.querySelector('.snip-canvas');
    const состояние = win.data.snip;
    const кадрВидно = холст.style.display !== 'none';

    /* Рисуем так, как рисует человек: мышью по холсту. Класть штрихи прямо
       в состояние было бы проверкой ни о чём — кнопка отмены при этом
       остаётся выключенной, и щелчок по ней ничего не делает. */
    const рисуй = (x1, y1, x2, y2) => {
      const r = холст.getBoundingClientRect();
      const точка = (x, y) => ({ clientX:r.left + x, clientY:r.top + y,
                                 pointerId:1, bubbles:true });
      холст.dispatchEvent(new PointerEvent('pointerdown', точка(x1, y1)));
      холст.dispatchEvent(new PointerEvent('pointermove', точка(x2, y2)));
      холст.dispatchEvent(new PointerEvent('pointerup', точка(x2, y2)));
    };
    рисуй(4, 4, 40, 40);
    рисуй(10, 30, 60, 12);
    const было = состояние.штрихи.length;

    const отменить = [...win.body.querySelectorAll('.snip-bottom button')]
      .find(b => b.textContent === '↶');
    const вернуть = [...win.body.querySelectorAll('.snip-bottom button')]
      .find(b => b.textContent === '↷');
    отменить.click();
    const послеОтмены = состояние.штрихи.length;
    вернуть.click();
    const послеВозврата = состояние.штрихи.length;

    /* Сохранение: без системы пишем в свою файловую систему */
    const сохранить = [...win.body.querySelectorAll('.snip-bottom button')]
      .find(b => /Сохранить/.test(b.textContent));
    сохранить.click();
    await new Promise(r => setTimeout(r, 400));
    const папка = FS.node(['Изображения', 'Снимки']);
    const файлов = папка && папка.children ? Object.keys(папка.children).length : 0;

    WM.close(win);
    return { есть:true, пустоВидно, холстСпрятан, кадрВидно,
             было, послеОтмены, послеВозврата, файлов };
  });

  check('Ножницы честно говорят, что снимка ещё нет',
    ножницы.есть && ножницы.пустоВидно && ножницы.холстСпрятан, JSON.stringify(ножницы));
  check('полученный кадр показывается',
    ножницы.кадрВидно === true, JSON.stringify(ножницы));
  check('разметку можно отменить и вернуть',
    ножницы.послеОтмены === ножницы.было - 1 && ножницы.послеВозврата === ножницы.было,
    JSON.stringify(ножницы));
  check('снимок сохраняется в Изображения → Снимки',
    ножницы.файлов === 1, JSON.stringify(ножницы));

  /* --- значки программ на рабочем столе --- */
  const наСтоле = await page.evaluate(async () => {
    S.deskApps = []; Store.save(); Shell.renderIcons();
    const было = document.querySelectorAll('#desktop-icons .di').length;

    /* Выносим через то же меню, которым пользуется человек */
    Shell.allApps(true);
    await new Promise(r => setTimeout(r, 250));
    const строка = [...document.querySelectorAll('#start-results .all-row')]
      .find(b => /Калькулятор/.test(b.textContent));
    if (!строка) return { есть:false };
    строка.dispatchEvent(new MouseEvent('contextmenu', { bubbles:true, clientX:100, clientY:100 }));
    await new Promise(r => setTimeout(r, 200));
    const пункт = [...document.querySelectorAll('#ctx button')]
      .find(n => /Вынести на рабочий стол/.test(n.textContent || ''));
    if (!пункт) return { есть:true, меню:false };
    пункт.click();
    await new Promise(r => setTimeout(r, 250));

    const стало = document.querySelectorAll('#desktop-icons .di').length;
    const подпись = [...document.querySelectorAll('#desktop-icons .di .lbl')]
      .some(n => n.textContent === 'Калькулятор');
    const вПамяти = (S.deskApps || []).includes('calc');

    /* И обратно: значок должен убираться */
    S.deskApps = []; Store.save(); Shell.renderIcons();
    const вернулось = document.querySelectorAll('#desktop-icons .di').length;
    Shell.closePanels();
    return { есть:true, меню:true, было, стало, подпись, вПамяти, вернулось };
  });

  check('программу можно вынести на рабочий стол из Пуска',
    наСтоле.меню === true && наСтоле.стало === наСтоле.было + 1 && наСтоле.подпись === true,
    JSON.stringify(наСтоле));
  check('вынесенное запоминается и убирается обратно',
    наСтоле.вПамяти === true && наСтоле.вернулось === наСтоле.было,
    JSON.stringify(наСтоле));

  /* --- встречающего файла на столе больше нет --- */
  check('на рабочем столе нет нашей записки',
    await page.evaluate(() => !FS.node(['Рабочий стол', 'Начало.txt'])));

  /* --- в настройке не спрашивают пароль от снятого замка --- */
  check('первоначальная настройка не спрашивает пароль',
    await page.evaluate(() => !Setup.STEPS.some(ш => /Пароль/.test(ш.title || ''))));

  /* Проверка «панель видит открытое» здесь больше не нужна: панель ушла
     на свою страницу и списка того, что её растягивает, у стола не
     осталось. Вместо неё — то, что у стола осталось: он должен верно
     говорить панели, занят ли экран им самим. */
  for (const [что, готовь] of [
    ['заставке',  () => { const б = el('div'); б.id = 'boot'; document.body.appendChild(б); }],
    ['мастере',   () => { const б = el('div'); б.id = 'setup'; document.body.appendChild(б); }],
    ['приветствии', () => { const б = el('div', 'welcome'); document.body.appendChild(б); }],
    ['входе',     () => { const б = el('div', 'вход'); document.body.appendChild(б); }]
  ]){
    const занят = await page.evaluate(([имя]) => {
      document.querySelectorAll('#boot, #setup, .welcome, .вход').forEach(н => н.remove());
      const готовь = {
        'заставке':   () => { const б = document.createElement('div'); б.id = 'boot'; document.body.appendChild(б); },
        /* С теми же признаками, с какими они живут на самом деле: и мастер,
           и приветствие до «on» прозрачны, а прозрачное экран не
           закрывает — стол это теперь различает. */
        'мастере':    () => { const б = document.createElement('div'); б.id = 'setup';
                              б.className = 'setup on'; document.body.appendChild(б); },
        'приветствии':() => { const б = document.createElement('div'); б.className = 'welcome on'; document.body.appendChild(б); },
        'входе':      () => { const б = document.createElement('div'); б.className = 'вход'; document.body.appendChild(б); }
      }[имя];
      готовь();
      const есть = Поверхности.прячемся();
      document.querySelectorAll('#boot, #setup, .welcome, .вход').forEach(н => н.remove());
      return есть;
    }, [что]);
    check('стол знает, что экран занят при ' + что, занят);
  }

  /* Обратная сторона того же: невидимый остаток экран не закрывает.

     У человека панель пропала целиком и навсегда, а стол при этом работал.
     Виноват был остаток в разметке: прозрачный, никому не видный — и стол
     до конца сеанса докладывал «занято». Панель честно пряталась и ждала
     слова, которого уже никто не скажет. */
  const остаток = await page.evaluate(() => {
    document.querySelectorAll('#boot, #setup, .welcome, .вход').forEach(н => н.remove());
    const б = document.createElement('div'); б.className = 'welcome';  // без «on» — прозрачный
    document.body.appendChild(б);
    document.querySelector('#desktop').classList.add('on');
    const есть = Поверхности.прячемся();
    б.remove();
    return есть;
  });
  check('невидимый остаток не прячет панель', остаток === false, 'стол сказал: ' + остаток);

  /* --- обзор: меню программ во весь экран --- */
  const обзор = await page.evaluate(async () => {
    /* Сперва дать прежним окнам закрыться до конца: закрытие с
       анимацией, и открытая заново одинокая программа вернула бы то же
       окно, которое в этот миг ещё уезжает. */
    WM.wins.slice().forEach(w => WM.close(w));
    await new Promise(r => setTimeout(r, 500));
    WM.open('calc');
    await new Promise(r => setTimeout(r, 300));
    Shell.обзор(true);
    await new Promise(r => setTimeout(r, 200));
    const узел = document.querySelector('#обзор');
    const было = {
      открыт: узел.classList.contains('on'),
      столы:  узел.querySelectorAll('.об-стол').length,
      окна:   узел.querySelectorAll('.об-окно').length,
      всего:  узел.querySelectorAll('.об-прог').length
    };
    /* Поиск: пишем в поле так же, как человек, и смотрим, что осталось. */
    const поле = document.querySelector('#об-искать');
    поле.value = 'калькул';
    поле.dispatchEvent(new Event('input'));
    await new Promise(r => setTimeout(r, 150));
    было.нашлось = [...узел.querySelectorAll('.об-прог .имя')].map(н => н.textContent);
    Shell.обзор(false);
    await new Promise(r => setTimeout(r, 150));
    было.закрылся = !узел.classList.contains('on');
    WM.wins.slice().forEach(w => WM.close(w));
    return было;
  });
  check('обзор открывается во весь экран', обзор.открыт === true);
  check('в обзоре наверху все рабочие столы',
    обзор.столы === await page.evaluate(() => WM.desks), 'столов: ' + обзор.столы);
  check('в обзоре видно открытое окно', обзор.окна === 1, 'окон: ' + обзор.окна);
  check('в обзоре все программы сразу',
    обзор.всего >= Object.keys(await page.evaluate(() => APPS)).length - 2,
    'в сетке: ' + обзор.всего);
  check('поиск в обзоре отбирает по имени',
    обзор.нашлось.length === 1 && /Калькулятор/.test(обзор.нашлось[0]),
    JSON.stringify(обзор.нашлось));
  check('обзор закрывается', обзор.закрылся === true);

  /* Развёрнутое окно в раздельном режиме: сверху ровно под полосой, снизу
     до самого края.

     Обе половины пришли с настоящей машины. Заголовок развёрнутой
     программы уходил под полосу, и ухватить его было нечем: наша страница
     во весь экран, а полоса живёт на своей поверхности сверху, и оконный
     сервер нашу страницу ей не подчиняет — отступ обязаны держать мы сами.
     Снизу же оставалась лента обоев: место держалось под док, который в
     этом режиме и так прячется, стоит открыть окно. */
  {
    const r = await page.evaluate(async () => {
      document.body.classList.add('поверхности-разделены');
      document.documentElement.style.setProperty('--полоса-сверху', '30px');
      WM.wins.slice().forEach(w => WM.close(w));
      await new Promise(r2 => setTimeout(r2, 300));
      const w = WM.open('notepad');
      if (!w.maximized) WM.toggleMax(w);
      await new Promise(r2 => setTimeout(r2, 800));
      /* Док в этот миг и так превращается в панель задач, но проверка не
         должна зависеть от того, успел ли он: ставим признак сами, иначе
         нижняя половина проверки ничего не проверяет. */
      document.body.classList.add('taskbar');
      const м = WM.maxRect();
      Object.assign(w.node.style, { top:м.top + 'px', height:м.height + 'px' });
      await new Promise(r2 => setTimeout(r2, 200));
      const б = w.node.getBoundingClientRect();
      return { верх:Math.round(б.top), низ:Math.round(innerHeight - б.bottom),
               панельЗадач:document.body.classList.contains('taskbar') };
    });
    check('развёрнутое окно начинается под полосой', r.верх === 30, JSON.stringify(r));
    check('и доходит до самого низа', r.низ <= 1, JSON.stringify(r));
    await page.evaluate(() => {
      document.body.classList.remove('поверхности-разделены', 'taskbar');
      document.documentElement.style.removeProperty('--полоса-сверху');
      WM.wins.slice().forEach(w => WM.close(w));
    });
    await page.waitForTimeout(300);
  }

  /* Док прячется не только от наших окон.
  
     Человек сказал прямо: нижняя панель не исчезает, когда открыт Firefox.
     И верно: Firefox — настоящее окно оконного сервера, в нашем списке окон
     его нет, и прятаться было не от чего. Признак чужих окон ставит опрос
     оконного сервера; здесь ставим его руками и проверяем, что док на него
     смотрит. */
  {
    const p2 = await browser.newPage({ viewport:{ width:1280, height:800 } });
    await p2.addInitScript(() => { try { localStorage.setItem('glower.setup.done', 'true'); } catch(e){} });
    await p2.goto(URL_APP + '?раздельно=1');
    await p2.waitForTimeout(2600);
    await p2.keyboard.press('Enter');
    await p2.waitForTimeout(1200);
    const своё = await p2.evaluate(async () => {
      WM.open('notepad');
      await new Promise(r => setTimeout(r, 1200));
      return document.body.classList.contains('док-прячется');
    });
    check('док прячется от своего окна', своё);
    const чужое = await p2.evaluate(async () => {
      WM.wins.slice().forEach(w => WM.close(w));
      await new Promise(r => setTimeout(r, 1200));
      const без = document.body.classList.contains('док-прячется');
      document.body.classList.add('чужие-окна');
      await new Promise(r => setTimeout(r, 1200));
      return { без, с:document.body.classList.contains('док-прячется') };
    });
    check('док прячется и от чужого окна', чужое.с && !чужое.без, JSON.stringify(чужое));
    await p2.close();
  }

  /* Часовой пояс должен доходить до всего, что показывает время. Виджет
     часов на столе его не знал — и, хуже того, вообще не шёл: обновление
     искало его по идентификатору, а рисовался он с классом, да ещё второй
     таймер писал туда время машины. */
  {
    const r = await page.evaluate(async () => {
      Store.set('tz', 'Asia/Tokyo');
      await new Promise(r2 => setTimeout(r2, 1400));
      const узел = document.querySelector('.dw-clock') || document.querySelector('#dw-clock');
      const здесь = new Date();
      const там = new Date(здесь.toLocaleString('en-US', { timeZone:'Asia/Tokyo' }));
      const надо = String(там.getHours()).padStart(2, '0') + ':' + String(там.getMinutes()).padStart(2, '0');
      const было = узел ? узел.textContent.trim() : '(нет виджета)';
      Store.set('tz', '');
      /* Минута может смениться между чтением узла и вычислением ожидаемого
         времени — тогда проверка падала бы на ровном месте. Сравниваем с
         допуском в минуту: нас интересуют часы, а не секундомер. */
      const вмин = т => { const [ч, м] = т.split(':').map(Number);
        return Number.isFinite(ч) ? ч * 60 + м : null; };
      const a2 = вмин(было), b2 = вмин(надо);
      return { было, надо, разный:там.getHours() !== здесь.getHours(),
               сошлось:a2 !== null && Math.abs(a2 - b2) <= 1 };
    });
    check('виджет часов показывает выбранный часовой пояс',
      !r.разный || r.сошлось, JSON.stringify(r));
  }

  /* «Экономия ресурсов» обещала отключать размытие и не отключала ничего,
     кроме скорости анимаций. На слабой машине размытие — самое дорогое,
     что есть на экране. */
  {
    const r = await page.evaluate(async () => {
      KV.set('ecoMode', true); applySettings();
      await new Promise(r2 => setTimeout(r2, 300));
      const окно = document.querySelector('.glass') || document.body;
      const из = getComputedStyle(окно);
      const скорость = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--speed'));
      /* Фон должен остаться плотным: без размытия сквозь полупрозрачное
         меню читается окно под ним — я это уже сделал и откатил. */
      const фон = из.backgroundColor;
      const альфа = (фон.match(/rgba?\([^)]*,\s*([\d.]+)\s*\)/) || [])[1];
      KV.set('ecoMode', false); applySettings();
      return { размытие:из.backdropFilter, скорость, альфа:альфа === undefined ? 1 : +альфа };
    });
    check('щадящий режим убирает размытие', /none/.test(r.размытие || 'none'), JSON.stringify(r));
    check('и ускоряет переходы', r.скорость <= 0.6, JSON.stringify(r));
    check('а панели остаются непрозрачными', r.альфа >= 0.9, JSON.stringify(r));
  }

  /* Установочная среда никогда не показывает пустой экран.
  
     Там намеренно нет рабочего стола — только мастер. Но пока мастер
     решает, можно ли ставить (а он переспрашивает машину до пяти минут),
     на экране не было ничего: чёрный прямоугольник и курсор. Если же
     опознание не удавалось, чёрным оно оставалось навсегда — человек сидит
     перед машиной и не знает, ждать или перезагружаться. */
  {
    const p3 = await browser.newPage({ viewport:{ width:1024, height:700 } });
    await p3.goto(URL_APP + '?install=1');
    await p3.waitForTimeout(3000);
    const r = await p3.evaluate(() => {
      const ж = document.querySelector('#уст-ждём');
      const окно = document.querySelector('.win');
      const видно = н => n2(н) && н.textContent.trim().length > 3;
      function n2(н){ return н && н.getBoundingClientRect().width > 200; }
      return { ждём:видно(ж), мастер:n2(окно),
               текст:ж ? ж.textContent.replace(/\s+/g, ' ').trim().slice(0, 40) : '' };
    });
    check('установочная среда не оставляет пустой экран', r.ждём || r.мастер, JSON.stringify(r));
    await p3.close();
  }

  /* Магазин занят настоящими программами Linux.
  
     Половину его занимал наш собственный каталог: помидорный таймер, 2048,
     конвертер и палитра. Четыре примера, написанные, чтобы показать, что
     приложения бывают, — и они же занимали место того, ради чего Магазин
     открывают. Каталога больше нет, и вернуться он не должен. */
  {
    const r = await page.evaluate(async () => {
      WM.wins.slice().forEach(w => WM.close(w));
      const w = WM.open('store');
      await new Promise(r2 => setTimeout(r2, 900));
      const т = w.node.textContent;
      const к = п => w.node.querySelectorAll(п).length;
      const был = ['pomodoro', '2048', 'conv', 'palette'].filter(id => APPS[id]);
      /* Считаем до перехода: после него на экране уже страница программы,
         и витрины с плитками там, разумеется, нет. Первая копия этой
         проверки считала после — и падала, ничего не поймав. */
      const было = { разделов:к('.ms-rail-i'), витрина:к('.ms-hero-card'),
                     кругляшей:к('.ms-chip'), строк:к('.ms-row') };
      /* переход на страницу программы: строка — это вход, а не кнопка */
      w.node.querySelector('.ms-row').click();
      await new Promise(r2 => setTimeout(r2, 500));
      const страница = !!w.node.querySelector('.st-page');
      return { ...было, был, страница, игрушки:/Помидор|Палитра|Конвертер/.test(т) };
    });
    check('слева полоса значков', r.разделов === 6, JSON.stringify(r));
    check('на главной витрина из четырёх карточек и полоса разделов',
      r.витрина === 4 && r.кругляшей >= 6 && r.строк >= 8, JSON.stringify(r));
    check('карточка открывает страницу программы', r.страница, JSON.stringify(r));
    check('наших игрушек в системе больше нет',
      !r.игрушки && !r.был.length, JSON.stringify(r));
  }

  check('в консоли нет ошибок JS', jsErrors.length === 0, jsErrors.slice(0, 3).join(' | '));

} catch (e){
  failed++;
  results.push('  ❌ упало с исключением — ' + e.message);
} finally {
  await browser.close();
}

console.log('\nСмоук-тесты GlowerOS\n');
console.log(results.join('\n'));
console.log(`\n  Пройдено: ${passed} · Провалено: ${failed}\n`);

/* Провалы повторяем в самом конце — после итога.
   Список тестов печатается одной простыней выше, и стоит посмотреть вывод
   через tail, как имя упавшего теста срезается: остаётся голое «Провалено: 1»
   и никакой зацепки. Одно такое падение из десяти прогонов у нас уже
   пропало без следа. Повтор внизу стоит три строки и переживает любую
   обрезку. */
if (failed){
  console.log('  Что не прошло:');
  console.log(results.filter(с => с.includes('\u274c')).join('\n'));
  console.log('');
}

process.exit(failed ? 1 : 0);
