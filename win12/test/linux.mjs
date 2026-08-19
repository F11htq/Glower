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
