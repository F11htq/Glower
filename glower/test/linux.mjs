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

let passed = 0, failed = 0;
const out = [];
const check = (n, c, d = '') => { c ? (passed++, out.push('  ✅ ' + n))
  : (failed++, out.push('  ❌ ' + n + (d ? ' — ' + d : ''))); };

if (process.platform !== 'linux'){
  console.log('\n  Эта проверка имеет смысл только на Linux — пропущена.\n');
  process.exit(0);
}

const WS = await mkdtemp(join(tmpdir(), 'glower-'));
/* Корзина — своя, проверочная: настоящую корзину машины не трогаем */
const КОРЗИНА = await mkdtemp(join(tmpdir(), 'glower-trash-'));
const agent = spawn(process.execPath,
  [join(root, 'agent/server.mjs'), '--port', String(PORT), '--root', WS, '--system'],
  { stdio:['ignore', 'pipe', 'pipe'],
    env:{ ...process.env, GLOWER_TRASH:КОРЗИНА } });
let agentLog = '';
agent.stdout.on('data', d => agentLog += d);
await new Promise(r => setTimeout(r, 1300));

/* Страницы рабочего стола (и браузерного движка под ней) больше нет: всё,
   что проверки спрашивали у агента через неё, спрашиваем напрямую. Тонкая
   прослойка с тем же видом, что был у страницы: page.evaluate(() =>
   Platform.rpc(...)) выполняется здесь, в Node, и ходит к агенту по HTTP. */
const rpc = async (method, params) => {
  const r = await fetch(`http://localhost:${PORT}/rpc`, {
    method:'POST', headers:{ 'Content-Type':'application/json' }, body:JSON.stringify({ method, params }) });
  const j = await r.json();
  if (!j.ok) throw new Error(j.error || 'ошибка агента');
  return j.result;
};
globalThis.Platform = { rpc, mode:'native' };
/* Те же вызовы, что были у страницы в js/os.js */
const OS = globalThis.OS = {
  caps:null,
  on(){ return !!this.caps; },
  procs:() => rpc('sys.procs'),
  apps:() => rpc('sys.apps'),
  launch:id => rpc('sys.launch', { id }),
  power:(action, сразу) => rpc('sys.power', { action, 'сразу':!!сразу }),
  volume:() => rpc('sys.volume.get'),
  brightness:() => rpc('sys.brightness.get'),
  wifiState:() => rpc('sys.wifi.state'),
};
const page = {
  evaluate: async (fn, arg) => fn(arg),
  waitForTimeout: ms => new Promise(r => setTimeout(r, ms)),
};

