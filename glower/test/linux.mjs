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
          'состояние':{ 'развёрнуто':true, 'вовесь':false, 'активно':true } }], 'можно':true });
      if (m === 'sys.icon') return Promise.resolve({ 'есть':true, 'тип':'image/png',
        'данные':'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==' });
      return было(m, p);
    };
    await new Promise(r => setTimeout(r, 3600));
    const кнопка = [...document.querySelectorAll('#dock-running .dock-item')]
      .find(b => /Telegram/.test(b.dataset.tip || ''));
    const ответ = {
      картинка:!!(кнопка && кнопка.querySelector('img')),
      развёрнуто:!!(кнопка && кнопка.classList.contains('развёрнуто')),
      вработе:!!(кнопка && кнопка.classList.contains('active')),
      подпись:кнопка ? кнопка.dataset.tip : ''
    };
    Platform.rpc = было;
    return ответ;
  });
  check('у чужого окна в панели задач свой значок',
    значкиВПанели.картинка === true, JSON.stringify(значкиВПанели));
  check('панель задач видит, что чужое окно развёрнуто',
    значкиВПанели['развёрнуто'] === true && значкиВПанели.вработе === true,
    JSON.stringify(значкиВПанели));
  check('о развёрнутом окне сказано и словами',
    /развёрнуто/.test(значкиВПанели.подпись || ''), значкиВПанели.подпись);

  /* --- чужое окно во весь экран: панель уходит с дороги --- */
  const вовесь = await page.evaluate(async () => {
    const было = Platform.rpc.bind(Platform);
    const сказано = [];
    Platform.rpc = (m, p) => {
      if (m === 'sys.windows') return Promise.resolve({ list:[
        { appId:'mpv', title:'Кино', 'оболочка':false,
          'состояние':{ 'развёрнуто':false, 'вовесь':true, 'активно':true } }],
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
