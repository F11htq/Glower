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
  check('терминал выполняет команды', /^42$/m.test(term) && /Windows 12 Prototype/.test(term));

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
  const store = await page.evaluate(() => {
    AppStore.install('g2048');
    const installed = !!APPS.g2048 && S.pinned.includes('g2048');
    AppStore.installPkg({ id:'smoke-app', name:'Смоук', glyph:'🧪',
      code:"win.body.appendChild(api.el('div','pad','ok'));" });
    return { installed, custom:!!APPS['smoke-app'] };
  });
  check('приложение из каталога устанавливается', store.installed);
  check('свой пакет устанавливается', store.custom);
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
  await page.waitForTimeout(2500); await page.keyboard.press('Enter'); await page.waitForTimeout(2200);
  check('окна восстанавливаются после перезагрузки',
    await page.evaluate(() => ['calc','clock'].every(a => WM.wins.some(w => w.appId === a))),
    await page.evaluate(() => WM.wins.map(w => w.appId).join(',')));
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
  await page.keyboard.press('Enter'); await page.waitForTimeout(400);
  check('без пароля вход закрыт', await page.evaluate(() => !$('#lock').classList.contains('gone')));
  await page.fill('.lock-pw input', 'wrong'); await page.keyboard.press('Enter'); await page.waitForTimeout(400);
  check('неверный пароль отклоняется', await page.$eval('.lock-err', n => n.textContent.length > 0));
  await page.fill('.lock-pw input', 'pass123'); await page.keyboard.press('Enter'); await page.waitForTimeout(800);
  check('верный пароль пускает', await page.evaluate(() => $('#lock').classList.contains('gone')));

  const guest = await page.evaluate(() => Profiles.list().find(p => p.name === 'Гость').id);
  await page.evaluate(id => Profiles.switchTo(id), guest);
  await page.waitForTimeout(2700); await page.keyboard.press('Enter'); await page.waitForTimeout(700);
  check('данные профилей изолированы',
    await page.evaluate(() => S.userName === 'Гость' && !FS.node(['Документы', 'личное.txt'])));

  /* --- журнал ошибок --- */
  check('в консоли нет ошибок JS', jsErrors.length === 0, jsErrors.slice(0, 3).join(' | '));

} catch (e){
  failed++;
  results.push('  ❌ упало с исключением — ' + e.message);
} finally {
  await browser.close();
}

console.log('\nСмоук-тесты Windows 12 Prototype\n');
console.log(results.join('\n'));
console.log(`\n  Пройдено: ${passed} · Провалено: ${failed}\n`);
process.exit(failed ? 1 : 0);