try {
  OS.caps = await rpc('sys.caps').catch(() => null);

  check('агент запустился и рассказал о машине', OS.on(), JSON.stringify(OS.caps).slice(0, 120));

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
  const insCan = await rpc('install.can').catch(e => ({ allowed:false, reason:e.message }));
  check('установка выключена, пока её не разрешили ключом',
    insCan.allowed === false && /--allow-install/.test(insCan.reason || ''), JSON.stringify(insCan));

  const insStart = await page.evaluate(() => Platform.rpc('install.start', { disk:'/dev/sda' })
    .then(() => 'запустилась', e => e.message));
  check('без --allow-install диск не трогают', /--allow-install/.test(insStart), insStart);

  const insDisks = await rpc('install.disks').then(d => d, e => ({ err:e.message }));
  check('список дисков читается из /sys',
    Array.isArray(insDisks.list) && insDisks.list.every(d => d.dev.startsWith('/dev/')),
    JSON.stringify(insDisks).slice(0, 200));

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

    const чужойЗапуск = await page.evaluate(() => Platform.rpc('sys.appimage', { 'путь':'/usr/bin/id' })
      .then(() => 'запустил', e => e.message));
    check('вместо программы одним файлом системную не запустить',
      /это не программа одним файлом|--allow-launch/.test(String(чужойЗапуск)), String(чужойЗапуск));

    await rm(join(WS, 'проба.deb'), { force:true });
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
    const панель = await readFile(join(root, 'panel', 'src', 'menus.rs'), 'utf8');
    const система = await readFile(join(root, 'agent', 'system.mjs'), 'utf8');
    /* «Сменить пользователя» панель делает сама — это выход на экран входа */
    const шлём = [...(панель.match(/const ДЕЙСТВИЯ[\s\S]*?\];/) || [''])[0].matchAll(/\("([a-z]+)", "/g)]
      .map(m => m[1]).filter(a => a !== 'switch');
    const знает = (система.match(/const map = \{[^}]*\}/s) || [''])[0];
    const чужие = шлём.filter(a => !new RegExp('\\b' + a + ':').test(знает));
    check('панель просит у агента только то, что он умеет',
      шлём.length >= 5 && чужие.length === 0, 'агент не знает: ' + чужие.join(', '));
  }

  {
    /* «Bluetooth не работает» — не ответ. Выключен рубильником, нет
       прошивки или адаптера нет вовсе — это три разные беды, и человеку
       делать надо разное. Система обязана сказать, какая из них. */
    const бт = await rpc('sys.bt.почему', {});
    check('система объясняет, почему нет Bluetooth',
      !!бт && typeof бт['словами'] === 'string' && бт['словами'].length > 10,
      JSON.stringify(бт).slice(0, 120));
  }

  /* Диспетчер задач: программы человека отдельно, внутренности системы
     отдельно. Человек шёл закрыть зависшую программу, а видел список, где
     она тонет среди WebKitWebProcess, systemd и наших же частей. */
  {
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
    await катал(join(корень, 'usr/share/glower/ui/agent'), { recursive:true });
    await пиши(join(корень, 'usr/share/glower/ui/agent/server.mjs'), '// агент\n');
    await пиши(join(корень, 'usr/bin/glower-session'), '#!/bin/sh\n');
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
      check('в пакете лежат сеанс и агент', /usr\/bin\/glower-session/.test(внутри)
        && /usr\/share\/glower\/ui\/agent\/server\.mjs/.test(внутри), внутри.slice(0, 80));
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
    // Маски ставит генератор по метке glower.fast: из строки ядра
    // пробный режим снять их не мог — systemd возвращал их при reload
    const генератор = await readFile(join(root, 'linux', 'glower-fast-generator'), 'utf8');
    const проба = await readFile(join(root, 'linux', 'glower-try'), 'utf8');
    check('в живой системе лишние службы выключены',
      /glower\.fast/.test(строка) && /NetworkManager\.service/.test(генератор)
      && /ufw\.service/.test(генератор) && /cups\.service/.test(генератор)
      && /system-generators\/glower-fast/.test(образ), строка.slice(0, 60));
    check('пробный режим снимает маски и поднимает сеть',
      /\/run\/glower-try/.test(генератор) && /touch \/run\/glower-try/.test(проба)
      && /daemon-reload/.test(проба) && /NetworkManager\.service/.test(проба));
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

    check('агент больше не раздаёт файлы по HTTP — страницы, которой они были нужны, нет',
      await fetch(`http://localhost:${PORT}/обои?p=` + encodeURIComponent('/etc/passwd')).then(r => r.status) === 404);

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
    /* Программы системы — это glower-panel под своим именем: в дереве
       сценария нет, есть ветка в main.rs и ссылка в образе */
    const главная0 = await readFile(join(root, 'panel', 'src', 'main.rs'), 'utf8');
    for (const прог of наши){
      const своя = главная0.includes('имя == "' + прог + '"');
      check('сочетания зовут ' + прог + ' — она есть в дереве',
        existsSync(join(root, 'linux', прог)) || своя);
      check('и она попадает в образ', своя ? new RegExp('ln -sf glower-panel "\\$ROOTFS/usr/bin/' + прог + '"|for prog in [^;]*' + прог).test(образ2)
        : образ2.includes('linux/' + прог));
      check('и в пакет обновления', пакет.includes('usr/bin/' + прог));
    }

    /* Окна системы живут внутри страницы стола, и оконный сервер видит их
       одним окном. Alt+F4, Alt+Tab и Win со стрелками на столе он должен
       отдавать странице, иначе до неё они не доходят вовсе: Alt+F4 на
       открытых Параметрах не делал ничего. */
    {
      const привязка = к => (rc.match(new RegExp('<keybind key="' + к + '">([\\s\\S]*?)</keybind>')) || [])[1] || '';
      /* Стол, панель и док — слои, а не окно: оконный сервер сам держит
         их ниже и выше окон. Сочетания — обычные действия labwc над
         обычными окнами, никакого «окна-оболочки» в правилах нет. */
      check('сочетания окон — действия оконного сервера',
        /<action name="Close" \/>/.test(привязка('A-F4')) && /NextWindow/.test(привязка('A-Tab'))
          && /SnapToEdge/.test(привязка('W-Left')) && /SnapToEdge/.test(привязка('W-Right'))
          && привязка('W-Tab').includes('glower-menu задачи') && привязка('W-d').includes('glower-menu клавиша W-d')
          && !/glowershell/.test(rc));
      {
        const ядро0 = await readFile(join(root, 'panel', 'src', 'core.rs'), 'utf8');
        check('панель переходит по столам служебными сочетаниями — они есть в rc.xml',
          [1, 2, 3, 4].every(n => new RegExp('<keybind key="W-C-A-F' + n + '"><action name="GoToDesktop" to="' + n + '" />').test(rc)
            && new RegExp('<keybind key="W-C-A-S-F' + n + '"><action name="SendToDesktop" to="' + n + '" follow="no" />').test(rc))
            && /"-M", "logo", "-M", "ctrl", "-M", "alt", "-k", &f/.test(ядро0) && /<desktops>/.test(rc));
      }
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
      const ядро1 = await readFile(join(root, 'panel', 'src', 'core.rs'), 'utf8');
      check('glower-menu передаёт сочетание панели, а она его слушает',
        /"\$\{1:-\}" = "клавиша"/.test(меню) && /"тема":"клавиша"/.test(меню)
          && /"клавиша"/.test(ядро1) && /покажи_стол/.test(ядро1) && /"\$\{1:-\}" = "задачи"/.test(меню));
    }

    /* Нативная панель (panel/, Rust). Она должна доехать и до образа, и до
       машин через пакет обновления, собираться на GitHub до образа — и не
       оставить человека без панели вовсе, если с ней что-то не так. */
    {
      const cargo = await readFile(join(root, 'panel', 'Cargo.toml'), 'utf8');
      const сборка = await readFile(join(root, '..', '.github', 'workflows', 'build-iso.yml'), 'utf8');
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
      {
        const сеанс2 = await readFile(join(root, 'linux', 'glower-session'), 'utf8');
        check('сеанс — labwc и панель, браузерного движка нет',
          /\/usr\/bin\/glower-panel/.test(сеанс2) && /falls -ge 5/.test(сеанс2)
            && !/glower-shell|webkit|WebKit2|chromium|BROWSER|cage/.test(сеанс2)
            && /WLR_BACKENDS=x11 WLR_RENDERER=pixman/.test(сеанс2)
            && !/gir1\.2-webkit2|python3-gi|chromium|cage /.test(образ2.split('\n').filter(с => !/^\s*#/.test(с)).join('\n'))
            && !/webkit/.test(пакет)
            && !existsSync(join(root, 'linux', 'glower-shell')));
      }
      /* Нативная говорит на той же шине, что страничная: стол ждёт от
         панели «готова» и её рост, и слушает она те же темы. */
      const главная = await readFile(join(root, 'panel', 'src', 'main.rs'), 'utf8');
      const темы = ['раскладка', 'часы', 'батарея', 'обновления', 'настройки', 'стол-занят', 'затемни', 'открой-пуск'];
      const меню3 = await readFile(join(root, 'linux', 'glower-menu'), 'utf8');
      const обзор = await readFile(join(root, 'panel', 'src', 'overview.rs'), 'utf8');
      check('меню программ рисует панель',
        /"программы" => overview::программы/.test(главная)
          && /"обзор-данные" => overview::данные/.test(главная)
          && /скажи\("обзор-дай"/.test(обзор) && /скажи\("обзор-дело"/.test(обзор)
          && /DesktopAppInfo::new/.test(обзор)
          && /drag_source_set/.test(обзор) && /drag_dest_set/.test(обзор)
          /* Как Launchpad: поверх всего, вместе с панелью, и с настоящим
             затемнением — самоотрисовываемое окно GTK само фон не льёт */
          && /set_layer\(Layer::Overlay\)/.test(обзор) && /set_exclusive_zone\(-1\)/.test(обзор)
          && /set_source_rgba\(0\.04, 0\.047, 0\.08, плотность \* ПОКАЗ/.test(обзор) && /\{ 0\.55 \} else \{ 0\.86 \}/.test(обзор)
          /* И появляется плавно, а не возникает разом */
          && /add_tick_callback/.test(обзор)
          && /"на-стол"/.test(обзор) && /"окно-на-стол"/.test(обзор) && /"окно-на-новый-стол"/.test(обзор));
      const док_ = await readFile(join(root, 'panel', 'src', 'dock.rs'), 'utf8');
      /* Свои программы док узнаёт по списку ядра: id и имя окна в одной строке */
      const наши_ = await readFile(join(root, 'panel', 'src', 'core.rs'), 'utf8');
      const родная = (окно, id) => new RegExp('id: "' + id + '",.*окно: "' + окно + '"').test(наши_);
      check('нативный док: поверх окон, прячется при окнах и выезжает у края',
        /mod dock;/.test(главная) && /dock::создай\(\)/.test(главная) && /"док" => dock::данные/.test(главная)
          && /set_layer\(Layer::Top\)/.test(док_) && /glower-dock-edge/.test(док_)
          && /"sys\.windows"/.test(док_) && /меню_открыто/.test(док_));
      const стол_ = await readFile(join(root, 'panel', 'src', 'desktop.rs'), 'utf8');
      check('нативный стол: нижний слой, обои и значки',
        /mod desktop;/.test(главная) && /desktop::создай\(\)/.test(главная)
          && /"значки" => desktop::значки/.test(главная) && /"обои" => desktop::обои/.test(главная)
          && /set_layer\(Layer::Background\)/.test(стол_)
          && /fn меню_стола/.test(стол_) && /fn меню_значков/.test(стол_) && /fn обнови_виджеты/.test(стол_));
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
        check('при включении — тот же экран, упал — поднимается снова',
          /\/usr\/bin\/glower-login/.test(вход) && /falls -ge 5/.test(вход) && !/glower-shell/.test(вход)
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
        check('Блокнот и Калькулятор на Rust: своё имя окна, вкладки, сохранение, счёт',
          /имя == "glower-notepad"/.test(главная) && /имя == "glower-calc"/.test(главная)
            && /set_prgname/.test(общее) && /gtk::Notebook/.test(блокнот) && /Сохранить как/.test(блокнот)
            && /fn нажми/.test(счёт) && /считает_как_страничный/.test(счёт));
        check('агент запускает родные программы',
          /'sys\.native\.list'/.test(агент2) && /'sys\.native'/.test(агент2) && /glower-notepad/.test(агент2));
        check('в доке окна родных программ — точкой под их значком, не чужим',
          родная('glower-notepad', 'notepad') && родная('glower-calc', 'calc') && /crate::core::НАШИ/.test(док_));
        const проводник = await readFile(join(root, 'panel', 'src', 'explorer.rs'), 'utf8');
        check('Проводник на Rust: вкладки, сетка и таблица, корзина системы, флешки сбоку',
          /имя == "glower-files"/.test(главная) && /gtk::IconView/.test(проводник) && /gtk::TreeView/.test(проводник)
            && /Место::Корзина/.test(проводник) && /\.trash\(/.test(проводник) && /VolumeMonitor/.test(проводник)
            && /monitor_directory/.test(проводник) && /text\/uri-list/.test(проводник) && /GLOWER_ROOT/.test(проводник)
            && /files:'glower-files'/.test(агент2) && /GLOWER_ROOT:ROOT/.test(агент2)
            && родная('glower-files', 'files') && /adwaita-icon-theme/.test(образ2) && /adwaita-icon-theme/.test(пакет));
        const параметры = await readFile(join(root, 'panel', 'src', 'settings.rs'), 'utf8');
        check('Параметры на Rust: все восемнадцать разделов, системное — у агента, своё — у ядра панели',
          /имя == "glower-settings"/.test(главная) && (параметры.match(/Раздел \{ id:/g) || []).length === 18
            && ['sys.wifi.scan', 'sys.bt', 'sys.printers', 'sys.drives', 'sys.users', 'pkg.upgrade.check', 'sys.firmware', 'sys.firewall', 'sys.screens', 'sys.hardware']
                 .every(м => параметры.includes('"' + м + '"'))
            && /"параметры-дело"/.test(параметры) && /слушай\(&\["параметры"\]/.test(параметры)
            && /settings:'glower-settings'/.test(агент2) && /\^\[a-z0-9\]\{1,20\}\$/.test(агент2)
            && родная('glower-settings', 'settings'));
        const магазин = await readFile(join(root, 'panel', 'src', 'store.rs'), 'utf8');
        const каталог = JSON.parse(await readFile(join(root, 'panel', 'src', 'store-catalog.json'), 'utf8'));
        check('Магазин на Rust: витрина, разделы, поиск, установка, обновления, «Моё»',
          /имя == "glower-store"/.test(главная) && ['"pkg.install"', '"pkg.remove"', '"pkg.search"', '"pkg.info"', '"pkg.job"', '"pkg.flathub"', '"pkg.upgrade.check"', '"sys.apps"', '"store.icon"']
              .every(м => магазин.includes(м))
            && /store:'glower-store'/.test(агент2) && /'store\.icon'/.test(агент2) && /'--find=' \+/.test(агент2)
            && родная('glower-store', 'store') && /steam-devices/.test(магазин));
        check('у Магазина есть подборки, витрина и разделы',
          !!каталог['подборки'] && Array.isArray(каталог['витрина']) && Array.isArray(каталог['разделы']));
        check('корзина одна: стол и Проводник кладут в системную, док открывает её Проводником',
          /'fs\.trash'/.test(агент2) && /'fs\.trash\.empty'/.test(агент2) && /GLOWER_TRASH/.test(агент2)
            && /"--trash"/.test(проводник) && /fn открой_корзину/.test(док_));
        const диспетчер = await readFile(join(root, 'panel', 'src', 'taskmgr.rs'), 'utf8');
        const фото = await readFile(join(root, 'panel', 'src', 'photos.rs'), 'utf8');
        const ножницы = await readFile(join(root, 'panel', 'src', 'snip.rs'), 'utf8');
        const снимок = await readFile(join(root, 'linux', 'glower-shot'), 'utf8');
        check('Диспетчер задач, Фото и Ножницы на Rust',
          /имя == "glower-taskmgr"/.test(главная) && /имя == "glower-photos"/.test(главная) && /имя == "glower-snip"/.test(главная)
            && /\/proc/.test(диспетчер) && /fn kill/.test(диспетчер) && /СИСТЕМНОЕ/.test(диспетчер)
            && /apply_embedded_orientation/.test(фото) && /\.trash\(/.test(фото) && /"wallpaper"/.test(фото)
            && /"grim"/.test(ножницы) && /wl-copy/.test(ножницы) && /Layer::Overlay/.test(ножницы) && /Замазать/.test(ножницы)
            && /taskmgr:'glower-taskmgr', photos:'glower-photos', snip:'glower-snip'/.test(агент2)
            && /exec glower-snip "\$MODE"/.test(снимок) && /"glower-photos"\)\.arg/.test(проводник)
            && родная('glower-taskmgr', 'taskmgr') && родная('glower-snip', 'snip'));
        const меню_ = await readFile(join(root, 'panel', 'src', 'menus.rs'), 'utf8');
        check('док выезжает и тает плавно, а не рывком',
          /fn покажи\(окно: &gtk::Window, видно: bool\)/.test(док_) && /set_layer_shell_margin\(Edge::Bottom, \(10\.0 - /.test(док_)
            && /set_opacity\(плавно\)/.test(док_) && /покажи\(&окно, видно\)/.test(док_));
        check('верхняя полоса темнеет, пока открыто окно; питание предупреждает стол',
          /следи_за_окнами/.test(главная) && /fn следи_за_окнами/.test(док_) && /set_source_rgba\(0\.055, 0\.063, 0\.086/.test(главная)
            && /скажи\("питание", json!\(id\)\)/.test(меню_));
        check('нативные программы системы попадают в образ и пакет',
          /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-notepad"/.test(образ2) && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-calc"/.test(образ2)
            && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-files"/.test(образ2)
            && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-settings"/.test(образ2) && пакет.includes('usr/bin/glower-settings')
            && /ln -sf glower-panel "\$ROOTFS\/usr\/bin\/glower-store"/.test(образ2) && пакет.includes('usr/bin/glower-store')
            && /for prog in glower-taskmgr glower-photos glower-snip/.test(образ2) && пакет.includes('usr/bin/glower-snip')
            && пакет.includes('usr/bin/glower-notepad') && пакет.includes('usr/bin/glower-calc') && пакет.includes('usr/bin/glower-files'));
        /* Всё, что прежде делала страница, теперь у ядра панели */
        const ядро = await readFile(join(root, 'panel', 'src', 'core.rs'), 'utf8');
        const увед = await readFile(join(root, 'panel', 'src', 'notify.rs'), 'utf8');
        const мастер = await readFile(join(root, 'panel', 'src', 'setup.rs'), 'utf8');
        const уст = await readFile(join(root, 'panel', 'src', 'installer.rs'), 'utf8');
        const муз = await readFile(join(root, 'panel', 'src', 'music.rs'), 'utf8');
        check('ядро хранит настройки и отвечает всем, кого прежде слушала страница',
          /settings\.json/.test(ядро) && ['"обзор-дело"', '"значки-дело"', '"параметры-дело"', '"файл-установщик"', '"windows"', '"пробуждение"', '"клавиша"', '"стол"']
            .every(т => ядро.includes(т)) && /"нативно": true/.test(ядро) && /core::начни\(\)/.test(главная)
            && /fn авто_замок/.test(ядро) && /"swayidle"/.test(ядро) && /fn окна_этого_стола/.test(ядро));
        check('уведомления: служба freedesktop, всплывающие и центр с «Не беспокоить»',
          /org\.freedesktop\.Notifications/.test(увед) && /"GetCapabilities"/.test(увед) && /ActionInvoked/.test(увед)
            && /fn центр/.test(увед) && /Не беспокоить/.test(увед) && /notify::служба\(\)/.test(главная) && /notify::колокол\(\)/.test(главная));
        check('мастер первого запуска и приветствие из пыли',
          /Раскладки клавиатуры/.test(мастер) && /"sys\.kb\.set"/.test(мастер) && /"sys\.locale\.set"/.test(мастер)
            && /"setupDone", json!\(true\)/.test(мастер) && /fn приветствие/.test(мастер) && /FillRule::EvenOdd/.test(мастер)
            && /setup::покажи\(\)/.test(ядро));
        check('установщик и Музыка — свои программы, в образе и пакете',
          /имя == "glower-installer"/.test(главная) && /"install\.start"/.test(уст) && /glower\.install=1/.test(уст) && /"cryptPassword"/.test(уст)
            && /имя == "glower-music"/.test(главная) && /--input-ipc-server/.test(муз) && /"end-file"/.test(муз)
            && /glower-installer glower-music/.test(образ2) && пакет.includes('usr/bin/glower-installer') && пакет.includes('usr/bin/glower-music')
            && /glower\.install=1/.test(ядро));
      }
      check('Win и Win+Tab просят панель, не поднимая ничего поверх программ',
        /"тема":"открой-пуск"/.test(меню3) && /WHAT=задачи/.test(меню3) && !/glowershell/.test(меню3));
      check('панель слушает свои темы шины',
        темы.every(т => главная.includes('"' + т + '"'))
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

    /* --- Bluetooth: кнопка там, где она поможет, и только там --- */
    {
      const пар = await readFile(join(root, 'panel', 'src', 'settings.rs'), 'utf8');
      check('Параметры дают включить Bluetooth, когда это поможет',
        /"sys\.bt\.power"/.test(пар) && /Попробовать включить/.test(пар));
    }

    /* Проба выделения области — та, что показывает «врач». */
    const проба = await page.evaluate(() =>
      Platform.rpc('sys.shot.проба').catch(e => ({ ошибка:String(e.message || e) })));
    check('агент умеет проверить, работает ли выделение области',
      проба && проба.есть && 'grim' in проба.есть, JSON.stringify(проба).slice(0, 120));

  }

  /* --- обновление системы как в Windows: обновить, потом перезагрузить ---
     Система подменяется: здесь не место ни настоящему apt, ни настоящей
     перезагрузке. Проверяем сам ход — экран, шаги и то, что питание зовут
     ровно один раз и только после удачного обновления. */
  {
    const ядро = await readFile(join(root, 'panel', 'src', 'core.rs'), 'utf8');
    const тело = ядро.slice(ядро.indexOf('fn обнови_и'), ядро.indexOf('fn обнови_и') + 4000);
    check('обновление системы: экран «не выключайте компьютер», потом питание — только при удаче',
      /Не выключайте компьютер/.test(тело) && /"pkg\.upgrade\.run"/.test(тело) && /"pkg\.job"/.test(тело)
        && /"sys\.power"/.test(тело));
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

  /* --- новые программы, тема, музыка в быстрых настройках, язык ---
     Это проверки на месте: что нужные куски есть и связаны между собой.
     Работу на экране проверяют стенд и тесты самой панели (cargo test). */
  {
    const src = f => readFile(join(root, 'panel', 'src', f), 'utf8');
    const [главная, ядро, тема, муз, медиа, язык, меню, образ, пакет, агент, рц, пакеты] = await Promise.all([
      src('main.rs'), src('core.rs'), src('theme.rs'), src('music.rs'), src('media.rs'), src('i18n.rs'), src('menus.rs'),
      readFile(join(root, 'linux', 'mkiso.sh'), 'utf8'), readFile(join(root, 'linux', 'собери-пакет.sh'), 'utf8'),
      readFile(join(root, 'agent', 'server.mjs'), 'utf8'), readFile(join(root, 'linux', 'labwc', 'rc.xml'), 'utf8'),
      readFile(join(root, 'agent', 'packages.mjs'), 'utf8')]);
    const новые = ['paint', 'calendar', 'clock', 'todo', 'props'];
    check('Paint, Календарь, Часы, Дела и Свойства — свои программы, в образе и в пакете',
      новые.every(п => главная.includes('"glower-' + п + '" =>') && образ.includes('glower-' + п) && пакет.includes('glower-' + п))
        && ['paint', 'calendar', 'clock', 'todo'].every(п => ядро.includes('id: "' + п + '"')));
    check('светлая тема и цвет акцента — именами цветов, на лету',
      /@define-color glower_accent/.test(тема) && /FileMonitor|monitor_file/.test(тема) && /theme::подключи\(\)/.test(главная));
    check('Музыка говорит по MPRIS, быстрые настройки и медиаклавиши ею управляют',
      /org\.mpris\.MediaPlayer2/.test(муз) && /PropertiesChanged/.test(муз) && /fn что_играет/.test(медиа)
        && /now-playing/.test(меню) && /XF86AudioPlay/.test(рц) && /медиа/.test(рц));
    check('язык интерфейса: словари всех языков, перевод при показе, имена файлов не трогает',
      ['en', 'uk', 'be', 'kk', 'de', 'fr', 'es', 'it', 'pt', 'pl', 'tr'].every(к => язык.includes('lang/' + к + '.json'))
        && /g_signal_add_emission_hook/.test(язык) && /no-i18n/.test(язык) && /i18n::подключи\(\)/.test(главная));
    check('обновление убирает прежний движок оболочки, но не браузер',
      /apt-mark', 'auto'/.test(пакеты) && /'autoremove', '-y'/.test(пакеты) && /gir1\.2-webkit2-4\.1/.test(пакеты)
        && !/epiphany|chromium|firefox/.test(пакеты.slice(пакеты.indexOf('СТАРЫЙ_ДВИЖОК'), пакеты.indexOf('const начни'))));
    check('агент запускает новые программы',
      новые.every(п => агент.includes(п + ":'glower-" + п + "'")));
  }

  /* --- страницы рабочего стола больше нет ---
     Ни её файлов, ни браузерного движка в агенте, ни раздачи файлов по HTTP:
     весь рабочий стол — glower-panel. */
  check('страницы рабочего стола и её движка в системе нет',
    ['index.html', 'панель.html', 'js', 'css', 'tools'].every(ф => !existsSync(join(root, ф)))
      && !existsSync(join(root, 'agent', 'browser.mjs'))
      && !/browser\.mjs|UI_DIR/.test(await readFile(join(root, 'agent', 'server.mjs'), 'utf8')));

  /* --- свой оконный сервер glower-wm ---
     Сам он проверяется своими тестами (cargo test в wm/) и на стенде;
     здесь — что он встроен в систему и что откат на labwc на месте. */
  {
    const wm = f => readFile(join(root, 'wm', 'src', f), 'utf8');
    const сеанс = await readFile(join(root, 'linux', 'glower-session'), 'utf8');
    const образ = await readFile(join(root, 'linux', 'mkiso.sh'), 'utf8');
    const пакет = await readFile(join(root, 'linux', 'собери-пакет.sh'), 'utf8');
    const сборка = await readFile(join(root, '..', '.github', 'workflows', 'build-iso.yml'), 'utf8');
    const ядро = await readFile(join(root, 'panel', 'src', 'core.rs'), 'utf8');
    const параметры = await readFile(join(root, 'panel', 'src', 'settings.rs'), 'utf8');
    const система = await readFile(join(root, 'agent', 'system.mjs'), 'utf8');
    const [главный, ключи, размытие, протоколы] = await Promise.all([wm('main.rs'), wm('keys.rs'), wm('blur.rs'), wm('protocols/mod.rs')]);
    check('glower-wm читает rc.xml labwc и понимает -C, -s и -e',
      /linux\/labwc\/rc\.xml/.test(ключи) && /"-C"/.test(главный) && /"-s"/.test(главный) && /"-e"/.test(главный));
    check('glower-wm: размытие маской по альфе, протоколы панели, замка и снимков',
      /smoothstep/.test(размытие) && ['foreign_toplevel', 'inhibitor', 'screencopy', 'gamma'].every(м => протоколы.includes('pub mod ' + м)));
    check('сеанс пробует glower-wm по выбору и откатывается на labwc',
      /run_glower_wm/.test(сеанс) && /"wm"\[\[:space:\]\]\*:/.test(сеанс) && /lived" -lt 20/.test(сеанс)
        && сеанс.indexOf('run_glower_wm && exit 0') < сеанс.indexOf('run_labwc && exit 0') && /glower-wm -e/.test(сеанс));
    check('glower-wm в образе, пакете и сборке; выбор — в Параметрах',
      /glower-wm/.test(образ) && /libseat1/.test(образ) && /usr\/bin\/glower-wm/.test(пакет) && /glower\/wm\/Cargo\.toml/.test(сборка)
        && /"wm", json!\(if v \{ "glower" \}/.test(параметры));
    check('панель просит столы у glower-wm словами, раскладку и SIGHUP понимают оба сервера',
      /GLOWER_WM_SOCKET/.test(ядро) && /"desk"/.test(ядро) && /'labwc', 'glower-wm'/.test(система));
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
    const r = JSON.stringify(await rpc('sys.hardware').catch(e => ({ ошибка:e.message })));
    const { readFileSync } = await import('node:fs');
    const модель = ((readFileSync('/proc/cpuinfo', 'utf8').match(/^model name\s*:\s*(.+)$/m) || [])[1] || '')
      .replace(/\((R|TM|tm)\)/g, '').replace(/\s+CPU\s+@.*$/, '').replace(/\s{2,}/g, ' ').trim().slice(0, 12);
    const сырая = ((readFileSync('/proc/cpuinfo', 'utf8').match(/^model name\s*:\s*(.+)$/m) || [])[1] || '').trim();
    void модель;
    check('«О системе» получает от машины модель процессора',
      !сырая || r.includes(JSON.stringify(сырая).slice(1, -1)), r.slice(0, 300));
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

  /* Удалённое — в системную корзину, и очищается оттуда */
  {
    await rpc('fs.write', { path:'Документы/в-корзину.txt', body:'мусор' });
    await rpc('fs.trash', { path:'Документы/в-корзину.txt' });
    const сколько = (await rpc('fs.trash.count'))['сколько'];
    await rpc('fs.trash.empty', {});
    const после = (await rpc('fs.trash.count'))['сколько'];
    const { readdir: rd } = await import('node:fs/promises');
    check('удалённое ложится в системную корзину и очищается оттуда',
      сколько === 1 && после === 0, JSON.stringify({ сколько, после }));
    check('сам файл с диска ушёл', !existsSync(join(WS, 'Документы', 'в-корзину.txt')), (await rd(join(WS, 'Документы')).catch(() => [])).join(','));
  }

  /* Состояние темы не вытесняется болтовнёй других тем */
  {
    const r = await page.evaluate(async () => {
      await Platform.rpc('ui.say', { 'тема':'проверка-состояния', 'что':{ 'док':'полный' } });
      for (let i = 0; i < 230; i++) await Platform.rpc('ui.say', { 'тема':'проверка-шум', 'что':i });
      return (await Platform.rpc('ui.last', { 'тема':'проверка-состояния' })).что;
    });
    check('шина помнит последнее по теме и после двухсот чужих сообщений', r && r['док'] === 'полный', JSON.stringify(r));
  }


} catch(e){
  failed++; out.push('  ❌ упало с исключением — ' + e.message);
} finally {
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
