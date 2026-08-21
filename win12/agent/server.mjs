#!/usr/bin/env node
/* ==========================================================================
   Системный агент GlowerOS

   Даёт оболочке то, чего у страницы нет: настоящий диск.
   Запуск:  node agent/server.mjs [--port 8123] [--root ~/Windows12]
   Затем открыть  http://localhost:8123  — оболочка увидит реальные файлы.

   Намеренно НЕ умеет: запускать программы, читать что-либо вне корневой
   папки, ходить в сеть. Это граница доверия, а не недоделка.
   ========================================================================== */
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, rm, rename, readdir, stat } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, resolve, relative, dirname, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';

const UI_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const arg = (name, def) => {
  const i = process.argv.indexOf('--' + name);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : def;
};
const PORT = +arg('port', 8123);
const ROOT = resolve(arg('root', join(homedir(), 'Windows12')));
const MAX_INLINE = 256 * 1024;          // файлы больше не грузим в дерево целиком
const ALLOW_OPEN   = process.argv.includes('--allow-open');    // открывать файлы в системе
const ALLOW_POWER  = process.argv.includes('--allow-power');   // выключение и перезагрузка
const ALLOW_LAUNCH = process.argv.includes('--allow-launch');  // запуск программ машины
const ALLOW_INSTALL= process.argv.includes('--allow-install'); // установка системы на диск
const ALLOW_NET    = process.argv.includes('--allow-net');     // подключение к сетям Wi-Fi
const ALLOW_PKG    = process.argv.includes('--allow-packages'); // установка программ машины
const SYSTEM       = process.argv.includes('--system') || ALLOW_POWER || ALLOW_LAUNCH;

/* счётчик изменений: оболочка по нему понимает, что папку правили снаружи */
let rev = 0;
const bump = () => { rev++; };

const TEXT = /\.(txt|md|log|json|js|css|html?|xml|csv|ini|conf|yml|yaml|py|sh|ts|jsx?)$/i;
const IMAGE = /\.(png|jpe?g|gif|webp|bmp|svg|ico)$/i;
const MIME = { html:'text/html', js:'text/javascript', css:'text/css', json:'application/json',
  png:'image/png', jpg:'image/jpeg', svg:'image/svg+xml', ico:'image/x-icon', mjs:'text/javascript' };

/* ---------- защита: наружу корня не выходим ---------- */
function safe(parts){
  const p = resolve(ROOT, ...(Array.isArray(parts) ? parts : String(parts || '').split('/')).filter(Boolean));
  const rel = relative(ROOT, p);
  if (rel.startsWith('..') || (rel.includes('..' + sep))) throw new Error('путь вне рабочей папки');
  return p;
}

/* ---------- чтение дерева ---------- */
async function tree(dir = ROOT, depth = 0){
  const out = {};
  let items = [];
  try { items = await readdir(dir, { withFileTypes:true }); } catch(e){ return out; }
  for (const it of items){
    if (it.name.startsWith('.')) continue;
    const full = join(dir, it.name);
    let st;
    try { st = await stat(full); } catch(e){ continue; }
    if (it.isDirectory()){
      out[it.name] = { type:'dir', name:it.name, ctime:+st.birthtimeMs || +st.ctimeMs, mtime:+st.mtimeMs,
        children: depth < 6 ? await tree(full, depth + 1) : {} };
    } else {
      const node = { type:'file', name:it.name, ctime:+st.birthtimeMs || +st.ctimeMs, mtime:+st.mtimeMs, size:st.size };
      if (st.size <= MAX_INLINE && TEXT.test(it.name)){
        try { node.body = await readFile(full, 'utf8'); } catch(e){ node.body = ''; }
      } else if (st.size <= MAX_INLINE && IMAGE.test(it.name)){
        try {
          const b = await readFile(full);
          const ext = extname(it.name).slice(1).toLowerCase();
          node.img = `data:${MIME[ext] || 'image/' + ext};base64,` + b.toString('base64');
        } catch(e){}
      } else {
        node.body = ''; node.truncated = true;
      }
      out[it.name] = node;
    }
  }
  return out;
}

