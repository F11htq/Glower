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
/* Настоящий сеанс говорит агенту, каким движком рисовать страницы, — здесь
   делаем то же самое. Раньше этого не было, и проверки встроенного браузера
   падали не из-за системы, а из-за того, что в проверочной машине нет
   команды chromium в путях. */
const agent = spawn(process.execPath,
  [join(root, 'agent/server.mjs'), '--port', String(PORT), '--root', WS, '--system'],
  { stdio:['ignore', 'pipe', 'pipe'],
    env:{ ...process.env, GLOWER_BROWSER:process.env.GLOWER_BROWSER || exe || '' } });
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
    check('столы переключаются с клавиатуры', /glower-desk/.test(rc));

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
