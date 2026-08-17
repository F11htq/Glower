#!/usr/bin/env node
/* ==========================================================================
   Системный агент Windows 12 Prototype

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

/* ---------- методы, доступные оболочке ---------- */
const API = {
  async ping(){
    return { ok:true, agent:'win12-agent', version:'1.0', root:ROOT,
             platform:process.platform, node:process.version, pid:process.pid };
  },
  async 'fs.tree'(){ return { root:{ type:'dir', name:'', children: await tree() } }; },
  async 'fs.read'({ path }){ return { body: await readFile(safe(path), 'utf8') }; },
  async 'fs.write'({ path, body }){
    const p = safe(path);
    await mkdir(dirname(p), { recursive:true });
    await writeFile(p, body ?? '', 'utf8');
    return { ok:true };
  },
  async 'fs.writeDataUrl'({ path, dataUrl }){
    const p = safe(path);
    await mkdir(dirname(p), { recursive:true });
    const b64 = String(dataUrl).split(',')[1] || '';
    await writeFile(p, Buffer.from(b64, 'base64'));
    return { ok:true };
  },
  async 'fs.mkdir'({ path }){ await mkdir(safe(path), { recursive:true }); return { ok:true }; },
  async 'fs.remove'({ path }){ await rm(safe(path), { recursive:true, force:true }); return { ok:true }; },
  async 'fs.rename'({ from, to }){ await rename(safe(from), safe(to)); return { ok:true }; },
  async 'fs.stat'({ path }){
    const st = await stat(safe(path));
    return { size:st.size, dir:st.isDirectory(), ctime:+st.birthtimeMs || +st.ctimeMs, mtime:+st.mtimeMs };
  },
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

server.listen(PORT, () => {
  console.log(`\n  Агент Windows 12 запущен`);
  console.log(`  Оболочка:      http://localhost:${PORT}`);
  console.log(`  Рабочая папка: ${ROOT}`);
  console.log(`  Разрешено: чтение и запись только внутри этой папки.`);
  console.log(`  Запуск программ и сеть агенту недоступны.\n`);
});