/* ---------- слежение за рабочей папкой ---------- */
import { watch } from 'node:fs';
let watcher = null;
function startWatch(){
  try {
    watcher = watch(ROOT, { recursive:true }, () => bump());
    watcher.on('error', () => { watcher = null; startPoll(); });
  } catch(e){ startPoll(); }
}
function startPoll(){                   // если рекурсивное слежение недоступно
  let last = '';
  setInterval(async () => {
    const sig = JSON.stringify(await tree()).length + ':' + Date.now().toString().slice(0, 8);
    const short = sig.split(':')[0];
    if (last && short !== last) bump();
    last = short;
  }, 3000);
}

/* ---------- системный слой: только если запрошен ключом --system ---------- */
let SYS = {};
if (SYSTEM){
  const m = await import('./system.mjs');
  SYS = {
    ...m.power(ALLOW_POWER), ...m.sound, ...m.backlight, ...m.net, ...m.procs,
    ...m.apps(ALLOW_LAUNCH), ...m.hardware, ...m.wifi(ALLOW_NET),
    ...(await import('./browser.mjs')).browser(PORT, ALLOW_LAUNCH),
    ...(await import('./install.mjs')).install(ALLOW_INSTALL),
    ...(await import('./packages.mjs')).packages(ALLOW_PKG),
    async 'sys.caps'(){ return m.capabilities({ power:ALLOW_POWER, launch:ALLOW_LAUNCH, open:ALLOW_OPEN, install:ALLOW_INSTALL, net:ALLOW_NET, packages:ALLOW_PKG }); }
  };
}

/* ---------- методы, доступные оболочке ---------- */
const API = {
  async ping(){
    return { ok:true, agent:'win12-agent', version:'1.2', root:ROOT,
             platform:process.platform, node:process.version, pid:process.pid,
             canOpen:ALLOW_OPEN, system:SYSTEM,
             canPower:ALLOW_POWER, canLaunch:ALLOW_LAUNCH, rev };
  },
  async 'fs.rev'(){ return { rev }; },
  async 'fs.tree'(){ return { root:{ type:'dir', name:'', children: await tree() } }; },
  async 'fs.read'({ path }){ return { body: await readFile(safe(path), 'utf8') }; },
  async 'fs.write'({ path, body }){
    const p = safe(path);
    await mkdir(dirname(p), { recursive:true });
    await writeFile(p, body ?? '', 'utf8');
    bump(); return { ok:true };
  },
  async 'fs.writeDataUrl'({ path, dataUrl }){
    const p = safe(path);
    await mkdir(dirname(p), { recursive:true });
    const b64 = String(dataUrl).split(',')[1] || '';
    await writeFile(p, Buffer.from(b64, 'base64'));
    bump(); return { ok:true };
  },
  async 'fs.mkdir'({ path }){ await mkdir(safe(path), { recursive:true }); bump(); return { ok:true }; },
  async 'fs.remove'({ path }){ await rm(safe(path), { recursive:true, force:true }); bump(); return { ok:true }; },
  async 'fs.rename'({ from, to }){ await rename(safe(from), safe(to)); bump(); return { ok:true }; },
  async 'fs.stat'({ path }){
    const st = await stat(safe(path));
    return { size:st.size, dir:st.isDirectory(), ctime:+st.birthtimeMs || +st.ctimeMs, mtime:+st.mtimeMs };
  },
  async 'sys.open'({ path }){
    if (!ALLOW_OPEN) throw new Error('открытие в системе выключено: запустите агент с ключом --allow-open');
    const p = safe(path);
    const { spawn } = await import('node:child_process');
    const cmd = process.platform === 'darwin' ? 'open'
              : process.platform === 'win32' ? 'explorer' : 'xdg-open';
    spawn(cmd, [p], { detached:true, stdio:'ignore' }).unref();
    return { ok:true, opened:p, via:cmd };
  },
  ...SYS,
  async 'sys.info'(){
    const { totalmem, freemem, cpus, hostname, userInfo, uptime, release } = await import('node:os');
    return { host:hostname(), user:userInfo().username, platform:process.platform, release:release(),
             cpus:cpus().length, mem:totalmem(), memFree:freemem(), uptime:Math.round(uptime()) };
  }
};

