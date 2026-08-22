#!/usr/bin/env node
/* ==========================================================================
   Проверка переноса на Linux: оболочка как система машины.

   Запуск:  node test/linux.mjs
   Поднимает агент с системным слоем и убеждается, что оболочка видит
   настоящие процессы, программы и возможности машины — и что опасное
   остаётся закрытым, пока его явно не разрешили.
   ========================================================================== */
import { spawn } from 'node:child_process';
import { mkdtemp, readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir, hostname, totalmem } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const PORT = 8700 + Math.floor(Math.random() * 200);

let chromium;
try { ({ chromium } = await import(pathToFileURL('/opt/node22/lib/node_modules/playwright/index.mjs').href)); }
catch(e){ try { ({ chromium } = await import('playwright')); } catch(e2){
  console.error('Нужен playwright: npm i -D playwright'); process.exit(2); } }
const exe = ['/opt/pw-browsers/chromium-1194/chrome-linux/chrome', '/usr/bin/chromium', '/usr/bin/google-chrome']
  .find(p => existsSync(p));

let passed = 0, failed = 0;
const out = [];
const check = (n, c, d = '') => { c ? (passed++, out.push('  ✅ ' + n))
  : (failed++, out.push('  ❌ ' + n + (d ? ' — ' + d : ''))); };

if (process.platform !== 'linux'){
  console.log('\n  Эта проверка имеет смысл только на Linux — пропущена.\n');
  process.exit(0);
}

const WS = await mkdtemp(join(tmpdir(), 'glower-'));
const agent = spawn(process.execPath,
  [join(root, 'agent/server.mjs'), '--port', String(PORT), '--root', WS, '--system'],
  { stdio:['ignore', 'pipe', 'pipe'] });
let agentLog = '';
agent.stdout.on('data', d => agentLog += d);
await new Promise(r => setTimeout(r, 1300));

const browser = await chromium.launch(exe ? { executablePath:exe, args:['--no-sandbox'] } : { args:['--no-sandbox'] });
const page = await browser.newPage({ viewport:{ width:1280, height:820 } });
/* мастер первого запуска проверяется в смоук-тестах — здесь он только мешал бы */
await page.addInitScript(() => {
  try { localStorage.setItem('win12.setup.done', 'true'); } catch(e){}
});
const errs = [];
page.on('pageerror', e => errs.push(e.message));

