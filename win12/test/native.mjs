#!/usr/bin/env node
/* ==========================================================================
   Проверка связки «оболочка + системный агент»: работает ли настоящий диск.

   Запуск:  node test/native.mjs
   Поднимает агент на свободном порту во временной папке, открывает оболочку
   в Chromium и проверяет, что файлы читаются и пишутся по-настоящему.
   ========================================================================== */
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const PORT = 8300 + Math.floor(Math.random() * 400);

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

/* ---------- рабочая папка с настоящими файлами ---------- */
const WS = await mkdtemp(join(tmpdir(), 'win12-'));
await mkdir(join(WS, 'Документы'), { recursive:true });
await mkdir(join(WS, 'Рабочий стол'), { recursive:true });
await writeFile(join(WS, 'Документы', 'с-диска.txt'), 'этот файл лежит на настоящем диске', 'utf8');

const agent = spawn(process.execPath, [join(root, 'agent/server.mjs'), '--port', String(PORT), '--root', WS],
  { stdio:['ignore', 'pipe', 'pipe'] });
await new Promise(r => setTimeout(r, 1200));

const browser = await chromium.launch(exe ? { executablePath:exe, args:['--no-sandbox'] } : { args:['--no-sandbox'] });
const page = await browser.newPage({ viewport:{ width:1280, height:820 } });
const errs = [];
page.on('pageerror', e => errs.push(e.message));

try {
  await page.goto(`http://localhost:${PORT}/`);
  await page.waitForTimeout(2600);
  await page.keyboard.press('Enter');
  await page.waitForTimeout(1800);

  check('оболочка отдаётся агентом и запускается',
    await page.evaluate(() => $('#desktop').classList.contains('on')));
  check('подключение к агенту установлено',
    await page.evaluate(() => Platform.mode === 'native'), await page.evaluate(() => Platform.mode));
  check('в трее виден индикатор системы', (await page.$$('#agent-badge')).length === 1);

  /* --- чтение настоящего файла --- */
  check('файл с диска виден в файловой системе оболочки',
    await page.evaluate(() => {
      const f = FS.node(['Документы', 'с-диска.txt']);
      return f && f.body.includes('настоящем диске');
    }));

  /* --- запись из оболочки попадает на диск --- */
  await page.evaluate(() => FS.write(['Документы'], 'из-оболочки.txt', 'создано в Windows 12'));
  await page.waitForTimeout(900);
  const written = existsSync(join(WS, 'Документы', 'из-оболочки.txt'))
    && (await readFile(join(WS, 'Документы', 'из-оболочки.txt'), 'utf8')).includes('создано в Windows 12');
  check('файл, созданный в оболочке, появился на диске', written);

  /* --- через приложение, а не напрямую --- */
  await page.evaluate(() => WM.open('files', { path:['Рабочий стол'] }));
  await page.waitForTimeout(900);
  await page.evaluate(() => FS.mkdir(['Рабочий стол'], 'Папка из Проводника'));
  await page.waitForTimeout(900);
  check('папка из Проводника создана на диске',
    existsSync(join(WS, 'Рабочий стол', 'Папка из Проводника')));

  /* --- переименование и удаление --- */
  await page.evaluate(() => FS.rename(['Документы'], 'из-оболочки.txt', 'переименован.txt'));
  await page.waitForTimeout(900);
  check('переименование дошло до диска',
    existsSync(join(WS, 'Документы', 'переименован.txt'))
    && !existsSync(join(WS, 'Документы', 'из-оболочки.txt')));

  await page.evaluate(() => FS.rm(['Документы'], 'переименован.txt', true));
  await page.waitForTimeout(900);
  check('удаление дошло до диска', !existsSync(join(WS, 'Документы', 'переименован.txt')));

  /* --- изменения на диске подхватываются --- */
  await writeFile(join(WS, 'Документы', 'снаружи.txt'), 'создан мимо оболочки', 'utf8');
  await page.evaluate(() => Platform.mount());
  await page.waitForTimeout(1200);
  check('файл, созданный вне оболочки, появляется после синхронизации',
    await page.evaluate(() => !!FS.node(['Документы', 'снаружи.txt'])));

  /* --- слежение: правку снаружи оболочка подхватывает сама --- */
  await writeFile(join(WS, 'Документы', 'сам-подхватил.txt'), 'без ручной синхронизации', 'utf8');
  let seen = false;
  for (let i = 0; i < 12 && !seen; i++){
    await page.waitForTimeout(1000);
    seen = await page.evaluate(() => !!FS.node(['Документы', 'сам-подхватил.txt']));
  }
  check('правка папки снаружи подхватывается без ручной синхронизации', seen);

  /* --- открытие в системе выключено, пока не разрешили ключом --- */
  const openBlocked = await page.evaluate(async () => {
    try { await Platform.open(['Документы', 'с-диска.txt']); return 'открыл'; }
    catch(e){ return e.message; }
  });
  check('без ключа --allow-open агент файлы в системе не открывает',
    /--allow-open/.test(openBlocked), openBlocked);

  /* --- граница доверия --- */
  const escape = await page.evaluate(async () => {
    try { await Platform.rpc('fs.read', { path:['..', '..', 'etc', 'passwd'] }); return 'прочитал'; }
    catch(e){ return e.message; }
  });
  check('за пределы рабочей папки агент не пускает', /вне рабочей папки/.test(escape), escape);

  const noExec = await page.evaluate(async () => {
    try { await Platform.rpc('sys.exec', { cmd:'ls' }); return 'выполнил'; }
    catch(e){ return e.message; }
  });
  check('запуск программ агенту недоступен', /нет такого метода/.test(noExec), noExec);

  check('в консоли нет ошибок JS', errs.length === 0, errs.slice(0, 2).join(' | '));

} catch(e){
  failed++; out.push('  ❌ упало с исключением — ' + e.message);
} finally {
  await browser.close();
  agent.kill();
}

console.log('\nПроверка работы с настоящей системой\n');
console.log(`  рабочая папка: ${WS}\n`);
console.log(out.join('\n'));
console.log(`\n  Пройдено: ${passed} · Провалено: ${failed}\n`);
process.exit(failed ? 1 : 0);
