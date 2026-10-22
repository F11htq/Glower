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
import { existsSync, statSync } from 'node:fs';
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
/* Настоящий сеанс говорит агенту, каким движком рисовать страницы, — здесь
   делаем то же самое. Раньше этого не было, и проверки встроенного браузера
   падали не из-за системы, а из-за того, что в проверочной машине нет
   команды chromium в путях. */
/* Корзина — своя, проверочная: настоящую корзину машины не трогаем */
const КОРЗИНА = await mkdtemp(join(tmpdir(), 'glower-trash-'));
const agent = spawn(process.execPath,
  [join(root, 'agent/server.mjs'), '--port', String(PORT), '--root', WS, '--system'],
  { stdio:['ignore', 'pipe', 'pipe'],
    env:{ ...process.env, GLOWER_BROWSER:process.env.GLOWER_BROWSER || exe || '', GLOWER_TRASH:КОРЗИНА } });
let agentLog = '';
agent.stdout.on('data', d => agentLog += d);
await new Promise(r => setTimeout(r, 1300));

const browser = await chromium.launch(exe ? { executablePath:exe, args:['--no-sandbox'] } : { args:['--no-sandbox'] });
const page = await browser.newPage({ viewport:{ width:1280, height:820 } });
/* мастер первого запуска проверяется в смоук-тестах — здесь он только мешал бы */
await page.addInitScript(() => {
  try { localStorage.setItem('glower.setup.done', 'true'); } catch(e){}
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
  check('диспетчер задач сразу показывает процессы машины',
    await page.evaluate(async () => {
      WM.open('taskmgr');
      await new Promise(r => setTimeout(r, 1200));
      const w = WM.wins.find(x => x.appId === 'taskmgr');
      const t = w.body.innerText;
      const итог = /Средняя нагрузка/.test(t) && /systemd|node|init|chrome/.test(t);
      WM.close(w);
      return итог;
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

  /* --- как именно запускаются программы: коротким flatpak и вручную --- */
  {
    const { mkdir, writeFile, symlink, rm } = await import('node:fs/promises');
    const бин = join(WS, 'бин');
    await mkdir(бин, { recursive:true });
    /* урезанный PATH: gio быть не должно, иначе ручная дорога не проверится */
    for (const имя of ['which']){
      const где = ['/usr/bin/' + имя, '/bin/' + имя].find(p => existsSync(p));
      if (где) await symlink(где, join(бин, имя)).catch(() => {});
    }
    const журнал = join(WS, 'flatpak.log');
    await rm(журнал, { force:true });
    await writeFile(join(бин, 'flatpak'),
      '#!/bin/sh\necho "$*" >> "$FLATPAK_LOG"\nexit 0\n', { mode:0o755 });

    const итог = await new Promise(resolve => {
      const child = spawn(process.execPath, [join(root, 'test/launch-child.mjs')],
        { stdio:['ignore', 'pipe', 'pipe'],
          env:{ ...process.env, PATH:бин, FLATPAK_LOG:журнал, GLOWER_TEST_DIR:WS } });
      let out = '';
      child.stdout.on('data', d => { out += d; });
      child.on('exit', () => { try { resolve(JSON.parse(out.trim().split('\n').pop())); }
        catch(e){ resolve({ ошибка:out.slice(0, 300) }); } });
    });

    check('программа из Flathub запускается коротким flatpak run',
      итог.flatpak && итог.flatpak.ok === true && итог.flatpak.via === 'flatpak',
      JSON.stringify(итог.flatpak || итог.ошибка));
    check('flatpak получает только имя программы, без пометок передачи файлов',
      итог.доводы === 'run glower.test.flatpak', String(итог.доводы));
    check('в списке программ видно, что она из Flathub',
      !!(итог.список && итог.список.flatpak === true), JSON.stringify(итог.список));
    check('пометки @@u и @@ не мешают обычному запуску без gio',
      итог.передача && итог.передача.ok === true && итог.запущено === true,
      JSON.stringify(итог.передача));
    check('упавшая программа объясняет причину, а не молчит',
      итог.падение && итог.падение.ok === false && /нет_такой_библиотеки/.test(итог.падение.error || ''),
      JSON.stringify(итог.падение));
    check('о пропавшем ярлыке говорится прямо',
      итог.нет && /такой программы на машине нет/.test(итог.нет.error || ''), JSON.stringify(итог.нет));
    check('путём вместо имени ярлыка подсунуть чужой файл нельзя',
      итог.путь && /неверный идентификатор/.test(итог.путь.error || ''), JSON.stringify(итог.путь));
  }

  /* --- то же самое, но с обычным окружением, где gio есть ---
     Проверки выше идут с урезанным PATH: так проверяется ручная дорога.
     Именно поэтому мимо них однажды прошла настоящая поломка — на живой
     машине gio есть, код уходил в его ветку, а она ни окружения экрана не
     передавала, ни падения не замечала: gio завершается успешно сразу, как
     только попросил программу открыться. Человек видел «откроется своим
     окном» и пустой экран. Повторяем те же проверки как есть. */
  if (existsSync('/usr/bin/gio')){
    const обычный = await new Promise(resolve => {
      const child = spawn(process.execPath, [join(root, 'test/launch-child.mjs')],
        { stdio:['ignore', 'pipe', 'pipe'],
          env:{ ...process.env, FLATPAK_LOG:join(WS, 'flatpak2.log'), GLOWER_TEST_DIR:WS } });
      let out = '';
      child.stdout.on('data', d => { out += d; });
      child.on('exit', () => { try { resolve(JSON.parse(out.trim().split('\n').pop())); }
        catch(e){ resolve({ ошибка:out.slice(0, 300) }); } });
    });

    check('с gio в системе программа всё равно запускается',
      обычный.передача && обычный.передача.ok === true && обычный.запущено === true,
      JSON.stringify(обычный.передача || обычный.ошибка));
    check('с gio в системе падение программы тоже замечается',
      обычный.падение && обычный.падение.ok === false,
      JSON.stringify(обычный.падение || обычный.ошибка));
  }

  /* --- имена программ берутся на языке системы --- */
  {
    const { mkdir, writeFile, rm } = await import('node:fs/promises');
    const { homedir } = await import('node:os');
    const каталог = join(homedir(), '.local/share/applications');
    await mkdir(каталог, { recursive:true });
    await writeFile(join(каталог, 'glower-перевод.desktop'),
      '[Desktop Entry]\nType=Application\nName=File Manager\nName[ru]=Файлы\n' +
      'Comment=Browse files\nComment[ru]=Просмотр файлов\nExec=/bin/true\n');
    const найдено = await page.evaluate(() => Platform.rpc('sys.apps')
      .then(d => (d.list.find(a => a.id === 'glower-перевод.desktop') || {}), () => ({})));
    await rm(join(каталог, 'glower-перевод.desktop'), { force:true });
    check('имя программы берётся на языке системы',
      найдено.name === 'Файлы' || найдено.name === 'File Manager',
      JSON.stringify(найдено));
  }

  /* --- нарисованные приложения уступают место настоящим --- */
  const подмена = await page.evaluate(async () => {
    const было = Platform.rpc.bind(Platform);
    Platform.rpc = (m, p) => {
      if (m === 'sys.apps') return Promise.resolve({ total:1, canLaunch:true,
        list:[{ id:'thunar.desktop', name:'Файлы', comment:'', flatpak:false, icon:'', categories:[] }] });
      if (m === 'sys.launch') return Promise.resolve({ ok:true, via:'проверка', id:p && p.id });
      return было(m, p);
    };
    await wireRealApps();
    const открыто = WM.wins.length;
    const итог = WM.open('files');
    await new Promise(r => setTimeout(r, 400));
    const ответ = { окноНеОткрылось:итог === null && WM.wins.length === openWins(открыто),
      подпись:APPS.files && APPS.files.sub,
      рисованныхУбрали:!APPS.paint && !APPS.todo && !APPS.calendar };
    Platform.rpc = было;
    return ответ;
    function openWins(n){ return n; }
  });
  check('«Файлы» открывают настоящую программу, а не рисованное окно',
    подмена.окноНеОткрылось === true && /Файлы системы/.test(подмена.подпись || ''),
    JSON.stringify(подмена));
  check('нарисованные приложения без замены с машины убраны',
    подмена.рисованныхУбрали === true, JSON.stringify(подмена));

  /* --- настоящие окна машины попадают в панель задач --- */
  const чужие = await page.evaluate(async () => {
    const было = Platform.rpc.bind(Platform);
    Platform.rpc = (m, p) => m === 'sys.windows'
      ? Promise.resolve({ list:[
          { appId:'org.telegram.desktop', title:'Telegram', 'оболочка':false },
          { appId:'glowershell', title:'Рабочий стол', 'оболочка':true }], 'можно':true })
      : было(m, p);
    await new Promise(r => setTimeout(r, 3600));
    const подписи = [...document.querySelectorAll('#dock-running .dock-item')].map(b => b.dataset.tip || '');
    Platform.rpc = было;
    return подписи;
  });
  check('окно чужой программы видно в панели задач',
    чужие.some(t => /Telegram/.test(t)), JSON.stringify(чужие));
  check('сама оболочка себя окном не считает',
    !чужие.some(t => /Рабочий стол|glowershell/.test(t)), JSON.stringify(чужие));

  /* --- у настоящей программы в панели задач свой значок --- */
  {
    const { writeFile, rm } = await import('node:fs/promises');
    /* Крошечная настоящая картинка: одна прозрачная точка */
    const точка = Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
      'base64');
    const файл = join(WS, 'значок-проверки.png');
    await writeFile(файл, точка);
    const свой = await page.evaluate(п => Platform.rpc('sys.icon', { 'имя':п })
      .then(d => d, e => ({ err:e.message })), файл);
    check('система отдаёт настоящий значок программы',
      свой['есть'] === true && String(свой['данные'] || '').startsWith('data:image/png;base64,'),
      JSON.stringify(свой).slice(0, 120));
    await rm(файл, { force:true });

    const нет = await page.evaluate(() => Platform.rpc('sys.icon', { 'имя':'такого-значка-нет-12345' })
      .then(d => d, e => ({ err:e.message })));
    check('о ненайденном значке система говорит прямо',
      нет['есть'] === false && !!нет['почему'], JSON.stringify(нет).slice(0, 120));
  }

  /* --- значок доезжает до панели задач и до Пуска --- */
  const значкиВПанели = await page.evaluate(async () => {
    const было = Platform.rpc.bind(Platform);
    Platform.rpc = (m, p) => {
      if (m === 'sys.apps') return Promise.resolve({ total:1, canLaunch:true,
        list:[{ id:'org.telegram.desktop.desktop', name:'Telegram', comment:'', flatpak:false,
                icon:'telegram', 'окно':'org.telegram.desktop', categories:[] }] });
      if (m === 'sys.windows') return Promise.resolve({ list:[
        { appId:'org.telegram.desktop', title:'Telegram', 'оболочка':false,
          'состояние':{ 'развёрнуто':true, 'свёрнуто':false, 'вовесь':false, 'активно':true } }],
        'можно':true, 'занятЭкран':true });
      if (m === 'sys.icon') return Promise.resolve({ 'есть':true, 'тип':'image/png',
        'данные':'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==' });
      return было(m, p);
    };
    /* Ждём не по часам, а по делу: значок приходит запросом к системе, и на
       загруженной машине он может опоздать. Прежнее ожидание в 3,6 секунды
       иногда не дожидалось — проверка падала на ровном месте и приучала не
       верить красному. */
    const дождись = async (что, сколько = 15000) => {
      const до = Date.now() + сколько;
      while (Date.now() < до){
        const r = что();
        if (r) return r;
        await new Promise(r2 => setTimeout(r2, 100));
      }
      return что();
    };
    const найдиКнопку = () => [...document.querySelectorAll('#dock-running .dock-item')]
      .find(b => /Telegram/.test(b.dataset.tip || ''));
    await дождись(() => { const b = найдиКнопку(); return b && b.querySelector('img'); });
    const кнопка = найдиКнопку();
    const ответ = {
      картинка:!!(кнопка && кнопка.querySelector('img')),
      вработе:!!(кнопка && кнопка.classList.contains('active')),
      впритык:document.body.classList.contains('впритык'),
      подпись:кнопка ? кнопка.dataset.tip : ''
    };
    Platform.rpc = было;
    document.body.classList.remove('впритык');
    return ответ;
  });
  /* --- системный лоток --- */
  /* Настоящую шину сюда не поднять, но всё, что делает оболочка, проверяется
     и так: рисует ли она пришедшие значки и доходит ли нажатие обратно. */
  const лоток = await page.evaluate(async () => {
    const было = Platform.rpc.bind(Platform);
    const позвали = [];
    Platform.rpc = (m, p) => {
      if (m === 'sys.tray') return Promise.resolve({ 'служба':true, 'значки':[
        { id:'a', service:'org.kde.StatusNotifierItem-1-1', path:'/StatusNotifierItem',
          name:'Happ', status:'Active', iconName:'', tooltip:'Happ · подключено',
          iconData:'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
          menu:'/MenuBar', itemIsMenu:false },
        { id:'b', service:'org.kde.StatusNotifierItem-2-1', path:'/StatusNotifierItem',
          name:'Качалка', status:'Active', iconName:'', tooltip:'', iconData:'',
          menu:'', itemIsMenu:true }] });
      if (m === 'sys.tray.нажать'){ позвали.push(p); return Promise.resolve({ ok:true }); }
      return было(m, p);
    };

    const дождись = async (что, сколько = 8000) => {
      const до = Date.now() + сколько;
      while (Date.now() < до){ if (что()) return true; await new Promise(r => setTimeout(r, 100)); }
      return false;
    };
    const появились = await дождись(() =>
      document.querySelectorAll('.tray-их .tray-чужой').length === 2);

    const кнопки = [...document.querySelectorAll('.tray-их .tray-чужой')];
    const скартинкой = !!(кнопки[0] && кнопки[0].querySelector('img'));
    const буквой = кнопки[1] ? кнопки[1].textContent.trim() : '';
    const подсказка = кнопки[0] ? кнопки[0].dataset.tip : '';

    /* Левая кнопка по обычному значку — «покажись», по значку-меню — меню */
    if (кнопки[0]) кнопки[0].click();
    if (кнопки[1]) кнопки[1].click();
    /* Правая — всегда меню */
    if (кнопки[0]) кнопки[0].dispatchEvent(new MouseEvent('contextmenu', { bubbles:true }));
    await new Promise(r => setTimeout(r, 200));

    Platform.rpc = было;
    return { появились, скартинкой, буквой, подсказка,
             действия:позвали.map(п => п['действие']) };
  });

  check('значки программ появляются в лотке',
    лоток.появились === true, JSON.stringify(лоток));
  check('значок с картинкой рисуется картинкой, а без неё — буквой',
    лоток.скартинкой === true && лоток.буквой === 'К', JSON.stringify(лоток));
  check('подсказка значка — та, что дала программа',
    лоток.подсказка === 'Happ · подключено', лоток.подсказка);
  check('нажатия доходят до программы: обычное, меню и правой кнопкой',
    JSON.stringify(лоток.действия) === JSON.stringify(['Activate', 'ContextMenu', 'ContextMenu']),
    JSON.stringify(лоток.действия));

  check('у чужого окна в панели задач свой значок',
    значкиВПанели.картинка === true, JSON.stringify(значкиВПанели));
  check('панель задач видит, какое чужое окно сейчас в работе',
    значкиВПанели.вработе === true, JSON.stringify(значкиВПанели));
  check('о развёрнутом окне сказано словами',
    /развёрнуто/.test(значкиВПанели.подпись || ''), значкиВПанели.подпись);
  check('при развёрнутом чужом окне панель прижимается к краю',
    значкиВПанели.впритык === true, JSON.stringify(значкиВПанели));

  /* --- чужое окно во весь экран: панель уходит с дороги --- */
  const вовесь = await page.evaluate(async () => {
    const было = Platform.rpc.bind(Platform);
    const сказано = [];
    Platform.rpc = (m, p) => {
      if (m === 'sys.windows') return Promise.resolve({ list:[
        { appId:'mpv', title:'Кино', 'оболочка':false,
          'состояние':{ 'развёрнуто':false, 'свёрнуто':false, 'вовесь':true, 'активно':true } }],
        'можно':true, 'вовесьЭкран':true });
      if (m === 'ui.say'){ сказано.push(p); return Promise.resolve({ ok:true }); }
      return было(m, p);
    };
    await new Promise(r => setTimeout(r, 3600));
    const ушла = document.body.classList.contains('чужой-вовесь');
    Platform.rpc = было;
    document.body.classList.remove('чужой-вовесь');
    return { ушла, сказано };
  });
  check('при чужом окне во весь экран панель уходит с дороги',
    вовесь.ушла === true, JSON.stringify(вовесь).slice(0, 160));

  /* --- программа, скачанная файлом: .deb ставится двойным щелчком --- */
  {
    const { mkdir, writeFile, rm, chmod } = await import('node:fs/promises');
    const { execFile:вызов } = await import('node:child_process');
    const собери = () => new Promise(готово => вызов('dpkg-deb',
      ['--build', join(WS, 'пробный'), join(WS, 'проба.deb')],
      { timeout:20000 }, e => готово(!e)));

    await mkdir(join(WS, 'пробный/DEBIAN'), { recursive:true });
    await mkdir(join(WS, 'пробный/usr/bin'), { recursive:true });
    await writeFile(join(WS, 'пробный/DEBIAN/control'),
      'Package: glower-proba\nVersion: 1.0\nSection: utils\nPriority: optional\n' +
      'Architecture: all\nMaintainer: GlowerOS <glower@localhost>\nInstalled-Size: 24\n' +
      'Description: Пробная программа\n');
    await writeFile(join(WS, 'пробный/usr/bin/glower-proba'), '#!/bin/sh\necho проба\n');
    await chmod(join(WS, 'пробный/usr/bin/glower-proba'), 0o755);
    const собран = await собери();

    if (собран){
      const про = await page.evaluate(п => Platform.rpc('pkg.file.info', { 'путь':п })
        .then(d => d, e => ({ err:e.message })), join(WS, 'проба.deb'));
      check('система читает, что за программа в файле',
        про['имя'] === 'glower-proba' && про['версия'] === '1.0' && про['размер'] > 0,
        JSON.stringify(про).slice(0, 200));
    } else check('система читает, что за программа в файле', true, 'dpkg-deb на машине нет — пропущено');

    const чужой = await page.evaluate(() => Platform.rpc('pkg.file.info', { 'путь':'/etc/passwd' })
      .then(() => 'принял', e => e.message));
    check('вместо пакета чужой файл подсунуть нельзя',
      /такие файлы не ставит|нет|повреждён/.test(String(чужой)), String(чужой));

    const относительный = await page.evaluate(() => Platform.rpc('pkg.file.info', { 'путь':'проба.deb' })
      .then(() => 'принял', e => e.message));
    check('путь к файлу должен быть полным',
      /полный путь/.test(String(относительный)), String(относительный));

    const безКлюча = await page.evaluate(п => Platform.rpc('pkg.file.install', { 'путь':п })
      .then(() => 'поставил', e => e.message), join(WS, 'проба.deb'));
    check('без --allow-packages программу из файла не поставить',
      /--allow-packages/.test(String(безКлюча)), String(безКлюча));

    /* Оболочка должна спросить согласия, а не ставить молча */
    const спросила = await page.evaluate(async () => {
      const было = Platform.rpc.bind(Platform);
      let звали = null;
      Platform.rpc = (m, p) => {
        if (m === 'pkg.file.info') return Promise.resolve({ 'имя':'проба', 'версия':'1.0',
          'размер':1024, 'место':2048, 'описание':'проба', файл:p['путь'] });
        if (m === 'pkg.file.install'){ звали = p; return Promise.resolve({ ok:true }); }
        return было(m, p);
      };
      const обещание = спросиПроУстановку({ путь:'/tmp/проба.deb' });
      await new Promise(r => setTimeout(r, 400));
      const окно = document.querySelector('.dlg');
      const текст = окно ? окно.innerText : '';
      /* отказываемся: система не должна ставить ничего */
      const отказ = окно && [...окно.querySelectorAll('button')].find(b => !/Поставить/.test(b.textContent));
      if (отказ) отказ.click();
      await обещание.catch(() => {});
      await new Promise(r => setTimeout(r, 200));
      Platform.rpc = было;
      return { текст, звали };
    });
    check('перед установкой из файла система спрашивает человека',
      /Поставить/.test(спросила.текст || '') && /проба/.test(спросила.текст || ''),
      JSON.stringify(спросила).slice(0, 200));
    check('на отказ система ничего не ставит',
      спросила.звали === null, JSON.stringify(спросила.звали));

    /* Программа одним файлом: её не ставят, а запускают — и только по
       согласию человека. */
    const одинФайл = await page.evaluate(async () => {
      const было = Platform.rpc.bind(Platform);
      let звали = null;
      Platform.rpc = (m, p) => {
        if (m === 'sys.appimage'){ звали = p; return Promise.resolve({ ok:true }); }
        return было(m, p);
      };
      const обещание = спросиПроУстановку({ путь:'/tmp/Программа.AppImage' });
      await new Promise(r => setTimeout(r, 400));
      const окно = document.querySelector('.dlg');
      const текст = окно ? окно.innerText : '';
      const да = окно && [...окно.querySelectorAll('button')].find(b => /Запустить/.test(b.textContent));
      if (да) да.click();
      await обещание.catch(() => {});
      await new Promise(r => setTimeout(r, 200));
      Platform.rpc = было;
      return { текст, звали };
    });
    check('о программе одним файлом система спрашивает и запускает её',
      /Запустить/.test(одинФайл.текст || '') && одинФайл.звали
        && одинФайл.звали['путь'] === '/tmp/Программа.AppImage',
      JSON.stringify(одинФайл).slice(0, 200));

    const чужойЗапуск = await page.evaluate(() => Platform.rpc('sys.appimage', { 'путь':'/usr/bin/id' })
      .then(() => 'запустил', e => e.message));
    check('вместо программы одним файлом системную не запустить',
      /это не программа одним файлом|--allow-launch/.test(String(чужойЗапуск)), String(чужойЗапуск));

    await rm(join(WS, 'проба.deb'), { force:true });
  }

  /* --- размер панели растёт вместе с экраном --- */
  {
    const р = await page.evaluate(() => ({ посчитан:Shell.размерДока(),
      высота:innerHeight, свой:S.dockSizeСвой }));
    check('размер значков панели считается по размеру экрана',
      р.свой === false && р.посчитан >= 44 && р.посчитан <= 92
        && р.посчитан === Math.round(Math.min(92, Math.max(44, Math.max(600, р.высота) * 0.052))),
      JSON.stringify(р));
    const свой = await page.evaluate(() => {
      S.dockSizeСвой = true; S.dockSize = 70;
      const мой = Shell.размерДока();
      S.dockSizeСвой = false;
      return мой;
    });
    check('заданный человеком размер главнее расчёта', свой === 70, String(свой));
  }

  /* --- съёмные носители --- */
  {
    const н = await page.evaluate(() => Platform.rpc('sys.drives').then(d => d, e => ({ err:e.message })));
    check('система перечисляет носители или честно говорит, почему нет',
      н['есть'] === true ? Array.isArray(н.list) : !!н['почему'], JSON.stringify(н).slice(0, 180));
    const чужой = await page.evaluate(() => Platform.rpc('sys.drive', { action:'mount', dev:'/etc/passwd' })
      .then(() => 'принял', e => e.message));
    check('вместо носителя чужой путь подсунуть нельзя',
      /неверное имя носителя|--allow-launch/.test(String(чужой)), String(чужой));
  }

  /* --- Bluetooth --- */
  {
    const bt = await page.evaluate(() => Platform.rpc('sys.bt').then(d => d, e => ({ err:e.message })));
    check('про Bluetooth система отвечает честно',
      bt['есть'] === true ? Array.isArray(bt.list) : !!bt['почему'], JSON.stringify(bt).slice(0, 140));
    const плохой = await page.evaluate(() => Platform.rpc('sys.bt.device', { action:'connect', 'адрес':'; rm -rf /' })
      .then(() => 'принял', e => e.message));
    check('вместо адреса устройства команду подсунуть нельзя',
      /неверный адрес|bluetoothctl|--allow-launch/.test(String(плохой)), String(плохой));
  }

  /* --- обновления системы --- */
  {
    const о = await page.evaluate(() => Platform.rpc('pkg.upgrade.check').then(d => d, e => ({ err:e.message })));
    check('система умеет проверять обновления',
      Array.isArray(о.list) && typeof о['всего'] === 'number',
      JSON.stringify({ всего:о['всего'], err:о.err }));
    const безКлюча = await page.evaluate(() => Platform.rpc('pkg.upgrade.run', {})
      .then(() => 'пошло', e => e.message));
    check('без --allow-packages система не обновляется',
      /--allow-packages/.test(String(безКлюча)), String(безКлюча));
  }

  /* --- вход по настоящему паролю --- */
  {
    const пусто = await page.evaluate(() => Platform.rpc('sys.auth', { 'пароль':'' })
      .then(d => d, e => ({ err:e.message })));
    check('пустой пароль система не принимает', пусто.ok === false, JSON.stringify(пусто));

    const кто = await page.evaluate(() => Platform.rpc('sys.me').then(d => d, e => ({ err:e.message })));
    check('система называет свою учётную запись',
      !!кто['имя'] && Array.isArray(кто['группы']), JSON.stringify(кто).slice(0, 140));

    const короткий = await page.evaluate(() => Platform.rpc('sys.passwd', { 'старый':'x', 'новый':'12' })
      .then(() => 'принял', e => e.message));
    check('слишком короткий пароль система не примет',
      /короче четырёх|--allow-launch/.test(String(короткий)), String(короткий));

    const спросил = await page.evaluate(async () => {
      const было = Platform.rpc.bind(Platform);
      let звали = null;
      Platform.rpc = (m, p) => {
        if (m === 'sys.auth'){ звали = p; return Promise.resolve({ ok:false, 'почему':'пароль не подошёл' }); }
        return было(m, p);
      };
      const итог = await Profiles.verify((Profiles.current() || {}).id, 'что-то');
      Platform.rpc = было;
      return { звали, итог };
    });
    check('вход спрашивает пароль у системы, а не у себя',
      спросил.звали && спросил.звали['пароль'] === 'что-то' && спросил.итог === false,
      JSON.stringify(спросил));
  }

  /* --- люди в системе --- */
  {
    const л = await page.evaluate(() => Platform.rpc('sys.users').then(d => d, e => ({ err:e.message })));
    check('система перечисляет своих людей',
      Array.isArray(л.list) && л.list.every(ч => ч['имя'] && ч.uid >= 1000),
      JSON.stringify((л.list || []).slice(0, 2)));

    const служебная = await page.evaluate(() => Platform.rpc('sys.user', { action:'удалить', 'имя':'root' })
      .then(() => 'принял', e => e.message));
    check('служебную учётную запись система не трогает',
      /служебная|--allow-launch/.test(String(служебная)), String(служебная));

    const дурное = await page.evaluate(() => Platform.rpc('sys.user',
      { action:'завести', 'имя':'Плохое Имя', 'пароль':'1234' }).then(() => 'принял', e => e.message));
    check('имя учётной записи проверяется по образцу',
      /имя учётной записи|--allow-launch/.test(String(дурное)), String(дурное));

    const слабый = await page.evaluate(() => Platform.rpc('sys.user',
      { action:'завести', 'имя':'ктото', 'пароль':'12' }).then(() => 'принял', e => e.message));
    check('слишком короткий пароль при заведении не принимается',
      /короче четырёх|--allow-launch/.test(String(слабый)), String(слабый));

    const себя = await page.evaluate(async () => {
      const я = await Platform.rpc('sys.me');
      return Platform.rpc('sys.user', { action:'удалить', 'имя':я['имя'] })
        .then(() => 'принял', e => e.message);
    });
    check('себя система удалить не даёт',
      /из-под которой работает|последний хозяин|служебная|--allow-launch/.test(String(себя)), String(себя));
  }

  /* --- печать --- */
  {
    const п = await page.evaluate(() => Platform.rpc('sys.printers').then(d => d, e => ({ err:e.message })));
    check('система отвечает про принтеры честно',
      п['есть'] === true ? Array.isArray(п.list) : !!п['почему'], JSON.stringify(п).slice(0, 160));
    const ключ = await page.evaluate(() => Platform.rpc('sys.printer', { action:'проба', 'принтер':'-o' })
      .then(() => 'принял', e => e.message));
    check('вместо имени принтера ключ подсунуть нельзя',
      /неверное имя принтера|--allow-launch/.test(String(ключ)), String(ключ));
  }

  /* --- масштаб экрана --- */
  {
    const э = await page.evaluate(() => Platform.rpc('sys.screens').then(d => d, e => ({ err:e.message })));
    check('система отвечает про экраны честно',
      э['есть'] === true ? Array.isArray(э.list) : !!э['почему'], JSON.stringify(э).slice(0, 140));
    const дикий = await page.evaluate(() => Platform.rpc('sys.screen.scale', { 'экран':'HDMI-1', 'масштаб':9 })
      .then(() => 'принял', e => e.message));
    check('несуразный масштаб система не примет',
      /не примет|--allow-launch/.test(String(дикий)), String(дикий));
  }

  /* --- диспетчер задач: настоящие числа и настоящее снятие задачи --- */
  {
    const п = await page.evaluate(async () => {
      await Platform.rpc('sys.procs');
      await new Promise(r => setTimeout(r, 700));
      return Platform.rpc('sys.procs').then(d => d, e => ({ err:e.message }));
    });
    check('доля процессора считается за последние секунды, а не за всю жизнь процесса',
      п['впервые'] === false && Array.isArray(п.list) && п.list.every(x => x.cpu >= 0 && x.cpu <= 100),
      JSON.stringify((п.list || []).slice(0, 2)));
    const снять = await page.evaluate(() => Platform.rpc('sys.stop', { pid:1 })
      .then(() => 'выполнилось', e => e.message));
    check('первый процесс машины снять нельзя',
      /неверный номер|--allow-launch/.test(снять), String(снять));
    const снятьЧужое = await page.evaluate(() => Platform.rpc('sys.stop', { pid:'-1; rm -rf /' })
      .then(() => 'выполнилось', e => e.message));
    check('вместо номера процесса команду подсунуть нельзя',
      /неверный номер|--allow-launch/.test(снятьЧужое), String(снятьЧужое));

    const окно = await page.evaluate(async () => {
      const w = WM.open('taskmgr');
      await new Promise(r => setTimeout(r, 1200));
      const строки = [...w.body.querySelectorAll('.tm-row')].length;
      const снять = [...w.body.querySelectorAll('.btn')].filter(b => b.textContent === 'Снять').length;
      WM.close(w);
      return { строки, снять };
    });
    check('в диспетчере задач видны настоящие процессы и их можно снять',
      окно.строки > 3 && окно.снять > 3, JSON.stringify(окно));
  }

  /* --- чем система открывает такой-то тип --- */
  {
    const { mkdir, writeFile, rm } = await import('node:fs/promises');
    const { homedir } = await import('node:os');
    const файл = join(homedir(), '.config/mimeapps.list');
    await mkdir(join(homedir(), '.config'), { recursive:true });
    let прежнее = null;
    try { прежнее = await (await import('node:fs/promises')).readFile(файл, 'utf8'); } catch(e){}
    await writeFile(файл, '[Default Applications]\nx-scheme-handler/https=firefox.desktop\n');
    const чем = await page.evaluate(() => Platform.rpc('sys.mime', { 'тип':'x-scheme-handler/https' })
      .then(d => d, e => ({ err:e.message })));
    if (прежнее === null) await rm(файл, { force:true }); else await writeFile(файл, прежнее);
    check('система знает, чем открывать ссылки',
      чем && чем['чем'] === 'firefox.desktop', JSON.stringify(чем));
  }

  const окна = await page.evaluate(() => Platform.rpc('sys.windows').then(d => d, e => ({ err:e.message })));
  check('система честно отвечает про свои окна',
    Array.isArray(окна.list), JSON.stringify(окна).slice(0, 160));
  const окноБезКлюча = await page.evaluate(() => Platform.rpc('sys.window', { action:'close', appId:'glowershell' })
    .then(() => 'выполнилось', e => e.message));
  check('без --allow-launch чужие окна не трогают',
    /--allow-launch/.test(окноБезКлюча), String(окноБезКлюча));

  /* --- починка: закрытый список действий, и только по ключу --- */
  const починкаБезКлюча = await page.evaluate(() => Platform.rpc('sys.fix', { что:'песочница' })
    .then(() => 'выполнилось', e => e.message));
  check('без --allow-launch система сама себя не чинит',
    /--allow-launch/.test(починкаБезКлюча), String(починкаБезКлюча));
  const починкаЧужая = await page.evaluate(() => Platform.rpc('sys.fix', { что:'rm -rf /' })
    .then(() => 'выполнилось', e => e.message));
  check('вместо починки чужую команду подсунуть нельзя',
    /неизвестная починка|--allow-launch/.test(починкаЧужая), String(починкаЧужая));

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

  /* Экран входа поднимает свой агент, отдельный от сеансового. Права ему
     выдаются отдельно — и однажды не выдались вовсе: кнопка выключения на
     экране входа отвечала «нет такого метода: sys.power», и выключить
     машину, не входя в систему, было нельзя.

     Проверяем оба конца: что экран входа просит это право и что без него
     метода действительно нет. */
  {
    const вход = await readFile(join(root, 'linux', 'glower-greeter'), 'utf8');
    const строка = вход.split('\n').find(с => /server\.mjs/.test(с) && !/^\s*#/.test(с)) || '';
    check('экран входа просит право выключать машину', /--allow-power/.test(строка), строка.trim());
    check('и не просит лишнего',
      !/--allow-(launch|install|open|packages)/.test(строка), строка.trim());
  }

  /* Панель зовёт агент по именам действий, и имена эти надо брать у него,
     а не придумывать. Я придумал — и кнопка «Перезагрузка» ответила
     «неизвестное действие: restart». Сверяем оба списка напрямую: всё, что
     панель может послать, агент обязан знать. */
  {
    const панель = await readFile(join(root, 'js', 'панель.js'), 'utf8');
    const система = await readFile(join(root, 'agent', 'system.mjs'), 'utf8');
    const шлём = [...панель.matchAll(/\{\s*id:'([a-z]+)'/g)].map(m => m[1]);
    const знает = (система.match(/const map = \{[^}]*\}/s) || [''])[0];
    const чужие = шлём.filter(a => !new RegExp('\\b' + a + ':').test(знает));
    check('панель просит у агента только то, что он умеет',
      шлём.length >= 5 && чужие.length === 0, 'агент не знает: ' + чужие.join(', '));
  }

  /* В меню программ на панели должны быть настоящие программы машины.
  
     Их там не было ни одной: список собирался из наших нарисованных
     приложений один раз при запуске, а настоящие приходят позже и лежат в
     другом списке. Человек открывал меню и не находил ни Firefox, ни того,
     что сам поставил.
  
     Проверяем сквозным путём, как оно и работает: подкладываем машине
     программу, говорим столу, что список изменился, и смотрим, что ушло
     на шину — именно это читает панель. */
  {
    const p2 = await browser.newPage({ viewport:{ width:1100, height:760 } });
    await p2.addInitScript(() => { try { localStorage.setItem('glower.setup.done', 'true'); } catch(e){} });
    await p2.goto(`http://localhost:${PORT}/?раздельно=1`);
    await p2.waitForTimeout(3000);
    await p2.keyboard.press('Enter');
    await p2.waitForTimeout(1500);
    await p2.evaluate(() => {
      OS.машинные = [{ id:'проба.desktop', name:'Пробная программа машины' }];
      document.dispatchEvent(new CustomEvent('glower:программы'));
    });
    await p2.waitForTimeout(1200);
    const список = await p2.evaluate(async () =>
      (await Platform.rpc('ui.last', { 'тема':'программы' }) || {}).что || []);
    const своя = список.find(п => п.id === 'проба.desktop');
    check('в меню панели есть настоящие программы машины',
      !!своя && своя['вид'] === 'машина', JSON.stringify(своя || список.slice(0, 2)));
    check('и наши приложения из него не пропали',
      список.some(п => п['вид'] === 'приложение'), 'всего: ' + список.length);

    /* Плитки быстрых настроек в панели переключают настройки стола — у неё
       самой их нет. Ходит это через шину, и проверять надо именно так:
       написать просьбу и посмотреть, изменилось ли на столе.
       
       Первая же попытка тут и провалилась: я проверял «window.Store», а
       Store объявлен через const и в window не попадает. Ветка молча не
       выполнялась — плитка щёлкала, сообщение доходило, настройка не
       менялась, и ни одной ошибки в журнале. */
    const было = await p2.evaluate(() => !!S.dnd);
    await p2.evaluate(б => Platform.rpc('ui.say',
      { 'тема':'настройка', что:{ 'ключ':'dnd', 'что':!б } }), было);
    await p2.waitForTimeout(1200);
    const стало = await p2.evaluate(async () => ({
      вПамяти:!!S.dnd,
      наШине:((await Platform.rpc('ui.last', { 'тема':'настройки' }) || {}).что || {}).dnd
    }));
    check('плитка из панели переключает настройку стола',
      стало.вПамяти === !было && стало.наШине === !было, JSON.stringify(стало));

    /* Заряд виден сразу, а не только тому, кто открыл меню: стол
       опрашивает машину и говорит вслух, панель по этому слову рисует
       заливку значка и молнию. Он же предупреждает, когда заряд на
       исходе. */
    const бат = await p2.evaluate(async () =>
      (await Platform.rpc('ui.last', { 'тема':'батарея' }) || {}).что);
    check('стол рассказывает панели о заряде',
      !!бат && 'есть' in бат && 'заряжается' in бат, JSON.stringify(бат));

    /* Без заставки стол говорит «занят» и «свободен» почти разом. Пока
       слова не шли по очереди, шина получала их наоборот — и панель после
       входа пряталась до повтора через пятнадцать секунд. */
    const занят = await p2.evaluate(async () => (await Platform.rpc('ui.last', { 'тема':'стол-занят' })).что);
    check('после входа шина знает, что стол свободен', занят === false, JSON.stringify(занят));

    /* Нативное меню программ (glower-panel): стол перестаёт открывать
       своё, отвечает о столах и окнах и делает то, что выбрано в меню. */
    const нат = await p2.evaluate(async () => {
      const ждать = мс => new Promise(r => setTimeout(r, мс));
      const последнее = async т => (await Platform.rpc('ui.last', { 'тема':т })).что;
      await Platform.rpc('ui.say', { 'тема':'обзор-свой', что:true });
      await ждать(1200);
      const и = {};
      const было = (await Platform.rpc('ui.last', { 'тема':'' })).n;
      Shell.toggleStart();
      await ждать(400);
      и.своёЗакрыто = !document.querySelector('#обзор.on');
      и.просьба = await последнее('открой-пуск');
      await Platform.rpc('ui.say', { 'тема':'обзор-дай', что:true });
      await ждать(1200);
      const д = await последнее('обзор-данные');
      и.данные = !!д && д['столы'] === WM.desks && Array.isArray(д['окна']) && Array.isArray(д['на-столах']);
      const столов = WM.desks;
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'стол-добавь' } });
      await ждать(1200);
      и.столДобавлен = WM.desks === столов + 1;
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'стол', n:WM.desks - 1 } });
      await ждать(1200);
      и.перешёл = WM.desk === WM.desks - 1;
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'стол', n:0 } });
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'стол-убери', n:WM.desks - 1 } });
      await ждать(1200);
      и.убран = WM.desks === столов;
      const окон = WM.wins.length;
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'запусти', id:'calc', 'вид':'приложение' } });
      await ждать(1200);
      и.запущено = WM.wins.some(w => w.appId === 'calc') && WM.wins.length >= окон;
      /* Перетаскивание в нативном меню: программа — на стол, окно — на
         другой стол и на «+» */
      const столов2 = WM.desks;
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'стол-добавь' } });
      await ждать(800);
      await Platform.rpc('ui.say', { 'тема':'обзор-дело',
        что:{ 'что':'на-стол', id:'notepad', 'вид':'приложение', n:WM.desks - 1 } });
      await ждать(1200);
      const блокнот = WM.wins.find(w => w.appId === 'notepad' && w.desk === WM.desks - 1);
      и.программаНаСтоле = !!блокнот && WM.desk !== WM.desks - 1;
      const к = WM.wins.find(w => w.appId === 'calc');
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'окно-на-стол', id:к.node.id, n:WM.desks - 1 } });
      await ждать(1200);
      и.окноПереехало = к.desk === WM.desks - 1;
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'окно-на-новый-стол', id:к.node.id } });
      await ждать(1200);
      и.окноНаНовом = WM.desks === столов2 + 2 && к.desk === WM.desks - 1;
      /* Нативный док: стол прячет свой и рассказывает о программах */
      await Platform.rpc('ui.say', { 'тема':'док-свой', что:true });
      await ждать(1200);
      и.свойДокСпрятан = document.body.classList.contains('док-нативный')
        && getComputedStyle(document.querySelector('#dock-wrap')).display === 'none';
      const док = await последнее('док');
      и.докЗнает = !!док && Array.isArray(док['закреплены']) && док['закреплены'].length > 0
        && typeof док['окон'] === 'number' && Array.isArray(док['запущены']) && док['запущены'].includes('calc');
      await Platform.rpc('ui.say', { 'тема':'обзор-дело', что:{ 'что':'закрой-все', id:'calc' } });
      await ждать(1200);
      и.закрытыВсе = !WM.wins.some(w => w.appId === 'calc');
      await Platform.rpc('ui.say', { 'тема':'док-свой', что:false });
      await ждать(1200);
      и.свойДокСнова = !document.body.classList.contains('док-нативный');
      /* Нативный стол: страница прозрачна, щелчки ловит только на своём,
         рассказывает о значках и обоях и делает выбранное на столе */
      await Platform.rpc('ui.say', { 'тема':'стол-свой', что:true });
      await ждать(1500);
      и.столПрозрачен = document.body.classList.contains('стол-нативный')
        && getComputedStyle(document.querySelector('#wallpaper')).display === 'none'
        && getComputedStyle(document.body).backgroundColor === 'rgba(0, 0, 0, 0)';
      const обл = window.__областиСтола || {};
      и.области = !обл['вся'] && Array.isArray(обл['области'])
        && обл['области'].every(r => r.length === 4 && r[2] > 0 && r[3] > 0)
        /* пустой стол целиком страница себе не забирает */
        && !обл['области'].some(r => r[2] >= innerWidth && r[3] >= innerHeight);
      const значки = await последнее('значки');
      /* Что лежит на столе, зависит от машины — берём любой значок */
      const заметка = (значки || [])[0];
      и.значки = Array.isArray(значки) && значки.length > 0
        && значки.every(з => з.n && Number.isFinite(з.x) && Number.isFinite(з.y) && з['вид']);
      и.какие = (значки || []).map(з => з.n + ':' + з['вид']).join(', ');
      const обои = await последнее('обои');
      и.обои = !!обои && typeof обои.css === 'string' && обои.css.length > 10;
      if (заметка){
        const окон2 = WM.wins.length;
        await Platform.rpc('ui.say', { 'тема':'значки-дело', что:{ 'что':'открой', n:заметка.n } });
        await ждать(1500);
        и.открылось = WM.wins.length > окон2;
        await Platform.rpc('ui.say', { 'тема':'значки-дело', что:{ 'что':'позиции', 'позиции':{ [заметка.n]:{ x:333, y:222 } } } });
        await ждать(1200);
        const п = KV.get('iconPos', {})[заметка.n];
        и.место = !!п && п.x === 333 && п.y === 222;
        await Platform.rpc('ui.say', { 'тема':'значки-дело', что:{ 'что':'выбор', 'имена':[заметка.n] } });
        await ждать(1000);
        и.выбор = $$('#desktop-icons .di.sel').map(d => d.dataset.name).join() === заметка.n;
      }
      await Platform.rpc('ui.say', { 'тема':'значки-дело', что:{ 'что':'меню-стола', x:400, y:300 } });
      await ждать(1000);
      и.менюСтола = document.querySelector('#ctx').classList.contains('on')
        && /Создать папку/.test(document.querySelector('#ctx').textContent);
      Shell.closePanels && Shell.closePanels();
      document.querySelector('#ctx').classList.remove('on');
      await Platform.rpc('ui.say', { 'тема':'стол-свой', что:false });
      await ждать(1500);
      и.столСнова = !document.body.classList.contains('стол-нативный') && !!(window.__областиСтола || {})['вся'];
      /* Страничная панель возвращает меню столу */
      await Platform.rpc('ui.say', { 'тема':'обзор-свой', что:false });
      await ждать(1200);
      Shell.toggleStart(true);
      await ждать(400);
      и.своёСнова = !!document.querySelector('#обзор.on');
      Shell.обзор(false);
      и.n = было;
      return и;
    });
    check('нативное меню: стол не открывает своё, а просит панель',
      нат.своёЗакрыто && нат.просьба === 'переключи', JSON.stringify(нат));
    check('нативное меню: стол рассказывает о столах и окнах',
      нат.данные, JSON.stringify(нат));
    check('нативное меню: столы заводятся, переключаются и убираются, программы запускаются',
      нат.столДобавлен && нат.перешёл && нат.убран && нат.запущено, JSON.stringify(нат));
    check('при страничной панели меню снова рисует стол', нат.своёСнова, JSON.stringify(нат));
    check('нативный стол: страница прозрачна и ловит щелчки только на своём',
      нат.столПрозрачен && нат.области, JSON.stringify(нат));
    check('нативный стол: страница рассказывает о значках и обоях',
      нат.значки && нат.обои, JSON.stringify(нат));
    check('нативный стол: открыть, запомнить место, выделить, меню стола — делает страница',
      нат.открылось && нат.место && нат.выбор && нат.менюСтола, JSON.stringify(нат));
    check('без нативного стола страница снова рисует его сама и ловит всё',
      нат.столСнова, JSON.stringify(нат));
    check('нативный док: стол прячет свой, рассказывает о программах и закрывает окна по просьбе',
      нат.свойДокСпрятан && нат.докЗнает && нат.закрытыВсе && нат.свойДокСнова, JSON.stringify(нат));
    check('перетаскивание в нативном меню: программа на стол, окно на стол и на «+»',
      нат.программаНаСтоле && нат.окноПереехало && нат.окноНаНовом, JSON.stringify(нат));

    /* Alt+F4 на столе оконный сервер отдаёт странице через шину — и
       закрыться должно верхнее окно системы, а не ничего. */
    const клав = await p2.evaluate(async () => {
      const w = WM.open('notepad');
      await new Promise(r => setTimeout(r, 500));
      const было = WM.wins.includes(w);
      await Platform.rpc('ui.say', { 'тема':'клавиша', что:'A-F4' });
      for (let i = 0; i < 25 && WM.wins.includes(w); i++) await new Promise(r => setTimeout(r, 200));
      return { было, закрыто:!WM.wins.includes(w) };
    });
    check('Alt+F4 с оконного сервера закрывает верхнее окно системы',
      клав.было && клав.закрыто, JSON.stringify(клав));
    await p2.close();
  }

  /* Панель читает ответы агента по именам полей, и придумывать их нельзя.
  
     Я придумал: читал «громкость», «процент», «заряжается», а агент
     отвечает volume, battery.level, charging. Ни одного такого поля в
     ответе нет — и панель честно показывала ноль процентов громкости,
     пустую яркость и прочерк вместо заряда. Человек сказал прямо:
     показатели должны быть настоящими.
  
     Проверяем разбор на настоящих ответах этой машины: берём ответ агента
     и пропускаем через тот же код, которым его читает панель. */
  {
    const p4 = await browser.newPage({ viewport:{ width:900, height:600 } });
    await p4.goto(`http://localhost:${PORT}/панель.html?surface=панель-верх&раздельно=1`);
    await p4.waitForTimeout(1200);
    const r = await p4.evaluate(async () => {
      const зов = (м, п) => fetch('/rpc', { method:'POST',
        headers:{ 'Content-Type':'application/json' },
        body:JSON.stringify({ method:м, params:п || {} }) })
        .then(о => о.json()).then(о => о.result);
      const [гром, ярк, бат] = await Promise.all([
        зов('sys.volume.get'), зов('sys.brightness.get'), зов('sys.battery')]);
      return {
        громкость:{ сырое:гром.volume, разбор:Разбор.громкость(гром) },
        яркость:{ сырое:ярк.value, разбор:Разбор.яркость(ярк) },
        батарея:{ есть:бат.present, уровень:Разбор.уровень(бат), словами:Разбор.батарея(бат) }
      };
    });
    /* Ответ «машина этим не управляет» — тоже верный ответ: важно, что
       разбор совпадает с тем, что на самом деле сказал агент. */
    check('панель читает настоящую громкость',
      r.громкость.разбор === (typeof r.громкость.сырое === 'number' ? r.громкость.сырое : null),
      JSON.stringify(r.громкость));
    check('и настоящую яркость',
      r.яркость.разбор === (typeof r.яркость.сырое === 'number' ? r.яркость.сырое : null),
      JSON.stringify(r.яркость));
    check('и настоящий заряд',
      (r.батарея.есть === false && r.батарея.уровень === null) || typeof r.батарея.уровень === 'number',
      JSON.stringify(r.батарея));
    check('а про батарею говорит словами, а не прочерком',
      typeof r.батарея.словами === 'string' && r.батарея.словами.length > 3, r.батарея.словами);

    /* «Bluetooth не работает» — не ответ. Выключен рубильником, нет
       прошивки или адаптера нет вовсе — это три разные беды, и человеку
       делать надо разное. Система обязана сказать, какая из них. */
    const бт = await p4.evaluate(async () => fetch('/rpc', { method:'POST',
      headers:{ 'Content-Type':'application/json' },
      body:JSON.stringify({ method:'sys.bt.почему', params:{} }) })
      .then(о => о.json()).then(о => о.result));
    check('система объясняет, почему нет Bluetooth',
      !!бт && typeof бт['словами'] === 'string' && бт['словами'].length > 10,
      JSON.stringify(бт).slice(0, 120));
    await p4.close();
  }

  /* Диспетчер задач: программы человека отдельно, внутренности системы
     отдельно. Человек шёл закрыть зависшую программу, а видел список, где
     она тонет среди WebKitWebProcess, systemd и наших же частей. */
  {
    const r = await page.evaluate(async () => {
      const w = WM.open('taskmgr');
      await new Promise(r2 => setTimeout(r2, 2600));
      const текст = w.node.textContent;
      return { есть:/Система и оболочка/.test(текст),
               программы:/Программы|Ни одной программы/.test(текст) };
    });
    check('диспетчер отделяет программы от системного', r.есть && r.программы, JSON.stringify(r));
    const поля = await page.evaluate(async () =>
      ((await Platform.rpc('sys.procs')).list || [])[0] || {});
    check('в списке процессов есть строка запуска', 'cmd' in поля, JSON.stringify(поля).slice(0, 90));
  }

  /* Значок в лотке должен закрываться сам, без похода в диспетчер. */
  {
    const r = await page.evaluate(async () => {
      try { await Platform.rpc('sys.tray.закрыть', { 'служба':'org.example.нетТакого' }); return 'прошло'; }
      catch(e){ return String(e.message || e); }
    });
    check('закрытие программы из лотка есть и проверяет имя',
      r !== 'прошло' && !/нет такого метода/.test(r), r);
  }

  /* Лица программ. Наш рисунок — последнее, что показывают, а не первое:
     у поставленной программы значок лежит на машине, и человек должен
     видеть его, а не наш знак.
     
     Проверяем на LibreOffice: он есть в любой сборке Ubuntu, и значок у
     него лежит не там, где по нынешним правилам положено, — в старом
     общем каталоге и под именем с хвостом (libreoffice-calc вместо
     libreoffice). Ровно на этом система его и не находила. */
  {
    const о = await page.evaluate(async () =>
      await Platform.rpc('sys.icon', { 'имя':'libreoffice', 'ярлык':'libreoffice' }));
    check('значок программы находится по имени пакета',
      !!о && о['есть'] === true && /libreoffice/.test(о['путь'] || ''),
      JSON.stringify(о && о['путь'] || о));
  }

  /* Обновление системы — обычным пакетом из обычного репозитория.
  
     До этого единственным способом получить новую версию была полная
     переустановка. Теперь наши файлы едут пакетом, и проверять надо три
     вещи: что пакет собирается и в нём лежит наше; что образ его ставит,
     а значит dpkg знает версию и apt будет с чем сравнивать; и что в
     образе прописан источник, откуда придут следующие.
     
     Пакет собираем по-настоящему, из поддельного корня: сборщик — это
     тот самый код, который отработает при выпуске версии. */
  {
    const { execFile } = await import('node:child_process');
    const { mkdtemp: мк, writeFile: пиши, mkdir: катал } = await import('node:fs/promises');
    const корень = await мк(join(tmpdir(), 'glower-корень-'));
    const выход = await мк(join(tmpdir(), 'glower-пакет-'));
    await катал(join(корень, 'usr/share/glower/ui'), { recursive:true });
    await катал(join(корень, 'usr/bin'), { recursive:true });
    await пиши(join(корень, 'usr/share/glower/ui/index.html'), '<html>');
    await пиши(join(корень, 'usr/bin/glower-shell'), '#!/usr/bin/python3\n');
    await пиши(join(корень, 'etc/glower-release'), 'v9.9\n').catch(async () => {
      await катал(join(корень, 'etc'), { recursive:true });
      await пиши(join(корень, 'etc/glower-release'), 'v9.9\n');
    });

    const собран = await new Promise(готово => execFile('bash',
      [join(root, 'linux', 'собери-пакет.sh'), корень, 'v9.9-2-gabcdef', выход],
      { timeout:30000 }, (e, out) => готово(e ? '' : String(out).trim())));
    check('пакет с нашими файлами собирается', /glower_9\.9\+2\.gabcdef_all\.deb$/.test(собран), собран);

    if (собран){
      const внутри = await new Promise(готово => execFile('dpkg-deb', ['-c', собран],
        { timeout:15000 }, (e, out) => готово(e ? '' : String(out))));
      check('в пакете лежит оболочка и страницы', /usr\/bin\/glower-shell/.test(внутри)
        && /usr\/share\/glower\/ui\/index\.html/.test(внутри), внутри.slice(0, 80));
      /* Чужих файлов в пакете быть не должно: dpkg откажется ставить
         пакет, который лезет на территорию другого. Именно поэтому наши
         настройки терминала переехали из /etc/xdg/foot к нам. */
      check('и ничего чужого', !/etc\/xdg\/foot/.test(внутри));
    }
  }

  /* Придержанные обновления apt называет отдельной строкой, и система
     обязана её прочитать: иначе человек видит «система обновлена» при том,
     что новая версия есть и не ставится. */
  {
    const есть = await page.evaluate(async () =>
      'удержано' in (await Platform.rpc('pkg.upgrade.check').catch(() => ({}))));
    check('система читает и придержанные обновления', есть);
  }

  {
    const образ = await readFile(join(root, 'linux', 'mkiso.sh'), 'utf8');
    check('образ собирает и ставит наш пакет',
      /собери-пакет\.sh/.test(образ) && /dpkg -i \/tmp\/glower\.deb/.test(образ));
    check('и прописывает, откуда брать обновления',
      /sources\.list\.d\/glower\.list/.test(образ) && /raw\.githubusercontent\.com/.test(образ));
    const сеанс = await readFile(join(root, 'linux', 'glower-session'), 'utf8');
    check('настройки терминала лежат у нас, а не в чужом пакете',
      /XDG_CONFIG_DIRS="\/usr\/share\/glower\/xdg/.test(сеанс));
  }

  /* Живая система грузится ради установщика, и всё, что не ведёт к нему,
     только отнимает время. Службы отключены прямо в пунктах меню загрузки:
     проверить это иначе можно лишь записав флешку и загрузившись, поэтому
     стережём хотя бы то, что список на месте и доезжает до ядра. */
  {
    const образ = await readFile(join(root, 'linux', 'mkiso.sh'), 'utf8');
    const строка = (образ.match(/^set fast=.*$/m) || [''])[0];
    check('в живой системе лишние службы выключены',
      /NetworkManager\.service/.test(строка) && /ufw\.service/.test(строка)
      && /cups\.service/.test(строка), строка.slice(0, 60));
    const пункты = образ.split('menuentry').slice(1);
    const установка = пункты.filter(п => /glower\.install=1/.test(п));
    check('быстрый путь стоит в пунктах установки',
      установка.filter(п => /\$fast/.test(п)).length >= 3);
    check('и запасной пункт грузит систему целиком',
      установка.some(п => /сообщениями системы/.test(п) && !/\$fast/.test(п)));
  }

  /* Две вещи, о которых человек попросил прямым текстом: не переписывать
     руками простыни диагностики и иметь возможность вставить текст в
     терминал. Обе живут только в образе, поэтому проверяем сборку образа —
     иначе они тихо выпадут при первой же перестановке строк. */
  {
    const образ = await readFile(join(root, 'linux', 'mkiso.sh'), 'utf8');
    check('в образ попадает «врач»', /install .*linux\/врач.*usr\/bin\/врач/.test(образ));
    check('и он же латиницей', /usr\/bin\/vrach/.test(образ) && /usr\/bin\/dhfx/.test(образ));
    check('в образ попадают настройки терминала',
      /usr\/share\/glower\/xdg\/foot\/foot\.ini/.test(образ));
    const настройки = await readFile(join(root, 'linux', 'foot.ini'), 'utf8');
    const вставка = настройки.split('\n').find(с => /^clipboard-paste=/.test(с)) || '';
    check('Ctrl+V в терминале вставляет', /Control\+v(\s|$)/.test(вставка), вставка);
  }

  /* --- обои системы: список и отдача файлов --- */
  {
    const о = await page.evaluate(() => Platform.rpc('sys.обои').catch(e => ({ ошибка:String(e.message || e) })));
    check('агент читает обои, лежащие в системе',
      !!о && Array.isArray(о.list), JSON.stringify(о).slice(0, 120));

    /* Дорожка отдачи — единственное место, где агент отдаёт наружу файл не
       из своей папки. Проверяем оба ответа: своё отдаёт, чужое не отдаёт. */
    const свои = (о.list || []).filter(x => /^\/usr\/share\/(backgrounds|wallpapers)\//.test(x.путь));
    if (свои.length){
      const код = await page.evaluate(async путь => {
        const r = await fetch(Platform.url + '/обои?p=' + encodeURIComponent(путь));
        return { код:r.status, тип:r.headers.get('content-type') || '' };
      }, свои[0].путь);
      check('обои системы отдаются страницей',
        код.код === 200 && /^image\//.test(код.тип), JSON.stringify(код));
    } else {
      check('обои системы отдаются страницей', true, 'на этой машине обоев нет — проверять нечего');
    }
    const чужой = await page.evaluate(async () => {
      const r = await fetch(Platform.url + '/обои?p=' + encodeURIComponent('/etc/passwd'));
      return r.status;
    });
    check('чужие файлы этой дорожкой не отдаются', чужой === 403, 'ответ ' + чужой);
    const вверх = await page.evaluate(async () => {
      const r = await fetch(Platform.url + '/обои?p='
        + encodeURIComponent('/usr/share/backgrounds/../../../etc/shadow.png'));
      return r.status;
    });
    check('и путём «вверх» тоже не отдаются', вверх === 403, 'ответ ' + вверх);

    /* Второй одинаковый набор «Персонализации» однажды уже съел правку:
       объявления поднимаются, работает последнее, и изменения в первом
       наборе не делали ровно ничего. */
    /* Сочетания клавиш зовут наши маленькие программы. Если такую
       переименовать или забыть положить в образ, оконный сервер будет
       честно звать её при каждом нажатии, а не произойдёт ничего — и
       понять это можно только на живой машине. */
    const rc = await readFile(join(root, 'linux', 'labwc', 'rc.xml'), 'utf8');
    const образ2 = await readFile(join(root, 'linux', 'mkiso.sh'), 'utf8');
    const пакет = await readFile(join(root, 'linux', 'собери-пакет.sh'), 'utf8');
    const зовёт = [...rc.matchAll(/command="(glower-[\w-]+)/g)].map(m => m[1]);
    const наши = [...new Set(зовёт)];
    for (const прог of наши){
      check('сочетания зовут ' + прог + ' — она есть в дереве',
        existsSync(join(root, 'linux', прог)));
      check('и она попадает в образ', образ2.includes('linux/' + прог));
      check('и в пакет обновления', пакет.includes('usr/bin/' + прог));
    }

    /* Окна системы живут внутри страницы стола, и оконный сервер видит их
       одним окном. Alt+F4, Alt+Tab и Win со стрелками на столе он должен
       отдавать странице, иначе до неё они не доходят вовсе: Alt+F4 на
       открытых Параметрах не делал ничего. */
    {
      const привязка = к => (rc.match(new RegExp('<keybind key="' + к + '">([\\s\\S]*?)</keybind>')) || [])[1] || '';
      const не_так = ['A-F4', 'A-Tab', 'W-q', 'W-Left', 'W-Right', 'W-Up', 'W-Down'].filter(к => {
        const п = привязка(к);
        return !(/<query identifier="glowershell" \/>/.test(п)
          && п.includes('glower-menu клавиша ' + к) && /<else>[\s\S]*<action name="(Close|NextWindow|SnapToEdge)"/.test(п));
      });
      check('на столе сочетания уходят странице, в чужих окнах — как прежде', !не_так.length,
        'не так: ' + не_так.join(', '));
      /* Win+L — из любой программы, а замок — без анимации кольца */
      {
        const меню_ = await readFile(join(root, 'linux', 'glower-menu'), 'utf8');
        const сис = await readFile(join(root, 'agent', 'system.mjs'), 'utf8');
        check('Win+L запирает из любой программы, замок без кольца и белого экрана',
          привязка('W-l').includes('glower-menu замок') && /"\$\{1:-\}" = "замок"/.test(меню_)
            && /"action":"lock"/.test(меню_)
            && /swaylock:\['-f', '-u', '-c', '0b0d1a'\]/.test(сис) && /i3lock:\['-c', '0b0d1a', '-u'\]/.test(сис));
      }
      check('Alt со стрелками остаётся программам («назад» в браузере)',
        ['A-Left', 'A-Right', 'A-Up', 'A-Down'].every(к => привязка(к).trim() === '<action name="None" />'));
      const меню = await readFile(join(root, 'linux', 'glower-menu'), 'utf8');
      const стол = await readFile(join(root, 'js', 'поверхности.js'), 'utf8');
      check('glower-menu передаёт сочетание странице, а страница его слушает',
        /"\$\{1:-\}" = "клавиша"/.test(меню) && /"тема":"клавиша"/.test(меню)
          && /слушай\(\['клавиша'\]/.test(стол) && /case 'A-F4'/.test(стол) && /case 'A-Tab'/.test(стол));
      /* WebKit называет Win «Super», и metaKey при ней у страницы ложный:
         Win+E, Win+R, Win+Tab не работали ни одно. Оболочка вручает их сама. */
      const оболочка = await readFile(join(root, 'linux', 'glower-shell'), 'utf8');
      check('оболочка вручает странице сочетания с Win как metaKey',
        /def win_для_страницы/.test(оболочка) && /metaKey:true/.test(оболочка)
          && (оболочка.match(/win_для_страницы\(окно, в\)/g) || []).length >= 2);
    }

    /* Нативная панель (panel/, Rust). Она должна доехать и до образа, и до
       машин через пакет обновления, собираться на GitHub до образа — и не
       оставить человека без панели вовсе, если с ней что-то не так. */
    {
      const cargo = await readFile(join(root, 'panel', 'Cargo.toml'), 'utf8');
      const сборка = await readFile(join(root, '..', '.github', 'workflows', 'build-iso.yml'), 'utf8');
      const оболочка = await readFile(join(root, 'linux', 'glower-shell'), 'utf8');
      check('панель на Rust: проект и замок версий на месте',
        /name = "glower-panel"/.test(cargo) && /gtk-layer-shell/.test(cargo)
          && existsSync(join(root, 'panel', 'Cargo.lock')) && existsSync(join(root, 'panel', 'src', 'main.rs')));
      check('GitHub собирает и проверяет панель до образа',
        /cargo test --release --locked --manifest-path glower\/panel\/Cargo.toml/.test(сборка)
          && /cargo build --release --locked --manifest-path glower\/panel\/Cargo.toml/.test(сборка)
          && /libgtk-layer-shell-dev/.test(сборка)
          && сборка.indexOf('Собрать панель') < сборка.indexOf('Собрать образ'));
      check('выпуск и репозиторий берут пакет любой архитектуры',
        !/glower_\*_all\.deb/.test(сборка) && (сборка.match(/glower_\*\.deb/g) || []).length >= 2);
      check('панель кладётся в образ и в пакет обновления (тогда он amd64)',
        /panel\/target\/release\/glower-panel/.test(образ2) && /usr\/bin\/glower-panel/.test(образ2)
          && пакет.includes('usr/bin/glower-panel') && /ARCH=amd64/.test(пакет)
          && /glower_\$\{DEB_VER\}_\$\{ARCH\}\.deb/.test(пакет) && /libgtk-layer-shell0/.test(пакет));
      check('оболочка поднимает нативную панель, а при беде — страничную',
        /shutil\.which\('glower-panel'\)/.test(оболочка) && /def страничная_панель/.test(оболочка)
          && /падений\['подряд'\] >= 2/.test(оболочка) && /get_exit_status\(\) == 3/.test(оболочка)
          && /if not нативная_панель\(\):\s*страничная_панель\(\)/.test(оболочка));
      /* Нативная говорит на той же шине, что страничная: стол ждёт от
         панели «готова» и её рост, и слушает она те же темы. */
      const главная = await readFile(join(root, 'panel', 'src', 'main.rs'), 'utf8');
      const страничная = await readFile(join(root, 'js', 'панель.js'), 'utf8');
      const темы = ['раскладка', 'часы', 'батарея', 'обновления', 'настройки', 'стол-занят', 'затемни', 'открой-пуск'];
      const меню3 = await readFile(join(root, 'linux', 'glower-menu'), 'utf8');
      const обзор = await readFile(join(root, 'panel', 'src', 'overview.rs'), 'utf8');
      check('нативное меню программ: панель берёт его себе, страничная — отдаёт',
        /скажи\("обзор-свой", json!\(true\)\)/.test(главная) && /Шина\.скажи\('обзор-свой', false\)/.test(страничная)
          && /"программы" => overview::программы/.test(главная) && /"обзор-данные" => overview::данные/.test(главная)
          && /скажи\("обзор-дай"/.test(обзор) && /скажи\("обзор-дело"/.test(обзор)
          && /DesktopAppInfo::new/.test(обзор)
          && /drag_source_set/.test(обзор) && /drag_dest_set/.test(обзор)
          /* Как Launchpad: поверх всего, вместе с панелью, и с настоящим
             затемнением — самоотрисовываемое окно GTK само фон не льёт */
          && /set_layer\(Layer::Overlay\)/.test(обзор) && /set_exclusive_zone\(-1\)/.test(обзор)
          && /set_source_rgba\(0\.04, 0\.047, 0\.08, 0\.86\)/.test(обзор)
          && /"на-стол"/.test(обзор) && /"окно-на-стол"/.test(обзор) && /"окно-на-новый-стол"/.test(обзор));
      const док_ = await readFile(join(root, 'panel', 'src', 'dock.rs'), 'utf8');
      check('нативный док: поверх окон, прячется при окнах и выезжает у края',
        /mod dock;/.test(главная) && /dock::создай\(\)/.test(главная) && /"док" => dock::данные/.test(главная)
          && /скажи\("док-свой", json!\(true\)\)/.test(главная) && /Шина\.скажи\('док-свой', false\)/.test(страничная)
          && /set_layer\(Layer::Top\)/.test(док_) && /glower-dock-edge/.test(док_)
          && /"sys\.windows"/.test(док_) && /меню_открыто/.test(док_));
      const стол_ = await readFile(join(root, 'panel', 'src', 'desktop.rs'), 'utf8');
      const оболочка2 = await readFile(join(root, 'linux', 'glower-shell'), 'utf8');
      check('нативный стол: нижний слой, обои и значки, страница над ним сквозная',
        /mod desktop;/.test(главная) && /desktop::создай\(\)/.test(главная)
          && /"значки" => desktop::значки/.test(главная) && /"обои" => desktop::обои/.test(главная)
          && /скажи\("стол-свой", json!\(true\)\)/.test(главная) && /Шина\.скажи\('стол-свой', false\)/.test(страничная)
          && /set_layer\(Layer::Background\)/.test(стол_) && /"значки-дело"/.test(стол_)
          && /def сквозь_пустое/.test(оболочка2) && /glowerDesk/.test(оболочка2)
          && /input_shape_combine_region\(область/.test(оболочка2) && /сквозь_пустое\(окно, в\)/.test(оболочка2));
      /* Замок по Win+L и вход при включении — один нативный экран */
      {
        const замок = await readFile(join(root, 'panel', 'src', 'lock.rs'), 'utf8');
        const сис2 = await readFile(join(root, 'agent', 'system.mjs'), 'utf8');
        const вход = await readFile(join(root, 'linux', 'glower-greeter'), 'utf8');
        check('замок настоящий: ввод заперт у оконного сервера, пароль проверяет PAM',
          /get_inhibitor/.test(замок) && /pam_authenticate/.test(замок) && /Layer::Overlay/.test(замок)
            && /KeyboardMode::Exclusive/.test(замок) && /glower-lock" \} else \{ "login"/.test(замок));
        check('Win+L запускает свой замок, чужие — запасными',
          /'glower-lock':\[\]/.test(сис2) && /\['glower-lock', 'swaylock'/.test(сис2)
            && /имя == "glower-lock"/.test(главная) && /имя == "glower-login"/.test(главная));
        check('при включении — тот же экран, а не поднялся — страничный',
          /\/usr\/bin\/glower-login/.test(вход) && /-ge 5/.test(вход) && /GLOWER_QUERY="login=1"/.test(вход)
            && /Режим::Вход => позови\("login\.users"/.test(замок) && /позови\("login\.enter"/.test(замок));
        check('замок и вход попадают в образ и пакет, PAM — со своей службой',
          /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-lock"/.test(образ2) && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-login"/.test(образ2)
            && /pam-glower-lock/.test(образ2) && existsSync(join(root, 'linux', 'pam-glower-lock'))
            && пакет.includes('usr/bin/glower-lock') && пакет.includes('etc/pam.d/glower-lock') && /libpam0g/.test(пакет)
            && /libpam0g-dev/.test(сборка));
      }
      /* Блокнот и Калькулятор — нативные окна, та же программа под своим именем */
      {
        const блокнот = await readFile(join(root, 'panel', 'src', 'notepad.rs'), 'utf8');
        const счёт = await readFile(join(root, 'panel', 'src', 'calc.rs'), 'utf8');
        const общее = await readFile(join(root, 'panel', 'src', 'apps.rs'), 'utf8');
        const агент2 = await readFile(join(root, 'agent', 'server.mjs'), 'utf8');
        const ос2 = await readFile(join(root, 'js', 'os.js'), 'utf8');
        const сис3 = await readFile(join(root, 'js', 'system.js'), 'utf8');
        check('Блокнот и Калькулятор на Rust: своё имя окна, вкладки, сохранение, счёт',
          /имя == "glower-notepad"/.test(главная) && /имя == "glower-calc"/.test(главная)
            && /set_prgname/.test(общее) && /gtk::Notebook/.test(блокнот) && /Сохранить как/.test(блокнот)
            && /fn нажми/.test(счёт) && /считает_как_страничный/.test(счёт));
        check('агент запускает родные программы, страница отдаёт им свои окна',
          /'sys\.native\.list'/.test(агент2) && /'sys\.native'/.test(агент2) && /glower-notepad/.test(агент2)
            && /function wireNativeSystemApps/.test(ос2) && /wireNativeSystemApps\(\)/.test(ос2)
            && /прежнее\(id, opts\)\)/.test(ос2));
        check('восстановление сеанса не плодит пустые Блокноты при входе',
          /OS\.нативные\.includes\(s\.app\)/.test(сис3) && /'восстановление':true/.test(сис3)
            && /opts\['восстановление'\]\) return null/.test(ос2));
        check('в доке окна родных программ — точкой под их значком, не чужим',
          /"glower-notepad", "notepad"/.test(док_) && /"glower-calc", "calc"/.test(док_) && /родные_для/.test(док_));
        const проводник = await readFile(join(root, 'panel', 'src', 'explorer.rs'), 'utf8');
        check('Проводник на Rust: вкладки, сетка и таблица, корзина системы, флешки сбоку',
          /имя == "glower-files"/.test(главная) && /gtk::IconView/.test(проводник) && /gtk::TreeView/.test(проводник)
            && /Место::Корзина/.test(проводник) && /\.trash\(/.test(проводник) && /VolumeMonitor/.test(проводник)
            && /monitor_directory/.test(проводник) && /text\/uri-list/.test(проводник) && /GLOWER_ROOT/.test(проводник)
            && /files:'glower-files'/.test(агент2) && /GLOWER_ROOT:ROOT/.test(агент2)
            && /id === 'files' && opts && Array\.isArray\(opts\.path\)/.test(ос2)
            && /"glower-files", "files"/.test(док_) && /adwaita-icon-theme/.test(образ2) && /adwaita-icon-theme/.test(пакет));
        const параметры = await readFile(join(root, 'panel', 'src', 'settings.rs'), 'utf8');
        const мост = await readFile(join(root, 'js', 'параметры.js'), 'utf8');
        check('Параметры на Rust: все двадцать разделов, системное — у агента, оболочечное — через страницу',
          /имя == "glower-settings"/.test(главная) && (параметры.match(/Раздел \{ id:/g) || []).length === 20
            && ['sys.wifi.scan', 'sys.bt', 'sys.printers', 'sys.drives', 'sys.users', 'pkg.upgrade.check', 'sys.firmware', 'sys.firewall', 'sys.screens', 'sys.hardware']
                 .every(м => параметры.includes('"' + м + '"'))
            && /"параметры-дело"/.test(параметры) && /слушай\(&\["параметры"\]/.test(параметры)
            && /settings:'glower-settings'/.test(агент2) && /\^\[a-z0-9\]\{1,20\}\$/.test(агент2)
            && /'параметры-раздел'/.test(ос2) && /"glower-settings", "settings"/.test(док_)
            && /Поверхности\.слушай\(\['параметры-дело'\]/.test(мост) && /js\/параметры\.js/.test(await readFile(join(root, 'index.html'), 'utf8')));
        const магазин = await readFile(join(root, 'panel', 'src', 'store.rs'), 'utf8');
        const каталог = JSON.parse(await readFile(join(root, 'panel', 'src', 'store-catalog.json'), 'utf8'));
        /* Подборки страничного и нативного Магазина — одни и те же */
        const магазинСтр = await readFile(join(root, 'js', 'store.js'), 'utf8');
        const достань = (имя, откр, закр) => {
          const i = магазинСтр.indexOf('const ' + имя + ' = ' + откр);
          let d = 0; const j = магазинСтр.indexOf(откр, i);
          for (let k = j; k < магазинСтр.length; k++){
            if (магазинСтр[k] === откр) d++;
            else if (магазинСтр[k] === закр && --d === 0) return Function('return ' + магазинСтр.slice(j, k + 1))();
          }
        };
        check('Магазин на Rust: витрина, разделы, поиск, установка, обновления, «Моё»',
          /имя == "glower-store"/.test(главная) && ['"pkg.install"', '"pkg.remove"', '"pkg.search"', '"pkg.info"', '"pkg.job"', '"pkg.flathub"', '"pkg.upgrade.check"', '"sys.apps"', '"store.icon"']
              .every(м => магазин.includes(м))
            && /store:'glower-store'/.test(агент2) && /'store\.icon'/.test(агент2) && /'--find=' \+/.test(агент2)
            && /'магазин-открой'/.test(ос2) && /"glower-store", "store"/.test(док_) && /steam-devices/.test(магазин));
        check('подборки нативного Магазина совпадают со страничными',
          JSON.stringify(каталог['подборки']) === JSON.stringify(достань('ПОДБОРКИ', '{', '}'))
            && JSON.stringify(каталог['витрина']) === JSON.stringify(достань('ВИТРИНА', '[', ']'))
            && JSON.stringify(каталог['разделы']) === JSON.stringify(достань('РАЗДЕЛЫ', '[', ']')));
        check('корзина одна: стол и Проводник кладут в системную, док открывает её Проводником',
          /'fs\.trash'/.test(агент2) && /'fs\.trash\.empty'/.test(агент2) && /GLOWER_TRASH/.test(агент2)
            && /permanent \? 'fs\.remove' : 'fs\.trash'/.test(await readFile(join(root, 'js', 'platform.js'), 'utf8'))
            && /"--trash"/.test(проводник) && /fn открой_корзину/.test(док_) && /id === 'trash' && есть\.includes\('files'\)/.test(ос2));
        check('Блокнот, Калькулятор, Проводник, Параметры и Магазин попадают в образ и пакет',
          /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-notepad"/.test(образ2) && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-calc"/.test(образ2)
            && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-files"/.test(образ2)
            && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-settings"/.test(образ2) && пакет.includes('usr/bin/glower-settings')
            && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-store"/.test(образ2) && пакет.includes('usr/bin/glower-store')
            && пакет.includes('usr/bin/glower-notepad') && пакет.includes('usr/bin/glower-calc') && пакет.includes('usr/bin/glower-files'));
      }
      check('Win при нативном меню не поднимает стол поверх программ',
        /"тема":"обзор-свой"/.test(меню3) && /\*'"что":true'\*\) ;;/.test(меню3));
      check('нативная панель слушает те же темы шины, что страничная',
        темы.every(т => страничная.includes("'" + т + "'") && главная.includes('"' + т + '"'))
          && /скажи\("панель-готова"/.test(главная) && /скажи\("полоса-высота"/.test(главная));
    }
    check('столы переключаются с клавиатуры', /glower-desk/.test(rc));

    /* Заставка загрузки: каждая картинка, которую зовёт сценарий, едет в
       образ (logo.png кладёт сборка из assets), а пакет просит пересобрать
       initramfs — иначе новую заставку машина увидит только с новым ядром. */
    {
      const { readdir } = await import('node:fs/promises');
      const сцен = await readFile(join(root, 'linux', 'plymouth', 'glower.script'), 'utf8');
      const зовёт = [...сцен.matchAll(/Image\("([^"]+)"\)/g)].map(m => m[1]);
      const есть = await readdir(join(root, 'linux', 'plymouth'));
      const нет = зовёт.filter(ф => ф !== 'logo.png' && !есть.includes(ф));
      check('заставка: все картинки сценария есть в теме', зовёт.includes('logo.png') && !нет.length,
        JSON.stringify({ зовёт, нет }));
      check('заставка: сборка кладёт все картинки темы, а пакет пересобирает initramfs',
        /linux\/plymouth\/\*\.png/.test(образ2) && /\[ -e "\$img" \] \|\| continue/.test(образ2)
          && /activate-noawait update-initramfs/.test(пакет));
      /* Анимации при включении, выключении и входе убраны по просьбе
         человека — осталось одно приветствие после первой настройки. */
      check('заставка загрузки неподвижна: без обновления кадров',
        !/SetRefreshFunction/.test(сцен) && !/Rotate\(/.test(сцен) && /Plymouth\.GetMode/.test(сцен)
          && /SetDisplayPasswordFunction/.test(сцен));
    }

    /* Программы Windows: glower-exe и его ярлык едут и в образ, и в пакет
       обновления — иначе машины, поставленные раньше, .exe открывать не
       научатся. И типы, на которые отзывается ярлык, те же, что сеанс
       записывает человеку: разойдись списки — часть .exe уйдёт в редактор. */
    {
      const сеанс = await readFile(join(root, 'linux', 'glower-session'), 'utf8');
      check('glower-exe есть в дереве и исполняемый',
        existsSync(join(root, 'linux', 'glower-exe'))
          && (statSync(join(root, 'linux', 'glower-exe')).mode & 0o111) !== 0);
      check('glower-exe попадает в образ и в пакет',
        образ2.includes('linux/glower-exe') && пакет.includes('usr/bin/glower-exe')
          && пакет.includes('usr/share/applications/glower-exe.desktop'));
      const типы = ((образ2.match(/Exec=\/usr\/bin\/glower-exe %f[\s\S]*?MimeType=([^\n]+)/) || [])[1] || '')
        .split(';').filter(Boolean).sort();
      const уЧеловека = [...сеанс.matchAll(/([\w./+-]+)=glower-exe\.desktop/g)].map(m => m[1]).sort();
      const вСписке = [...образ2.matchAll(/^([\w./+-]+)=glower-exe\.desktop$/gm)].map(m => m[1]).sort();
      check('типы .exe у ярлыка, в списке образа и у человека совпадают',
        типы.length >= 4 && JSON.stringify(типы) === JSON.stringify(уЧеловека)
          && JSON.stringify(типы) === JSON.stringify(вСписке),
        JSON.stringify({ типы, уЧеловека, вСписке }));
      /* Сам Wine в образ не кладём: с ним образ перерос 2 ГБ, и GitHub
         отказался его выкладывать (v11.8). Его ставит агент, а образ лишь
         заранее разрешает архитектуру i386 для его 32-битной половины. */
      const пакеты = await readFile(join(root, 'agent', 'packages.mjs'), 'utf8');
      check('Wine не раздувает образ, а систему к нему образ готовит',
        /add-architecture i386/.test(образ2) && !/apt-get install[^\n]*wine/.test(образ2));
      check('агент сам доставляет Wine после входа, но не с носителя',
        /доставьWine\(true\)/.test(пакеты) && /живая\(\)/.test(пакеты.slice(пакеты.indexOf('GLOWER_NO_AUTO_WINE'))));
    }

    /* --- Bluetooth: кнопка там, где она поможет, и только там ---

       Система сама писала «выключен программно — его можно включить» и
       сама же не давала ничем: строка без кнопки, плитка в панели гасла.
       Проверяем все четыре состояния разом, подменив ответ машины: на этой
       Bluetooth может не быть вовсе, а поведение должно быть верным на
       любой. */
    for (const [случай, ответ, ждём] of [
      ['выключен программно',
       { есть:false, почему:'Bluetooth выключен программно — его можно включить',
         подробно:{ блокировка:{ программно:true, рубильником:false }, железо:'Intel' } }, 'Включить'],
      ['выключен кнопкой на корпусе',
       { есть:false, почему:'Bluetooth выключен переключателем на корпусе или клавишей на клавиатуре',
         подробно:{ блокировка:{ программно:true, рубильником:true }, железо:'Intel' } }, null],
      ['адаптера нет вовсе',
       { есть:false, почему:'Bluetooth-адаптера на этой машине не видно',
         подробно:{ блокировка:null, железо:null } }, null],
      ['нечем управлять',
       { есть:false, почему:'на машине нет bluetoothctl — Bluetooth не настроен' }, null],
      ['ядро не дало хода',
       { есть:false, почему:'Адаптер в машине есть, но ядро не дало ему хода — похоже, не хватает прошивки',
         подробно:{ блокировка:{ программно:false, рубильником:false }, железо:'Intel' } },
       'Попробовать включить']
    ]){
      const вышло = await page.evaluate(async о => {
        const было = Platform.rpc.bind(Platform);
        window.__звали = [];
        Platform.rpc = (m, p) => {
          if (m === 'sys.bt' || m === 'sys.bt.scan') return Promise.resolve(о);
          if (m === 'sys.bt.power'){ window.__звали.push(p); return Promise.resolve({ ok:true }); }
          return было(m, p);
        };
        WM.wins.filter(w => w.appId === 'settings').forEach(w => WM.close(w));
        await new Promise(r => setTimeout(r, 300));
        WM.open('settings', { section:'bt' });
        await new Promise(r => setTimeout(r, 1500));
        const w = WM.wins.find(x => x.appId === 'settings');
        const б = [...w.body.querySelectorAll('button')]
          .find(b => /^(Включить|Попробовать включить)$/.test(b.textContent.trim()));
        /* Подпись читаем до нажатия: нажатая кнопка пишет «Включаю…». */
        const подпись = б ? б.textContent.trim() : null;
        if (б) б.click();
        await new Promise(r => setTimeout(r, 400));
        const звали = window.__звали.slice();
        Platform.rpc = было;
        WM.wins.filter(x => x.appId === 'settings').forEach(x => WM.close(x));
        return { кнопка:подпись, звали };
      }, ответ);
      check('Bluetooth, ' + случай + ': кнопка ' + (ждём ? '«' + ждём + '»' : 'не нужна'),
        (вышло.кнопка || null) === ждём, 'вышло: ' + вышло.кнопка);
      if (ждём)
        check('и она просит машину включить Bluetooth',
          вышло.звали.length === 1 && вышло.звали[0]['включить'] === true,
          JSON.stringify(вышло.звали));
    }

    /* Части самой системы из меню программ не пропадают, что бы ни поставил
       человек.

       У человека пропал Магазин: он поставил программу, в ярлыке которой
       оказалось то же слово, — и наш Магазин вычеркнулся из списка как
       «дубль». Найти его после этого было негде: он и есть то место,
       откуда ставят программы. */
    const своиНаМесте = await page.evaluate(() => {
      const было = window.OS;
      window.OS = { машинные:[{ id:'чужой.desktop', name:'Магазин' },
                              { id:'ещё.desktop', name:'Параметры' }] };
      const список = (window.Поверхности && Поверхности.программы)
        ? Поверхности.программы() : null;
      window.OS = было;
      if (!список) return { нет:true };
      const свои = список.filter(п => п['вид'] === 'приложение').map(п => п.id);
      return { магазин:свои.includes('store'), параметры:свои.includes('settings'),
               всего:список.length };
    });
    check('свой Магазин не исчезает из-за чужой программы с тем же именем',
      своиНаМесте.магазин === true && своиНаМесте.параметры === true,
      JSON.stringify(своиНаМесте));

    /* Проба выделения области — та, что показывает «врач». */
    const проба = await page.evaluate(() =>
      Platform.rpc('sys.shot.проба').catch(e => ({ ошибка:String(e.message || e) })));
    check('агент умеет проверить, работает ли выделение области',
      проба && проба.есть && 'grim' in проба.есть, JSON.stringify(проба).slice(0, 120));

    const настройки = await readFile(join(root, 'js', 'apps.js'), 'utf8');
    const сколько = (настройки.match(/function pPerson\(/g) || []).length;
    check('набор «Персонализации» в «Параметрах» один', сколько === 1, 'нашлось: ' + сколько);
  }

  /* --- обновление системы как в Windows: обновить, потом перезагрузить ---
     Система подменяется: здесь не место ни настоящему apt, ни настоящей
     перезагрузке. Проверяем сам ход — экран, шаги и то, что питание зовут
     ровно один раз и только после удачного обновления. */
  for (const [случай, удачно] of [['удачно', true], ['с отказом', false]]){
    const итог = await page.evaluate(async удачно => {
      const было = Platform.rpc.bind(Platform);
      let шаг = 0; const питание = [];
      Platform.rpc = (m, p) => {
        if (m === 'pkg.upgrade.run') return Promise.resolve({ started:true });
        if (m === 'pkg.job'){ шаг++; return Promise.resolve(шаг < 3
          ? { running:true, step:'Устанавливаю', percent:шаг * 30 }
          : (удачно ? { running:false, ok:true } : { running:false, ok:false, error:'нет места на диске' })); }
        if (m === 'sys.power'){ питание.push(p.action); return Promise.resolve({ ok:true }); }
        return было(m, p);
      };
      const ждём = OS.обновиИ('reboot');
      await new Promise(r => setTimeout(r, 400));
      const экранБыл = !!document.querySelector('.обновление-системы');
      await Promise.race([ждём, new Promise(r => setTimeout(r, 9000))]);
      const экран = document.querySelector('.обновление-системы');
      const кнопки = экран ? [...экран.querySelectorAll('button')].map(b => b.textContent) : [];
      Platform.rpc = было;
      if (экран) экран.remove();
      return { экранБыл, питание, кнопки };
    }, удачно);
    check('обновление системы ' + случай + ': экран «не выключайте компьютер»', итог.экранБыл === true);
    if (удачно)
      check('после обновления машина перезагружается — один раз',
        итог.питание.length === 1 && итог.питание[0] === 'reboot', JSON.stringify(итог));
    else
      check('при отказе не перезагружает молча, а даёт выбор',
        итог.питание.length === 0 && итог.кнопки.some(т => /без обновления/.test(т))
          && итог.кнопки.some(т => /Вернуться/.test(т)), JSON.stringify(итог));
  }

  /* --- обновление подтягивает новое ядро ---
     Новое ядро Ubuntu приходит новым пакетом, а обычное apt-get upgrade
     новых пакетов не ставит: ядро «придерживалось» и навсегда оставалось
     тем, что было в образе. Проверяем, что и проверка обновлений, и сами
     обновления идут с разрешением ставить новое. */
  {
    const пакеты = await readFile(join(root, 'agent', 'packages.mjs'), 'utf8');
    const вызовы = пакеты.split('\n').filter(с => /'upgrade'/.test(с) && !/^\s*\/\*|^\s*\*/.test(с));
    check('обновление системы подтягивает новое ядро',
      вызовы.length >= 2 && вызовы.every(с => /--with-new-pkgs/.test(с)),
      вызовы.map(с => с.trim()).join(' | '));
  }

  /* --- программа называется своим именем, а не именем своего подменю ---

     У человека Steam в меню назывался «Магазин» и вытеснял наш Магазин.
     В ярлыке Steam есть подменю «Store» с русским именем «Магазин»; у самой
     программы русского имени нет, и разбор, шедший по всему файлу подряд,
     брал первое русское имя, какое находил. Поднимаем отдельного агента
     со своим домом и русским языком и кладём ему такой же ярлык. */
  {
    const { mkdir, writeFile } = await import('node:fs/promises');
    const дом = await mkdtemp(join(tmpdir(), 'glower-дом-'));
    await mkdir(join(дом, '.local/share/applications'), { recursive:true });
    await writeFile(join(дом, '.local/share/applications', 'проба-steam.desktop'), [
      '[Desktop Entry]', 'Name=Steam', 'Comment=Games', 'Exec=true', 'Type=Application',
      'Actions=Store;', '',
      '[Desktop Action Store]', 'Name=Store', 'Name[ru]=Магазин', 'Exec=true', ''
    ].join('\n'));
    const порт2 = PORT + 300;
    const второй = spawn(process.execPath,
      [join(root, 'agent/server.mjs'), '--port', String(порт2), '--root', дом, '--system'],
      { stdio:'ignore', env:{ ...process.env, HOME:дом, LANG:'ru_RU.UTF-8' } });
    await new Promise(r => setTimeout(r, 1500));
    let имя = null;
    try {
      const r = await fetch(`http://localhost:${порт2}/rpc`, { method:'POST',
        headers:{ 'Content-Type':'application/json' },
        body:JSON.stringify({ method:'sys.apps', params:{} }) });
      const д = await r.json();
      имя = ((д.result && д.result.list) || []).find(a => a.id === 'проба-steam.desktop');
      имя = имя ? имя.name : '(нет в списке)';
    } catch(e){ имя = 'агент не ответил: ' + e.message; }
    второй.kill();
    check('программа называется своим именем, а не именем подменю', имя === 'Steam', 'вышло: ' + имя);
  }

  /* --- программы Windows: ярлык Wine в меню и запуск ---

     Wine кладёт ярлык каждой поставленной программы в подпапку
     applications/wine/Programs/…, и строка запуска у него с кавычками
     посередине и экранированным пробелом. Прежде такой ярлык не был виден
     вовсе (подпапки не смотрели), а будь виден — запустился бы с испорченной
     папкой Windows. Wine здесь подставной: он только записывает, с чем его
     позвали. Настоящего Wine на стенде нет, и тянуть его ради теста незачем —
     проверяем то, что делаем мы. */
  {
    const { mkdir, writeFile, readFile, chmod, rm } = await import('node:fs/promises');
    const дом = await mkdtemp(join(tmpdir(), 'glower-win-'));
    const бин = join(дом, 'bin');
    await mkdir(бин, { recursive:true });
    await writeFile(join(бин, 'wine'),
      '#!/bin/sh\nprintf "%s\\n" "PREFIX=$WINEPREFIX" "$@" > "$HOME/wine-позвали"\n');
    await chmod(join(бин, 'wine'), 0o755);
    const ярл = join(дом, '.local/share/applications/wine/Programs/Notepad++');
    await mkdir(ярл, { recursive:true });
    await writeFile(join(ярл, 'Notepad++.desktop'), [
      '[Desktop Entry]', 'Name=Notepad++', 'Type=Application',
      'Exec=env WINEPREFIX="' + дом + '/.wine" wine C:\\\\\\\\ProgramData\\\\\\\\Microsoft\\\\\\\\Windows\\\\\\\\Start\\\\ Menu\\\\\\\\Programs\\\\\\\\Notepad++.lnk',
      'Icon=A1B2_notepad++.0', ''
    ].join('\n'));
    await mkdir(join(дом, 'Загрузки'), { recursive:true });
    await writeFile(join(дом, 'Загрузки', 'My Setup.exe'), 'MZ');

    const зов = async (порт, method, params = {}) => {
      const r = await fetch(`http://localhost:${порт}/rpc`, { method:'POST',
        headers:{ 'Content-Type':'application/json' }, body:JSON.stringify({ method, params }) });
      return r.json();
    };
    const подними = (порт, путь) => spawn(process.execPath,
      [join(root, 'agent/server.mjs'), '--port', String(порт), '--root', дом, '--system', '--allow-launch'],
      { stdio:'ignore', env:{ ...process.env, HOME:дом, PATH:путь } });
    const позвали = async () => { try { return (await readFile(join(дом, 'wine-позвали'), 'utf8')).split('\n'); } catch(e){ return []; } };

    const порт3 = PORT + 301;
    const с_wine = подними(порт3, бин + ':' + process.env.PATH);
    await new Promise(r => setTimeout(r, 1500));
    let ид = null, запуск = null, изПроводника = null, вызов1 = [], вызов2 = [];
    try {
      const д = await зов(порт3, 'sys.apps');
      const п = ((д.result && д.result.list) || []).find(a => a.name === 'Notepad++');
      ид = п ? п.id : null;
      if (ид){
        запуск = await зов(порт3, 'sys.launch', { id:ид });
        await new Promise(r => setTimeout(r, 400));
        вызов1 = await позвали();
      }
      await rm(join(дом, 'wine-позвали'), { force:true });
      изПроводника = await зов(порт3, 'sys.windows.open', { path:'Загрузки/My Setup.exe' });
      await new Promise(r => setTimeout(r, 600));
      вызов2 = await позвали();
    } catch(e){ запуск = { error:e.message }; }
    с_wine.kill();

    check('программа Windows из подпапки Wine видна в меню',
      ид === 'wine-Programs-Notepad++-Notepad++.desktop', 'ид: ' + ид);
    check('ярлык Wine запускается с верной папкой и целым путём',
      запуск && запуск.ok && вызов1[0] === 'PREFIX=' + дом + '/.wine'
        && вызов1[1] === 'C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Notepad++.lnk',
      JSON.stringify({ запуск, вызов1 }));
    check('.exe из Проводника уходит в Wine целым путём',
      изПроводника && изПроводника.ok && изПроводника.result.ok
        && вызов2.includes(join(дом, 'Загрузки', 'My Setup.exe')),
      JSON.stringify({ изПроводника, вызов2 }));

    /* Без Wine — не ошибка, а ответ, по которому оболочка предложит его поставить. */
    const порт4 = PORT + 302;
    const без_wine = подними(порт4, '/usr/bin:/bin');
    await new Promise(r => setTimeout(r, 1500));
    let нет = null;
    try { нет = await зов(порт4, 'sys.windows.open', { path:'Загрузки/My Setup.exe' }); }
    catch(e){ нет = { error:e.message }; }
    без_wine.kill();
    check('без Wine система говорит «нет Wine», а не падает',
      нет && нет.ok && нет.result['нетWine'] === true, JSON.stringify(нет));
    check('чужое расширение за программу Windows не выдать',
      /не программа Windows/.test(JSON.stringify(await (async () => {
        const п = подними(порт4, бин + ':' + process.env.PATH);
        await new Promise(r => setTimeout(r, 1500));
        await writeFile(join(дом, 'Загрузки', 'note.txt'), 'x');
        try { return await зов(порт4, 'sys.windows.open', { path:'Загрузки/note.txt' }); }
        finally { п.kill(); }
      })())), '');
  }

  /* --- обновление системы доставляет Wine, и его неудача обновление не портит ---

     apt, dpkg и sudo подставные: они записывают, что с ними делали. Прошлая
     версия этой очереди проверяла Wine уже после старта обновления, и
     быстрое обновление успевало закончиться раньше — Wine не приходил. */
  {
    const { mkdir, writeFile, chmod, readFile: читай } = await import('node:fs/promises');
    const папка = await mkdtemp(join(tmpdir(), 'glower-apt-'));
    const бин = join(папка, 'bin');
    await mkdir(бин);
    const скрипт = (имя, тело) => writeFile(join(бин, имя), '#!/bin/sh\n' + тело).then(() => chmod(join(бин, имя), 0o755));
    await скрипт('sudo', '[ "$1" = "-n" ] && shift\nexec "$@"\n');
    await скрипт('apt-get', 'echo "apt-get $*" >> "$APTLOG"\ncase "$*" in *wine32*) [ -n "${WINE_FAIL:-}" ] && { echo "E: Unable to locate package wine32" >&2; exit 100; } ;; esac\nexit 0\n');
    await скрипт('dpkg', 'echo "dpkg $*" >> "$APTLOG"\nexit 0\n');
    await скрипт('dpkg-query', 'exit 1\n');
    await writeFile(join(папка, 't.mjs'),
      "import { packages } from " + JSON.stringify(pathToFileURL(join(root, 'agent/packages.mjs')).href) + ";\n" +
      "const p = packages(true); await p['pkg.upgrade.run']({});\n" +
      "for (let i = 0; i < 150; i++){ const j = await p['pkg.job'](); if (!j.running){ console.log(JSON.stringify(j)); break; } await new Promise(r => setTimeout(r, 100)); }\n");
    const прогон = async провал => {
      const журнал = join(папка, 'log' + (провал ? '-провал' : ''));
      const итог = await new Promise(готово => {
        let вывод = '';
        const д = spawn(process.execPath, [join(папка, 't.mjs')], { env:{ ...process.env,
          PATH:бин + ':' + process.env.PATH, APTLOG:журнал, ...(провал ? { WINE_FAIL:'1' } : {}) } });
        д.stdout.on('data', x => вывод += x);
        д.on('close', () => { try { готово(JSON.parse(вывод)); } catch(e){ готово({ вывод }); } });
      });
      return { итог, журнал:await читай(журнал, 'utf8').catch(() => '') };
    };
    const хорошо = await прогон(false);
    check('обновление системы следом ставит Wine с 32-битной половиной',
      хорошо.итог.ok && хорошо.итог.name === 'wine'
        && /add-architecture i386/.test(хорошо.журнал)
        && /install -y --no-install-recommends wine wine64 wine32:i386/.test(хорошо.журнал)
        && хорошо.журнал.indexOf('upgrade') < хорошо.журнал.indexOf('install'),
      JSON.stringify(хорошо.итог).slice(0, 200));
    const плохо = await прогон(true);
    check('не вышло с Wine — обновление всё равно «готово», причина в заметке',
      плохо.итог.ok === true && !плохо.итог.error && /wine32/.test(плохо.итог['заметка'] || ''),
      JSON.stringify(плохо.итог).slice(0, 200));
  }

  /* --- удаление программ по ярлыку ---

     Wine подставной: «uninstaller --list» читает список из файла, а
     «--remove ключ» вычёркивает строку — как настоящий деинсталлятор,
     который отработал. Проверяем то, что делаем мы: кто хозяин ярлыка,
     что удаляется и что пропадает из меню и со стола. */
  {
    const { mkdir, writeFile, chmod, readFile: читай } = await import('node:fs/promises');
    const дом = await mkdtemp(join(tmpdir(), 'glower-удал-'));
    const бин = join(дом, 'bin');
    await mkdir(бин, { recursive:true });
    await writeFile(join(бин, 'wine'), [
      '#!/bin/sh',
      'L="$HOME/wine-list"',
      'if [ "$1" = uninstaller ] && [ "$2" = --list ]; then cat "$L" 2>/dev/null; exit 0; fi',
      'if [ "$1" = uninstaller ] && [ "$2" = --remove ]; then grep -v "^$3|||" "$L" > "$L.n"; mv "$L.n" "$L"; exit 0; fi',
      'exit 0', ''].join('\n'));
    await writeFile(join(бин, 'wineserver'), '#!/bin/sh\nexit 0\n');
    await chmod(join(бин, 'wine'), 0o755); await chmod(join(бин, 'wineserver'), 0o755);
    await mkdir(join(дом, '.wine/drive_c'), { recursive:true });
    await writeFile(join(дом, 'wine-list'), 'Steam|||Steam\n{A1}|||Другое\n');
    const прил = join(дом, '.local/share/applications');
    const ярлык = (путь, имя, exec) => mkdir(dirname(путь), { recursive:true })
      .then(() => writeFile(путь, ['[Desktop Entry]', 'Type=Application', 'Name=' + имя, 'Exec=' + exec, ''].join('\n')));
    const винEx = 'env WINEPREFIX="' + дом + '/.wine" wine C:\\\\\\\\x.lnk';
    await ярлык(join(прил, 'wine/Programs/Steam/Steam.desktop'), 'Steam', винEx);
    await ярлык(join(прил, 'wine/Programs/Steam/Uninstall Steam.desktop'), 'Uninstall Steam', винEx);
    await ярлык(join(дом, 'Desktop/Steam.desktop'), 'Steam', винEx);
    await ярлык(join(прил, 'wine/Programs/Портативная.desktop'), 'Портативная', винEx);
    await mkdir(join(дом, 'Apps'), { recursive:true });
    await writeFile(join(дом, 'Apps/Прога.AppImage'), 'x');
    await ярлык(join(прил, 'appimagekit-prog.desktop'), 'Прога', join(дом, 'Apps/Прога.AppImage') + ' %U');
    await ярлык(join(прил, 'мой-ярлык.desktop'), 'Мой ярлык', 'true');

    const порт5 = PORT + 303;
    const аг = spawn(process.execPath,
      [join(root, 'agent/server.mjs'), '--port', String(порт5), '--root', дом, '--system', '--allow-packages'],
      { stdio:'ignore', env:{ ...process.env, HOME:дом, PATH:бин + ':' + process.env.PATH, GLOWER_NO_AUTO_WINE:'1' } });
    await new Promise(r => setTimeout(r, 1500));
    const зов = async (method, params = {}) => {
      const r = await fetch(`http://localhost:${порт5}/rpc`, { method:'POST',
        headers:{ 'Content-Type':'application/json' }, body:JSON.stringify({ method, params }) });
      return r.json();
    };
    const итог = {};
    try {
      итог.хозяинСтим = (await зов('pkg.owner', { id:'wine-Programs-Steam-Steam.desktop' })).result;
      итог.удалениеСтим = (await зов('pkg.uninstall', { id:'wine-Programs-Steam-Steam.desktop' })).result;
      for (let i = 0; i < 20; i++){
        итог.ходСтим = (await зов('pkg.uninstall.windows', { id:'wine-Programs-Steam-Steam.desktop' })).result;
        if (!итог.ходСтим['идёт']) break;
        await new Promise(r => setTimeout(r, 500));
      }
      итог.портативная = (await зов('pkg.uninstall', { id:'wine-Programs-Портативная.desktop' })).result;
      итог.образ = (await зов('pkg.owner', { id:'appimagekit-prog.desktop' })).result;
      итог.образУд = (await зов('pkg.uninstall', { id:'appimagekit-prog.desktop' })).result;
      итог.ярлык = (await зов('pkg.uninstall', { id:'мой-ярлык.desktop' })).result;
      итог.подлог = await зов('pkg.uninstall', { id:'../../etc/passwd.desktop' });
      итог.список = ((await зов('sys.apps')).result.list || []).map(a => a.name);
      итог.сброс = (await зов('pkg.windows.reset')).result;
    } catch(e){ итог.беда = e.message; }
    аг.kill();

    check('ярлык Wine узнаётся как программа Windows с записью деинсталлятора',
      итог.хозяинСтим && итог.хозяинСтим['вид'] === 'windows' && итог.хозяинСтим['ключ'] === 'Steam',
      JSON.stringify(итог.хозяинСтим));
    check('программа Windows удаляется своим деинсталлятором, и её ярлыки уходят отовсюду',
      итог.ходСтим && итог.ходСтим['удалено']
        && !existsSync(join(прил, 'wine/Programs/Steam'))
        && !existsSync(join(дом, 'Desktop/Steam.desktop'))
        && !/Steam\|\|\|/.test(await читай(join(дом, 'wine-list'), 'utf8').catch(() => ''))
        && existsSync(join(дом, 'wine-list')),
      JSON.stringify({ ход:итог.ходСтим }));
    check('программа Windows без записи — убирается ярлык, и об этом сказано',
      итог.портативная && итог.портативная['удалено'] && итог.портативная['толькоЯрлык']
        && !existsSync(join(прил, 'wine/Programs/Портативная.desktop')), JSON.stringify(итог.портативная));
    check('AppImage удаляется вместе со своим файлом',
      итог.образ && итог.образ['вид'] === 'appimage' && итог.образУд && итог.образУд['удалено']
        && !existsSync(join(дом, 'Apps/Прога.AppImage')) && !existsSync(join(прил, 'appimagekit-prog.desktop')),
      JSON.stringify({ о:итог.образ, у:итог.образУд }));
    check('ярлык без пакета удаляется сам', итог.ярлык && итог.ярлык['удалено']
      && !existsSync(join(прил, 'мой-ярлык.desktop')), JSON.stringify(итог.ярлык));
    check('подложный идентификатор программы отвергается',
      итог.подлог && итог.подлог.ok === false, JSON.stringify(итог.подлог));
    check('удалённых программ нет в списке меню',
      Array.isArray(итог.список) && !итог.список.some(н => /^(Steam|Прога|Мой ярлык|Портативная)$/.test(н)),
      JSON.stringify(итог.список));
    check('«удалить все программы Windows» убирает папку Windows и ярлыки Wine',
      итог.сброс && итог.сброс['удалено'] && !existsSync(join(дом, '.wine'))
        && !existsSync(join(прил, 'wine')), JSON.stringify(итог.сброс));
  }

  /* --- комплектующие: «О системе» спрашивает машину, а не браузер ---
     Раньше там было «8 логических ядер» и всё: это знал браузер. Теперь —
     модель процессора, ядра и потоки, видеокарты, диски, плата. */
  {
    const r = await page.evaluate(async () => {
      const w = WM.open('settings', { section:'about' });
      await new Promise(r2 => setTimeout(r2, 2500));
      const т = w.body.innerText;
      WM.close(w);
      return т;
    });
    const { readFileSync } = await import('node:fs');
    const модель = ((readFileSync('/proc/cpuinfo', 'utf8').match(/^model name\s*:\s*(.+)$/m) || [])[1] || '')
      .replace(/\((R|TM|tm)\)/g, '').replace(/\s+CPU\s+@.*$/, '').replace(/\s{2,}/g, ' ').trim().slice(0, 12);
    check('«О системе» показывает модель процессора, потоки и память машины',
      (!модель || r.includes(модель)) && /поток/.test(r) && /Оперативная память/.test(r) && !/логических ядер/.test(r),
      r.slice(0, 300));
    const { разбери_lspci } = await import(pathToFileURL(join(root, 'agent/system.mjs')).href);
    const у = разбери_lspci([
      '00:02.0 VGA compatible controller [0300]: Intel Corporation HD Graphics 520 [8086:1916] (rev 07)',
      '\tKernel driver in use: i915',
      '01:00.0 3D controller [0302]: NVIDIA Corporation GM108M [GeForce 940MX] [10de:134d] (rev a2)',
      '02:00.0 Network controller [0280]: Intel Corporation Wireless 8260 [8086:24f3] (rev 3a)',
      '\tKernel driver in use: iwlwifi'].join('\n'));
    check('разбор lspci: видеокарты, сеть и драйверы',
      у.length === 3 && у[0]['имя'] === 'Intel Corporation HD Graphics 520' && у[0]['драйвер'] === 'i915'
        && у[1]['класс'] === '3D controller' && у[1]['драйвер'] === null && у[2]['драйвер'] === 'iwlwifi',
      JSON.stringify(у));
  }

  /* --- раскладки: список из системы и выбор, который доходит до неё ---

     Агенту даём свой дом и свою папку сеанса: он пишет выбор в
     ~/.config/glower/keyboard и в environment личной папки labwc. Сценарий
     входа читает тот же файл — проверяем и его кусок: испорченная строка не
     должна оставить человека без клавиатуры. */
  {
    const { mkdir, writeFile, readFile: читай } = await import('node:fs/promises');
    const дом = await mkdtemp(join(tmpdir(), 'glower-kb-'));
    const рт = join(дом, 'rt');
    await mkdir(join(рт, 'glower-labwc'), { recursive:true });
    await writeFile(join(рт, 'glower-labwc', 'environment'), 'XKB_DEFAULT_LAYOUT=us,ru\nXKB_DEFAULT_OPTIONS=grp:alt_shift_toggle\n');
    const порт6 = PORT + 304;
    const аг = spawn(process.execPath, [join(root, 'agent/server.mjs'), '--port', String(порт6), '--root', дом, '--system'],
      { stdio:'ignore', env:{ ...process.env, HOME:дом, XDG_RUNTIME_DIR:рт, GLOWER_NO_AUTO_WINE:'1' } });
    await new Promise(r => setTimeout(r, 1500));
    const зов = async (method, params = {}) => (await fetch(`http://localhost:${порт6}/rpc`, { method:'POST',
      headers:{ 'Content-Type':'application/json' }, body:JSON.stringify({ method, params }) })).json();
    const и = {};
    try {
      и.список = (await зов('sys.kb.list')).result;
      и.пять = await зов('sys.kb.set', { 'раскладки':['us', 'ru', 'de', 'fr', 'it'] });
      и.плохое = await зов('sys.kb.set', { 'раскладки':['us;reboot'] });
      и.нет = await зов('sys.kb.set', { 'раскладки':['us', 'zzzz'] });
      и.ок = (await зов('sys.kb.set', { 'раскладки':['us', 'ru', 'ua'] })).result;
      и.файл = (await читай(join(дом, '.config/glower/keyboard'), 'utf8')).trim();
      и.env = await читай(join(рт, 'glower-labwc', 'environment'), 'utf8');
      и.снова = (await зов('sys.kb.list')).result['выбрано'];
    } catch(e){ и.беда = e.message; }
    аг.kill();
    const xkb = и.список && и.список['список'] || [];
    const есть = !!xkb.length;
    check('раскладки берутся из базы XKB системы',
      !есть || (xkb.length > 50 && xkb.some(x => x.id === 'ru') && xkb.some(x => x.id === 'ua')),
      JSON.stringify((xkb || []).slice(0, 3)));
    check('больше четырёх раскладок, чужие знаки и несуществующие — отвергаются',
      и.пять && !и.пять.ok && и.плохое && !и.плохое.ok && (!есть || (и.нет && !и.нет.ok)),
      JSON.stringify({ пять:и.пять, плохое:и.плохое, нет:и.нет }));
    check('выбор раскладок записан для входа и в настройки сеанса labwc',
      и.файл === 'us,ru,ua' && /^XKB_DEFAULT_LAYOUT=us,ru,ua$/m.test(и.env || '')
        && /XKB_DEFAULT_OPTIONS=grp:alt_shift_toggle/.test(и.env || '')
        && (и.env.match(/XKB_DEFAULT_LAYOUT=/g) || []).length === 1
        && JSON.stringify(и.снова) === JSON.stringify(['us', 'ru', 'ua']),
      JSON.stringify({ файл:и.файл, env:и.env, снова:и.снова, беда:и.беда }));

    /* Кусок сценария входа: чтение файла раскладок */
    const сеанс = await readFile(join(root, 'linux', 'glower-session'), 'utf8');
    const кусок = (сеанс.match(/^KB_USER=""[\s\S]*?^export XKB_DEFAULT_LAYOUT XKB_DEFAULT_OPTIONS$/m) || [''])[0];
    const раскладкаПри = async строка => {
      await writeFile(join(дом, '.config/glower/keyboard'), строка + '\n');
      return new Promise(готово => {
        let вывод = '';
        const д = spawn('sh', ['-c', кусок + '\nprintf %s "$XKB_DEFAULT_LAYOUT"'],
          { env:{ PATH:process.env.PATH, HOME:дом } });
        д.stdout.on('data', x => вывод += x);
        д.on('close', () => готово(вывод));
      });
    };
    const р = {
      хорошая:await раскладкаПри('us,ru,de'),
      знаки:await раскладкаПри('us;reboot'),
      пять:await раскладкаПри('us,ru,de,fr,it'),
      пусто:await раскладкаПри('')
    };
    check('вход берёт раскладки из файла, а испорченный файл — заменяет привычными',
      кусок && р.хорошая === 'us,ru,de' && р.знаки === 'us,ru' && р.пять === 'us,ru' && р.пусто === 'us,ru',
      JSON.stringify(р));

    /* Кусок сценария входа: личная папка labwc */
    const папка = (сеанс.match(/^  LABWC_SYS=[\s\S]*?^  fi$/m) || [''])[0];
    const сис = join(дом, 'sys');
    await mkdir(сис, { recursive:true });
    await writeFile(join(сис, 'rc.xml'), '<labwc_config/>');
    const рт2 = join(дом, 'rt2');
    await mkdir(рт2, { recursive:true });
    const куда = await new Promise(готово => {
      let вывод = '';
      const д = spawn('sh', ['-c', папка + '\nprintf %s "$LABWC_DIR"'], { env:{ PATH:process.env.PATH, HOME:дом,
        XDG_RUNTIME_DIR:рт2, GLOWER_LABWC:сис, XKB_DEFAULT_LAYOUT:'us,de', XKB_DEFAULT_OPTIONS:'grp:alt_shift_toggle' } });
      д.stdout.on('data', x => вывод += x);
      д.on('close', () => готово(вывод));
    });
    const { lstatSync } = await import('node:fs');
    const env2 = await читай(join(рт2, 'glower-labwc', 'environment'), 'utf8').catch(() => '');
    check('сеанс собирает labwc личную папку: системные файлы ссылками, раскладки своим файлом',
      куда === join(рт2, 'glower-labwc') && lstatSync(join(куда, 'rc.xml')).isSymbolicLink()
        && /XKB_DEFAULT_LAYOUT=us,de/.test(env2) && !lstatSync(join(куда, 'environment')).isSymbolicLink(),
      JSON.stringify({ куда, env2 }));
  }

  /* --- язык программ Linux: локаль для входа ---
     Агент пишет её в ~/.config/glower/locale; сценарий входа выставляет
     LANG, только если локаль в системе собрана (locale здесь подставной). */
  {
    const { mkdir, writeFile, chmod, readFile: читай } = await import('node:fs/promises');
    const дом = await mkdtemp(join(tmpdir(), 'glower-лок-'));
    const порт7 = PORT + 305;
    const аг = spawn(process.execPath, [join(root, 'agent/server.mjs'), '--port', String(порт7), '--root', дом, '--system'],
      { stdio:'ignore', env:{ ...process.env, HOME:дом, GLOWER_NO_AUTO_WINE:'1' } });
    await new Promise(r => setTimeout(r, 1500));
    const зов = async (method, params = {}) => (await fetch(`http://localhost:${порт7}/rpc`, { method:'POST',
      headers:{ 'Content-Type':'application/json' }, body:JSON.stringify({ method, params }) })).json();
    const и = {};
    try {
      и.ук = (await зов('sys.locale.set', { 'язык':'uk' })).result;
      и.файл = (await читай(join(дом, '.config/glower/locale'), 'utf8')).trim();
      и.чужой = await зов('sys.locale.set', { 'язык':'xx;reboot' });
    } catch(e){ и.беда = e.message; }
    аг.kill();
    check('язык программ записан локалью, чужой язык отвергнут',
      и.ук && и.ук['локаль'] === 'uk_UA.UTF-8' && и.ук['пакет'] === 'language-pack-uk'
        && и.файл === 'uk_UA.UTF-8' && и.чужой && !и.чужой.ok, JSON.stringify(и));

    const сеанс = await readFile(join(root, 'linux', 'glower-session'), 'utf8');
    const кусок = (сеанс.match(/^if \[ -r "\$HOME\/\.config\/glower\/locale" \]; then[\s\S]*?^fi$/m) || [''])[0];
    const бин = join(дом, 'bin');
    await mkdir(бин, { recursive:true });
    const языкПри = async (локали, файл) => {
      await writeFile(join(бин, 'locale'), '#!/bin/sh\nprintf "%s\\n" ' + локали.map(л => `"${л}"`).join(' ') + '\n');
      await chmod(join(бин, 'locale'), 0o755);
      await writeFile(join(дом, '.config/glower/locale'), файл + '\n');
      return new Promise(готово => {
        let вывод = '';
        const д = spawn('sh', ['-c', 'LANG=C.UTF-8\n' + кусок + '\nprintf "%s|%s" "$LANG" "${LANGUAGE:-}"'],
          { env:{ PATH:бин + ':' + process.env.PATH, HOME:дом } });
        д.stdout.on('data', x => вывод += x);
        д.on('close', () => готово(вывод));
      });
    };
    const р = {
      есть:await языкПри(['C.utf8', 'uk_UA.utf8'], 'uk_UA.UTF-8'),
      нет:await языкПри(['C.utf8'], 'uk_UA.UTF-8'),
      мусор:await языкПри(['C.utf8', 'uk_UA.utf8'], 'uk_UA.UTF-8; reboot')
    };
    check('вход выставляет программам только собранную локаль',
      кусок && р.есть === 'uk_UA.UTF-8|uk_UA' && р.нет === 'C.UTF-8|' && р.мусор === 'C.UTF-8|', JSON.stringify(р));
    const фикс = await readFile(join(root, 'linux', 'glower-fix'), 'utf8');
    check('glower-fix собирает только локали из закрытого списка',
      /язык\)/.test(фикс) && /uk_UA\.UTF-8\|/.test(фикс) && /неизвестная локаль/.test(фикс));
  }

  /* Начальная настройка: раскладки списком из каталога, не больше четырёх */
  {
    const r = await page.evaluate(async () => {
      const был = Setup.i;
      Setup.data.layouts = ['en', 'ru'];
      Setup.i = Setup.STEPS.findIndex(s => /Раскладки/.test(s.title));
      const откр = !document.getElementById('setup');
      if (откр) Setup.open && Setup.open();
      Setup.i = Setup.STEPS.findIndex(s => /Раскладки/.test(s.title));
      Setup.paint();
      await new Promise(r2 => setTimeout(r2, 300));
      const строки = [...document.querySelectorAll('#setup .kb-строка')];
      const нажми = имя => строки.find(с => с.textContent.includes(имя)).click();
      нажми('Украинская'); нажми('Немецкая'); нажми('Французская');
      const итог = { строк:строки.length, выбрано:Setup.data.layouts.slice(),
        заблок:document.querySelectorAll('#setup .kb-строка:disabled').length };
      Setup.i = был;
      const у = document.getElementById('setup'); if (у && откр) у.remove();
      return итог;
    });
    check('в настройке раскладки — списком, и больше четырёх выбрать нельзя',
      r.строк >= 50 && JSON.stringify(r.выбрано) === JSON.stringify(['en', 'ru', 'ua', 'de']) && r.заблок > 40,
      JSON.stringify(r));
  }

  /* --- ход установки: плашка справа внизу, а Магазин не трясётся ---

     Подставной apt «ставит» Wine четыре секунды и сообщает проценты.
     Прежде Магазин раз в секунду перерисовывал страницу целиком — она
     дёргалась всё время установки. Теперь ход показывает плашка, и она же
     — один и тот же элемент, у которого меняются только текст и полоса. */
  {
    const { mkdir, writeFile, chmod } = await import('node:fs/promises');
    const дом = await mkdtemp(join(tmpdir(), 'glower-плашка-'));
    const бин = join(дом, 'bin');
    await mkdir(бин, { recursive:true });
    const скрипт = (имя, тело) => writeFile(join(бин, имя), '#!/bin/sh\n' + тело).then(() => chmod(join(бин, имя), 0o755));
    await скрипт('sudo', '[ "$1" = "-n" ] && shift\nexec "$@"\n');
    await скрипт('dpkg', 'exit 0\n');
    await скрипт('dpkg-query', 'exit 1\n');
    await скрипт('apt-get', 'case "$*" in *install*) for p in 10 30 55 80; do echo "pmstatus:wine:$p:Ставлю" >&3; sleep 1; done ;; esac\nexit 0\n');
    const порт8 = PORT + 306;
    const аг = spawn(process.execPath, [join(root, 'agent/server.mjs'), '--port', String(порт8), '--root', дом,
      '--system', '--allow-packages'],
      { stdio:'ignore', env:{ ...process.env, HOME:дом, PATH:бин + ':' + process.env.PATH, GLOWER_NO_AUTO_WINE:'1' } });
    await new Promise(r => setTimeout(r, 1500));
    const стр = await browser.newPage({ viewport:{ width:1280, height:800 } });
    await стр.addInitScript(() => { localStorage.setItem('glower.setup.done', 'true'); localStorage.setItem('glower.perf.userChoice', 'true'); });
    let и = {};
    try {
      await стр.goto(`http://localhost:${порт8}/`);
      await стр.waitForTimeout(3500);
      и = await стр.evaluate(async () => {
        document.querySelectorAll('.welcome, .вход').forEach(н => н.remove());
        /* Сначала работа, потом Магазин: открытый посреди работы, он
           подхватывает её сам — прежде именно так он и начинал трястись */
        await Platform.rpc('pkg.windows.install');
        Работы.следи('wine', 'Поддержка программ Windows', '🪟');
        WM.open('store');
        await new Promise(r => setTimeout(r, 1800));
        const плашка = document.querySelector('.работа');
        const ст = document.querySelector('.st-body');
        const первая = ст && ст.firstElementChild;
        const было = { есть:!!плашка, текст:плашка ? плашка.textContent : '', вСтолбике:!!(плашка && плашка.closest('#toasts')) };
        const место = плашка ? плашка.getBoundingClientRect() : null;
        await new Promise(r => setTimeout(r, 2000));
        const та = document.querySelector('.работа') === плашка;
        const страницаНаМесте = !!первая && ст.firstElementChild === первая;
        const проценты = (плашка.querySelector('.работа-шаг') || {}).textContent;
        await new Promise(r => setTimeout(r, 3500));
        return { ...было, та, страницаНаМесте, проценты,
          справаВнизу:!!место && место.right > innerWidth - 60 && место.bottom > innerHeight * 0.6,
          итог:(document.querySelector('.работа-шаг') || {}).textContent,
          готово:!!document.querySelector('.работа.готово') };
      });
    } catch(e){ и.беда = e.message; }
    await стр.close();
    аг.kill();
    check('ход установки — плашка справа внизу с названием и процентами',
      и.есть && и.вСтолбике && /Поддержка программ Windows/.test(и.текст) && /%/.test(и.проценты || '') && и.справаВнизу,
      JSON.stringify(и));
    check('плашка не пересоздаётся, а страница Магазина не перерисовывается во время установки',
      и.та && и.страницаНаМесте, JSON.stringify(и));
    check('в конце плашка говорит «Установлено»', и.готово && /Установлено/.test(и.итог || ''), JSON.stringify(и));
  }

  /* В меню программ по правой кнопке есть «Удалить» — у программ машины. */
  {
    const r = await page.evaluate(() => {
      Shell.менюПрограммы({ clientX:200, clientY:200, preventDefault(){} },
        { id:'проба.desktop', 'вид':'машина', 'имя':'Проба' });
      const текст = document.getElementById('ctx').textContent;
      Shell.менюПрограммы({ clientX:200, clientY:200, preventDefault(){} },
        { id:'notepad', 'вид':'приложение', 'имя':'Блокнот' });
      const текст2 = document.getElementById('ctx').textContent;
      document.getElementById('ctx').classList.remove('on');
      return { текст, текст2 };
    });
    check('в меню программ у программы машины есть «Удалить», у частей системы — нет',
      /Удалить/.test(r.текст) && !/Удалить/.test(r.текст2), JSON.stringify(r));
  }

  /* --- .exe в Проводнике уходит системе, а не в Блокнот ---
     Здесь агенту запуск не разрешён, и это кстати: его отказ доказывает,
     что файл дошёл до системного слоя, а не открылся как текст. */
  {
    const r = await page.evaluate(async () => {
      await Platform.rpc('fs.write', { path:'проба.exe', body:'MZ' });
      const окон = WM.wins.filter(w => w.appId === 'notepad').length;
      Assoc.open({ name:'проба.exe', type:'file' }, []);
      await new Promise(r2 => setTimeout(r2, 900));
      const текст = document.body.innerText;
      const итог = { блокнот:WM.wins.filter(w => w.appId === 'notepad').length > окон,
                     отказ:/запуск программ выключен/.test(текст) };
      document.dispatchEvent(new KeyboardEvent('keydown', { key:'Escape', bubbles:true }));
      await new Promise(r2 => setTimeout(r2, 300));
      const w = WM.open('settings', { section:'apps' });
      await new Promise(r2 => setTimeout(r2, 1500));
      const карта = w.node.querySelector('.win-wine');
      итог.карта = карта ? карта.innerText.replace(/\s+/g, ' ').slice(0, 160) : null;
      WM.close(w);
      return итог;
    });
    check('.exe в Проводнике уходит системе, а не открывается Блокнотом',
      !r.блокнот && r.отказ, JSON.stringify(r));
    check('в Параметрах → Приложения есть «Программы Windows» с состоянием Wine',
      r.карта && /Wine/.test(r.карта) && !/Проверяю/.test(r.карта), JSON.stringify(r));
  }

  /* Куда уходит WM.open, когда на машине есть родные программы */
  {
    const r = await page.evaluate(async () => {
      const было = { rpc: Platform.rpc, open: WM.open, нат: OS.нативные };
      const вызовы = [];
      let нетФайла = false;
      Platform.rpc = async (м, п) => {
        вызовы.push([м, п]);
        if (м === 'sys.native.list') return { list:['notepad', 'calc', 'files', 'settings', 'store'] };
        if (м === 'fs.stat' && нетФайла) throw new Error('нет');
        if (м === 'sys.windows') return { list:[{ appId:'glower-calc', title:'Калькулятор' }] };
        return {};
      };
      try {
        await wireNativeSystemApps();
        const ждём = () => new Promise(r2 => setTimeout(r2, 60));
        const итог = {};
        итог.файл = WM.open('notepad', { file:{ path:['Документы'], name:'а.txt' } });
        await ждём();
        итог.путь = (вызовы.find(в => в[0] === 'sys.native') || [])[1];
        вызовы.length = 0;
        WM.open('files', { path:['Документы', 'Отчёты'] });
        await ждём();
        итог.папка = (вызовы.filter(в => в[0] === 'sys.native').pop() || [])[1];
        вызовы.length = 0;
        WM.open('settings', { section:'net' });
        await ждём();
        итог.параметры = (вызовы.filter(в => в[0] === 'sys.native').pop() || [])[1];
        вызовы.length = 0;
        WM.open('store', { 'вкладка':'обновления' });
        await ждём();
        итог.магазин = (вызовы.filter(в => в[0] === 'sys.native').pop() || [])[1];
        WM.open('trash');
        await ждём();
        итог.корзина = (вызовы.filter(в => в[0] === 'sys.native').pop() || [])[1];
        вызовы.length = 0;
        итог.сеанс = WM.open('notepad', { x:1, y:1, w:300, h:200, 'восстановление':true });
        await ждём();
        итог.сеансЗвал = вызовы.length;
        WM.open('calc');
        await ждём();
        итог.калк = вызовы.map(в => в[0] + (в[1] && в[1].action ? ':' + в[1].action : ''));
        вызовы.length = 0;
        нетФайла = true;
        WM.open('notepad', { file:{ path:['Документы'], name:'нет.txt', content:'x' } });
        await new Promise(r2 => setTimeout(r2, 300));
        const своё = WM.wins.find(w => w.appId === 'notepad');
        итог.запасной = !!своё;
        if (своё) WM.close(своё);
        return итог;
      } finally {
        Platform.rpc = было.rpc; WM.open = было.open; OS.нативные = было.нат;
      }
    });
    check('Блокнот открывает файл нативным окном, с путём файла',
      r.файл === null && r.путь && r.путь['программа'] === 'notepad' && JSON.stringify(r.путь['путь']) === JSON.stringify(['Документы', 'а.txt']),
      JSON.stringify(r));
    check('папка открывается нативным Проводником, с путём папки',
      r.папка && r.папка['программа'] === 'files' && JSON.stringify(r.папка['путь']) === JSON.stringify(['Документы', 'Отчёты']),
      JSON.stringify(r));
    check('Параметры открываются нативными, сразу на нужном разделе',
      r.параметры && r.параметры['программа'] === 'settings' && r.параметры['раздел'] === 'net', JSON.stringify(r));
    check('Магазин открывается нативным, сразу на нужной вкладке',
      r.магазин && r.магазин['программа'] === 'store' && r.магазин['раздел'] === 'updates', JSON.stringify(r));
    check('корзину на машине показывает Проводник',
      r.корзина && r.корзина['программа'] === 'files' && r.корзина['раздел'] === 'trash', JSON.stringify(r));
    check('восстановление сеанса нативных не запускает', r.сеанс === null && r.сеансЗвал === 0, JSON.stringify(r));
    check('Калькулятор один: открыт — переходим к нему', r.калк.includes('sys.window:focus') && !r.калк.includes('sys.native'), JSON.stringify(r));
    check('не вышло нативно — открывается своё окно, как раньше', r.запасной, JSON.stringify(r));
  }

  /* Удалённое со стола — в системную корзину, а не в память страницы */
  {
    const r = await page.evaluate(async () => {
      const было = (KV.get('trash', []) || []).length;
      FS.write(['Документы'], 'в-корзину.txt', 'мусор');
      await new Promise(r2 => setTimeout(r2, 400));
      FS.rm(['Документы'], 'в-корзину.txt');
      await new Promise(r2 => setTimeout(r2, 700));
      const сколько = (await Platform.rpc('fs.trash.count'))['сколько'];
      const вПамяти = (KV.get('trash', []) || []).length - было;
      Shell.делоПрограммы('очисти-корзину', {});
      await new Promise(r2 => setTimeout(r2, 500));
      const после = (await Platform.rpc('fs.trash.count'))['сколько'];
      return { сколько, вПамяти, после };
    });
    const { readdir: rd } = await import('node:fs/promises');
    check('удалённое со стола ложится в системную корзину и очищается оттуда',
      r.сколько === 1 && r.вПамяти === 0 && r.после === 0, JSON.stringify(r));
    check('сам файл с диска ушёл', !existsSync(join(WS, 'Документы', 'в-корзину.txt')), (await rd(join(WS, 'Документы')).catch(() => [])).join(','));
  }

  /* Мост нативных Параметров: страница рассказывает и слушается */
  {
    const r = await page.evaluate(async () => {
      const с = МостПараметров.состояние();
      const было = S.wctl;
      await МостПараметров.дело({ 'что':'set', k:'wctl', v:'mac' });
      const стало = S.wctl;
      await МостПараметров.дело({ 'что':'set', k:'wctl', v:было });
      const вДоке = S.dockApps.includes('calc');
      await МостПараметров.дело({ 'что':'список', k:'dockApps', id:'calc' });
      const переключилось = S.dockApps.includes('calc') !== вДоке;
      await МостПараметров.дело({ 'что':'список', k:'dockApps', id:'calc' });
      return { обоев:с['обои'].length, акцентов:с['акценты'].length, программ:с['программы'].length,
               раскладок:с['все_раскладки'].length, стало, переключилось, вернулось:S.dockApps.includes('calc') === вДоке };
    });
    check('страница рассказывает Параметрам обои, акценты, программы и раскладки',
      r.обоев > 10 && r.акцентов === 10 && r.программ > 10 && r.раскладок > 5, JSON.stringify(r));
    check('просьбы Параметров меняют настройки оболочки', r.стало === 'mac' && r.переключилось && r.вернулось, JSON.stringify(r));
  }

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

/* Провалы повторяем в самом конце — после итога.
   Список тестов печатается одной простыней выше, и стоит посмотреть вывод
   через tail, как имя упавшего теста срезается: остаётся голое «Провалено: 1»
   и никакой зацепки. Одно такое падение из десяти прогонов у нас уже
   пропало без следа. Повтор внизу стоит три строки и переживает любую
   обрезку. */
if (failed){
  console.log('  Что не прошло:');
  console.log(out.filter(с => с.includes('\u274c')).join('\n'));
  console.log('');
}

process.exit(failed ? 1 : 0);