try {
  await page.goto(`http://localhost:${PORT}/`);
  await page.waitForTimeout(2600);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(2500);

  check('оболочка запустилась и подключилась к машине',
    await page.evaluate(() => OS.on()), await page.evaluate(() => Platform.mode + '/' + !!OS.caps));

  check('машина опознана правильно',
    await page.evaluate(() => OS.caps.platform === 'linux' && !!OS.caps.host) &&
    (await page.evaluate(() => OS.caps.host)) === hostname());

  /* --- настоящие процессы --- */
  const procs = await page.evaluate(() => OS.procs());
  check('видны настоящие процессы машины',
    procs.total > 5 && procs.list.some(p => p.pid === 1), 'всего ' + procs.total);
  check('память машины совпадает с настоящей',
    Math.abs(procs.mem.total - totalmem()) < 1024, procs.mem.total + ' против ' + totalmem());
  check('свой же процесс агента виден в списке',
    procs.list.some(p => p.name.includes('node')));

  /* --- установленные программы --- */
  const apps = await page.evaluate(() => OS.apps());
  const desktopFiles = existsSync('/usr/share/applications');
  check('список программ машины прочитан из .desktop',
    desktopFiles ? apps.total >= 0 && Array.isArray(apps.list) : true, 'найдено ' + apps.total);
  check('запуск программ выключен, пока не разрешён ключом',
    apps.canLaunch === false);

  const launch = await page.evaluate(async () => {
    try { await OS.launch('gedit.desktop'); return 'запустил'; } catch(e){ return e.message; }
  });
  check('без --allow-launch программы не запускаются', /--allow-launch/.test(launch), launch);

  /* --- питание --- */
  const power = await page.evaluate(async () => {
    try { await OS.power('poweroff'); return 'выключил'; } catch(e){ return e.message; }
  });
  check('без --allow-power машину не выключить', /--allow-power/.test(power), power);

  /* --- честность про отсутствующие возможности --- */
  const vol = await page.evaluate(() => OS.volume());
  check('громкость: либо настоящее значение, либо честная причина',
    (typeof vol.volume === 'number' && vol.via) || (vol.volume === null && !!vol.reason),
    JSON.stringify(vol));

  const br = await page.evaluate(() => OS.brightness());
  check('яркость: либо настоящая подсветка, либо честная причина',
    (typeof br.value === 'number' && br.device) || (br.value === null && !!br.reason),
    JSON.stringify(br));

  /* --- произвольные команды по-прежнему невозможны --- */
  const exec = await page.evaluate(async () => {
    try { await Platform.rpc('sys.exec', { cmd:'id' }); return 'выполнил'; } catch(e){ return e.message; }
  });
  check('произвольные команды агенту недоступны', /нет такого метода/.test(exec), exec);

  const bad = await page.evaluate(async () => {
    try { await Platform.rpc('sys.launch', { id:'../../etc/passwd' }); return 'принял'; } catch(e){ return e.message; }
  });
  check('подложить путь вместо имени программы не выйдет',
    /неверный идентификатор|--allow-launch/.test(bad), bad);

  /* --- диспетчер задач показывает машину --- */
  check('диспетчер задач показывает процессы машины',
    await page.evaluate(async () => {
      WM.open('taskmgr');
      await new Promise(r => setTimeout(r, 900));
      const w = WM.wins.find(x => x.appId === 'taskmgr');
      const b = [...w.body.querySelectorAll('.btn')].find(x => x.textContent.includes('Процессы машины'));
      if (!b) return false;
      b.click();
      await new Promise(r => setTimeout(r, 1200));
      const t = w.body.innerText;
      return /Средняя нагрузка/.test(t) && /systemd|node|init/.test(t);
    }));

  check('приложение «Программы машины» появилось только в этом режиме',
    await page.evaluate(() => !!APPS.native));

  /* --- настройки говорят о машине, а не о браузере --- */
  const devText = await page.evaluate(async () => {
    WM.open('settings');
    await new Promise(r => setTimeout(r, 700));
    const w = WM.wins.find(x => x.appId === 'settings');
    const nav = [...w.body.querySelectorAll('.set-nav button, .nav-item, button')]
      .find(b => /Устройства и датчики/.test(b.textContent));
    if (nav) nav.click();
    await new Promise(r => setTimeout(r, 1400));
    return w.body.innerText;
  });

  check('оборудование показано по данным машины, а не по оценке браузера',
    /Процессор/.test(devText) && !/округлено браузером/.test(devText), devText.slice(0, 200));
  check('память названа настоящим объёмом',
    devText.includes((totalmem() / 1073741824).toFixed(1) + ' ГБ'),
    devText.match(/Оперативная память[\s\S]{0,40}/));
  check('вместо «Платформа: Linux» — система и её ядро',
    /Система/.test(devText) && new RegExp(process.platform === 'linux' ? 'ядро Linux' : '.').test(devText));
  check('строки про мобильное устройство и hover убраны',
    !/Мобильное устройство/.test(devText) && !/hover/i.test(devText));
  check('про Web Bluetooth оболочка больше не оправдывается',
    !/Web Bluetooth/.test(devText) && /Bluetooth/.test(devText), devText.slice(0, 200));

  const sysText = await page.evaluate(async () => {
    const w = WM.wins.find(x => x.appId === 'settings');
    const nav = [...w.body.querySelectorAll('.set-nav button, .nav-item, button')]
      .find(b => /^\s*.?\s*Система\s*$/.test(b.textContent));
    if (nav) nav.click();
    await new Promise(r => setTimeout(r, 900));
    return w.body.innerText;
  });
  check('внутренняя кухня агента убрана из настроек настоящей системы',
    !/Подключено к системе/.test(sysText) && !/Перечитать диск/.test(sysText), sysText.slice(0, 200));

  /* --- встроенный браузер --- */
  const web = await page.evaluate(async () => {
    const out = {};
    WM.open('browser');
    await new Promise(r => setTimeout(r, 800));
    const w = WM.wins.find(x => x.appId === 'browser');
    out.старт = !!w.body.querySelector('.br-start');

    const i = w.body.querySelector('.br-bar input');
    i.value = 'about:blank';
    i.dispatchEvent(new KeyboardEvent('keydown', { key:'Enter', bubbles:true }));
    for (let k = 0; k < 60 && !w._web.tabs[0].view; k++) await new Promise(r => setTimeout(r, 200));
    const v = w._web.tabs[0].view;
    out.движок = !!v;
    if (!v) return out;
    for (let k = 0; k < 40 && !w.body.querySelector('.br-frame').src; k++) await new Promise(r => setTimeout(r, 200));
    out.кадр = (w.body.querySelector('.br-frame').src || '').startsWith('data:image/jpeg');

    /* ставим поле и кнопку прямо на страницу движка и работаем с ними как человек */
    await v.cdp.send('Runtime.evaluate', { expression:
      `document.body.innerHTML = '<input id=p style="position:fixed;left:10px;top:10px;width:300px;height:40px">' +
        '<button id=b style="position:fixed;left:10px;top:70px;width:200px;height:40px">кнопка</button>' +
        '<div style="height:4000px"></div>';
       window.__c = 0; document.getElementById('b').onclick = () => window.__c++;
       document.getElementById('p').focus(); 'ok'`, returnByValue:true }, v.sid);

    const r = v.node.getBoundingClientRect();
    for (const ch of 'привет'){
      v.node.dispatchEvent(new KeyboardEvent('keydown', { key:ch, bubbles:true, cancelable:true }));
      v.node.dispatchEvent(new KeyboardEvent('keyup', { key:ch, bubbles:true, cancelable:true }));
    }
    v.node.dispatchEvent(new MouseEvent('mousedown', { clientX:r.left + 100, clientY:r.top + 90, button:0, buttons:1, detail:1, bubbles:true, cancelable:true }));
    v.node.dispatchEvent(new MouseEvent('mouseup', { clientX:r.left + 100, clientY:r.top + 90, button:0, buttons:0, detail:1, bubbles:true, cancelable:true }));
    v.node.dispatchEvent(new WheelEvent('wheel', { deltaY:400, clientX:r.left + 200, clientY:r.top + 200, bubbles:true, cancelable:true }));
    await new Promise(r2 => setTimeout(r2, 900));

    const got = await v.cdp.send('Runtime.evaluate', { expression:
      'JSON.stringify({ t:document.getElementById("p").value, c:window.__c, y:window.scrollY })',
      returnByValue:true }, v.sid);
    Object.assign(out, JSON.parse(got.result.value));

    /* новая вкладка по Ctrl+T */
    w.body.querySelector('.app').dispatchEvent(
      new KeyboardEvent('keydown', { key:'t', ctrlKey:true, bubbles:true, cancelable:true }));
    out.вкладок = w._web.tabs.length;
    return out;
  });

  check('браузер открывается на своей начальной странице', web.старт);
  check('движок машины поднялся и отдал кадр', web.движок && web.кадр, JSON.stringify(web));
  check('клавиатура доходит до страницы', web.t === 'привет', String(web.t));
  check('щелчок доходит до страницы', web.c === 1, String(web.c));
  check('колесо прокручивает страницу', web.y > 0, String(web.y));
  check('Ctrl + T открывает вкладку', web.вкладок === 2, String(web.вкладок));

  /* --- Wi-Fi --- */
  const wifi = await page.evaluate(() => OS.wifiState().then(x => x, e => ({ err:e.message })));
  check('система честно отвечает про Wi-Fi этой машины',
    typeof wifi.supported === 'boolean' && (wifi.supported || !!wifi.reason), JSON.stringify(wifi));
  check('без --allow-net сети остаются закрытыми', wifi.allowed === false, JSON.stringify(wifi));

  const wifiScan = await page.evaluate(() => Platform.rpc('sys.wifi.scan')
    .then(() => 'просканировал', e => e.message));
  check('без --allow-net к сетям не подключиться', /--allow-net/.test(wifiScan), wifiScan);

  /* --- выключение: проверяем без последствий, что машину есть чем гасить --- */
  const powerWays = await page.evaluate(() => Platform.rpc('sys.power.check'));
  check('система знает, чем выключать машину, и честно об этом отчитывается',
    Array.isArray(powerWays.ways) && powerWays.ways.length >= 2 &&
    powerWays.ways.every(w => w.ok || (w.why && w.why.length > 3)),
    JSON.stringify(powerWays));
  check('без --allow-power выключение остаётся закрытым', powerWays.allowed === false);

  /* --- установка на диск: без ключа она закрыта, и это видно --- */
  const insCan = await page.evaluate(() => Install.can());
  check('установка выключена, пока её не разрешили ключом',
    insCan.allowed === false && /--allow-install/.test(insCan.reason || ''), JSON.stringify(insCan));

  const insStart = await page.evaluate(() => Platform.rpc('install.start', { disk:'/dev/sda' })
    .then(() => 'запустилась', e => e.message));
  check('без --allow-install диск не трогают', /--allow-install/.test(insStart), insStart);

  const insDisks = await page.evaluate(() => Install.disks().then(d => d, e => ({ err:e.message })));
  check('список дисков читается из /sys',
    Array.isArray(insDisks.list) && insDisks.list.every(d => d.dev.startsWith('/dev/')),
    JSON.stringify(insDisks).slice(0, 200));

  check('мастер установки не появляется там, где установка невозможна',
    await page.evaluate(() => !APPS.installer));

  /* --- запуск программ машины: без gio тоже должно работать --- */
  {
    const { mkdir, writeFile } = await import('node:fs/promises');
    const { homedir } = await import('node:os');
    const метка = join(WS, 'запущено.txt');
    const каталог = join(homedir(), '.local/share/applications');
    await mkdir(каталог, { recursive:true });
    await writeFile(join(каталог, 'glower-проверка.desktop'),
      '[Desktop Entry]\nType=Application\nName=Проверка запуска\n' +
      'Exec=/usr/bin/touch ' + метка + ' %U\n');

    const виден = await page.evaluate(() => OS.apps().then(d =>
      d.list.some(a => a.name === 'Проверка запуска'), () => false));
    check('ярлык программы машины виден системе', виден);

    const пуск = await page.evaluate(() => Platform.rpc('sys.launch', { id:'glower-проверка.desktop' })
      .then(r => r.via, e => 'ошибка: ' + e.message));
    check('без --allow-launch запуск закрыт', /--allow-launch/.test(String(пуск)), String(пуск));
  }

  /* --- программы Linux: поиск открыт, установка под ключом --- */
  const pkgState = await page.evaluate(() => Platform.rpc('pkg.state').then(x => x, e => ({ err:e.message })));
  check('система знает, чем ставить программы',
    pkgState.apt === true && pkgState.allowed === false, JSON.stringify(pkgState));
  check('без --allow-packages установка закрыта, и причина названа',
    /--allow-packages/.test(pkgState.reason || ''), String(pkgState.reason));

  const pkgFind = await page.evaluate(() => Platform.rpc('pkg.search', { query:'coreutils' })
    .then(r => r.list.length, e => 'ошибка: ' + e.message));
  check('поиск по репозиториям работает и без разрешения на установку',
    typeof pkgFind === 'number' && pkgFind > 0, String(pkgFind));

  const pkgTry = await page.evaluate(() => Platform.rpc('pkg.install', { name:'htop' })
    .then(() => 'запустилось', e => e.message));
  check('без ключа программу не поставить', /--allow-packages/.test(pkgTry), String(pkgTry));

  const pkgBad = await page.evaluate(() => Platform.rpc('pkg.install', { name:'--reinstall' })
    .then(() => 'приняло', e => e.message));
  check('вместо имени пакета ключ подсунуть нельзя',
    /недопустимое имя|--allow-packages/.test(pkgBad), String(pkgBad));

  check('агент сообщил о системном слое при запуске', /Системный слой:\s+включён/.test(agentLog));

  check('в консоли нет ошибок JS', errs.length === 0, errs.slice(0, 2).join(' | '));

} catch(e){
  failed++; out.push('  ❌ упало с исключением — ' + e.message);
} finally {
  await browser.close();
  agent.kill();
}

console.log('\nПеренос на Linux: оболочка как система машины\n');
console.log(out.join('\n'));
console.log(`\n  Пройдено: ${passed} · Провалено: ${failed}\n`);
process.exit(failed ? 1 : 0);