/* ---------- сервер: статика оболочки + JSON-RPC ---------- */
const send = (res, code, body, headers = {}) => {
  res.writeHead(code, { 'Access-Control-Allow-Origin':'*',
    'Access-Control-Allow-Headers':'content-type', 'Cache-Control':'no-store', ...headers });
  res.end(body);
};

const server = createServer(async (req, res) => {
  if (req.method === 'OPTIONS') return send(res, 204, '');

  if (req.url.split('?')[0] === '/rpc'){
    let raw = '';
    req.on('data', c => raw += c);
    req.on('end', async () => {
      let out;
      try {
        const { method, params } = JSON.parse(raw || '{}');
        if (!API[method]) throw new Error('нет такого метода: ' + method);
        out = { ok:true, result: await API[method](params || {}) };
      } catch(e){ out = { ok:false, error:String(e.message || e) }; }
      send(res, 200, JSON.stringify(out), { 'Content-Type':'application/json; charset=utf-8' });
    });
    return;
  }

  // статика оболочки
  let p = decodeURIComponent(req.url.split('?')[0]);
  if (p === '/') p = '/index.html';
  const file = resolve(UI_DIR, '.' + p);
  if (!file.startsWith(UI_DIR) || !existsSync(file)) return send(res, 404, 'not found');
  try {
    const data = await readFile(file);
    const ext = extname(file).slice(1).toLowerCase();
    send(res, 200, data, { 'Content-Type':(MIME[ext] || 'text/plain') + '; charset=utf-8' });
  } catch(e){ send(res, 500, 'error'); }
});

await mkdir(ROOT, { recursive:true });
if (!existsSync(join(ROOT, 'Документы'))){
  await mkdir(join(ROOT, 'Документы'), { recursive:true });
  await mkdir(join(ROOT, 'Рабочий стол'), { recursive:true });
  await mkdir(join(ROOT, 'Изображения'), { recursive:true });
  await mkdir(join(ROOT, 'Загрузки'), { recursive:true });
  await writeFile(join(ROOT, 'Рабочий стол', 'Начало.txt'),
    'Это настоящий файл на вашем диске.\n\nОткройте его, поправьте, сохраните — и посмотрите в папке ' + ROOT + '\n', 'utf8');
}

startWatch();

server.listen(PORT, () => {
  console.log(`\n  Агент GlowerOS запущен`);
  console.log(`  Оболочка:      http://localhost:${PORT}`);
  console.log(`  Рабочая папка: ${ROOT}`);
  console.log(`  Разрешено: чтение и запись только внутри этой папки.`);
  console.log(`  Открытие файлов в системе: ${ALLOW_OPEN ? 'разрешено ключом --allow-open' : 'выключено'}`);
  console.log(`  Системный слой:            ${SYSTEM ? 'включён ключом --system' : 'выключен'}`);
  if (SYSTEM){
    console.log(`    громкость, яркость, сеть, процессы, список программ: чтение и настройка`);
    console.log(`    питание:  ${ALLOW_POWER ? 'разрешено ключом --allow-power' : 'выключено'}`);
    console.log(`    запуск программ: ${ALLOW_LAUNCH ? 'разрешён ключом --allow-launch' : 'выключен'}`);
    console.log(`    установка на диск: ${ALLOW_INSTALL ? 'разрешена ключом --allow-install' : 'выключена'}`);
    console.log(`    сети Wi-Fi:        ${ALLOW_NET ? 'разрешены ключом --allow-net' : 'выключены'}`);
    console.log(`    программы машины:  ${ALLOW_PKG ? 'разрешены ключом --allow-packages' : 'выключены'}`);
  }
  console.log(`  Произвольные команды агенту недоступны в любом режиме.\n`);
});
