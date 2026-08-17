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

  /* --- темы --- */
  for (const t of ['dark', 'light', 'glass']){
    await page.evaluate(x => Store.set('theme', x), t);
    await page.waitForTimeout(250);
  }
  check('темы переключаются без ошибок', await page.evaluate(() => S.theme === 'glass'));
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
