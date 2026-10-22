#!/usr/bin/env node
/* ==========================================================================
   Системный агент GlowerOS

   Даёт оболочке то, чего у страницы нет: настоящий диск.
   Запуск:  node agent/server.mjs [--port 8123] [--root ~/GlowerOS]
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
const ROOT = resolve(arg('root', join(homedir(), 'GlowerOS')));
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
  png:'image/png', jpg:'image/jpeg', svg:'image/svg+xml', ico:'image/x-icon', mjs:'text/javascript',
  /* обои дистрибутивов бывают и такими */
  jpeg:'image/jpeg', webp:'image/webp', avif:'image/avif' };

/* Программы системы, переписанные на Rust: id страницы → имя программы */
const НАТИВНЫЕ = { notepad:'glower-notepad', calc:'glower-calc', files:'glower-files', settings:'glower-settings', store:'glower-store',
  taskmgr:'glower-taskmgr', photos:'glower-photos', snip:'glower-snip' };

/* Системная корзина. GLOWER_TRASH — для проверок: им незачем трогать
   настоящую корзину машины */
const ПОГОДА = new Map();

function корзина(){
  const корень = process.env.GLOWER_TRASH
    || join(process.env.XDG_DATA_HOME || join(homedir(), '.local/share'), 'Trash');
  return { files:join(корень, 'files'), info:join(корень, 'info') };
}

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
    ...m.apps(ALLOW_LAUNCH), ...m.hardware, ...m.wifi(ALLOW_NET), ...m.shots(ALLOW_LAUNCH),
    ...m.wallpapers,
    ...m.keyboard,
    ...m.locale,
    ...m.tray(ALLOW_LAUNCH),
    ...(await import('./browser.mjs')).browser(PORT, ALLOW_LAUNCH),
    ...(await import('./install.mjs')).install(ALLOW_INSTALL),
    ...(await import('./packages.mjs')).packages(ALLOW_PKG),
    async 'sys.caps'(){ return m.capabilities({ power:ALLOW_POWER, launch:ALLOW_LAUNCH, open:ALLOW_OPEN, install:ALLOW_INSTALL, net:ALLOW_NET, packages:ALLOW_PKG }); }
  };
  /* Папку Windows готовим заранее — только в настоящем сеансе, где
     программы вообще разрешено запускать. */
  if (ALLOW_LAUNCH) m.подготовь_wine();
}

/* Не выключать посреди установки.

   Человек получил «dpkg was interrupted» — и дальше не мог ни обновить,
   ни поставить, ни удалить ничего. Так бывает, когда установку обрывают:
   выход из сеанса, перезагрузка, выключение, пока apt ещё работает.
   Установка — дочерний процесс сеанса, и вместе с сеансом погибает и она,
   оставляя пакеты наполовину распакованными.

   Проверяем здесь, в одном месте, через которое идут все просьбы о питании:
   из панели, со стола, с клавиатуры. Спящий режим пропускаем — после него
   работа просто продолжится. Наша же «Обновить и перезагрузить» зовёт
   питание только после того, как установка кончилась. */
if (SYS['sys.power'] && SYS['pkg.job']){
  const питание = SYS['sys.power'];
  SYS['sys.power'] = async (доводы = {}) => {
    if (['reboot', 'poweroff', 'logout'].includes(доводы.action)){
      const j = await SYS['pkg.job']().catch(() => null);
      if (j && j.running)
        throw new Error('Сейчас идёт установка программ' + (j.name ? ' («' + j.name + '»)' : '')
          + ' — если прервать её выключением или выходом, система останется наполовину '
          + 'обновлённой. Подождите, пока закончится: ход виден в Магазине и в «Параметрах».');
    }
    return питание(доводы);
  };
}

/* разговор поверхностей оболочки: рабочий стол и панель задач */
const BUS = (await import('./bus.mjs')).bus();

/* Экран входа: он живёт до всякого рабочего стола, и системный слой ему не
   нужен — нужен только разговор с greetd. Появляется, лишь когда агент
   запущен именно как экран входа. */
const LOGIN = process.env.GREETD_SOCK
  ? (await import('./login.mjs')).login()
  : {};

/* ---------- методы, доступные оболочке ---------- */
const API = {
  ...BUS,
  ...LOGIN,
  async ping(){
    return { ok:true, agent:'glower-agent', version:'1.2', root:ROOT,
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
  /* Корзина — системная, по стандарту freedesktop: та же, что у
     Проводника и у любой программы Linux. Удалённое со стола не пропадает
     и не прячется в памяти страницы, а лежит в ~/.local/share/Trash,
     откуда его видно и можно вернуть. */
  async 'fs.trash'({ path }){
    const p = safe(path);
    const { files, info } = корзина();
    await mkdir(files, { recursive:true }); await mkdir(info, { recursive:true });
    const имя = p.split(sep).pop();
    let куда = имя, n = 1;
    while (existsSync(join(files, куда)) || existsSync(join(info, куда + '.trashinfo'))){
      n++;
      const т = имя.lastIndexOf('.');
      куда = т > 0 ? `${имя.slice(0, т)} (${n})${имя.slice(т)}` : `${имя} (${n})`;
    }
    const когда = new Date(Date.now() - new Date().getTimezoneOffset() * 60000).toISOString().slice(0, 19);
    await writeFile(join(info, куда + '.trashinfo'),
      `[Trash Info]\nPath=${encodeURI(p).replace(/%2F/gi, '/')}\nDeletionDate=${когда}\n`);
    try { await rename(p, join(files, куда)); }
    catch(e){
      /* Другой диск: переименованием не перенести — копируем и убираем */
      if (e.code !== 'EXDEV') { await rm(join(info, куда + '.trashinfo'), { force:true }); throw e; }
      const { cp } = await import('node:fs/promises');
      await cp(p, join(files, куда), { recursive:true });
      await rm(p, { recursive:true, force:true });
    }
    bump(); return { ok:true, 'имя':куда };
  },
  async 'fs.trash.count'(){
    try { return { 'сколько':(await readdir(корзина().files)).length }; } catch(e){ return { 'сколько':0 }; }
  },
  async 'fs.trash.empty'(){
    const { files, info } = корзина();
    for (const д of [files, info]){
      let список = [];
      try { список = await readdir(д); } catch(e){}
      for (const ф of список) await rm(join(д, ф), { recursive:true, force:true });
    }
    return { ok:true };
  },
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
  /* Программы системы на Rust (glower-notepad, glower-calc): страница
     открывает их настоящими окнами, а не своими. Путь к файлу — внутри
     рабочей папки, как у всех вызовов страницы. Какие из них есть на
     машине, страница спрашивает один раз: нет — открывает свои. */
  /* Значок программы с Flathub — для нативного Магазина: у него самого
     нет HTTPS. Берём простой адрес, а нет там — спрашиваем Flathub, где
     лежит. Скачанное храним на диске: значок не меняется. */
  async 'store.icon'({ ид }){
    const id = String(ид || '');
    if (!/^[A-Za-z0-9._-]{3,120}$/.test(id)) return { 'есть':false };
    const папка = join(process.env.XDG_CACHE_HOME || join(homedir(), '.cache'), 'glower', 'магазин');
    const файл = join(папка, id + '.png');
    const отдай = б => ({ 'есть':true, 'данные':'data:image/png;base64,' + б.toString('base64') });
    try { return отдай(await readFile(файл)); } catch(e){}
    const возьми = async адрес => {
      try {
        const о = await fetch(адрес, { signal:AbortSignal.timeout(8000) });
        return о.ok ? Buffer.from(await о.arrayBuffer()) : null;
      } catch(e){ return null; }
    };
    let б = await возьми('https://dl.flathub.org/repo/appstream/x86_64/icons/128x128/' + id + '.png');
    if (!б){
      try {
        const о = await fetch('https://flathub.org/api/v2/appstream/' + encodeURIComponent(id),
          { signal:AbortSignal.timeout(8000) }).then(r => r.ok ? r.json() : null);
        const адрес = о && (о.icon || (Array.isArray(о.icons) && о.icons.length
          ? (о.icons.find(x => x.height === 128) || о.icons[0]).url : ''));
        if (адрес) б = await возьми(адрес);
      } catch(e){}
    }
    if (!б) return { 'есть':false };
    try { await mkdir(папка, { recursive:true }); await writeFile(файл, б); } catch(e){}
    return отдай(б);
  },
  /* Погода — для виджета стола и панели. Раньше её спрашивала страница
     сама; теперь страницы нет, и спрашивает агент: у него есть сеть.
     Ответ храним полчаса — погода так быстро не меняется. */
  async 'weather'({ город }){
    const где = String(город || 'Москва').trim().slice(0, 60);
    const было = ПОГОДА.get(где);
    if (было && Date.now() - было.ts < 30 * 60000) return было;
    const знак = c => c === 0 ? '☀️' : c <= 2 ? '🌤' : c === 3 ? '☁️' : c <= 48 ? '🌫' : c <= 57 ? '🌦'
      : c <= 67 ? '🌧' : c <= 77 ? '❄️' : c <= 82 ? '🌧' : c <= 86 ? '🌨' : '⛈';
    const слово = c => c === 0 ? 'Ясно' : c <= 2 ? 'Малооблачно' : c === 3 ? 'Пасмурно' : c <= 48 ? 'Туман'
      : c <= 57 ? 'Морось' : c <= 67 ? 'Дождь' : c <= 77 ? 'Снег' : c <= 82 ? 'Ливень' : c <= 86 ? 'Снегопад' : 'Гроза';
    const сигнал = AbortSignal.timeout(10000);
    const g = await fetch('https://geocoding-api.open-meteo.com/v1/search?count=1&language=ru&name='
      + encodeURIComponent(где), { signal:сигнал }).then(r => r.json());
    const место = g.results && g.results[0];
    if (!место) throw new Error('город не найден: ' + где);
    const w = await fetch(`https://api.open-meteo.com/v1/forecast?latitude=${место.latitude}`
      + `&longitude=${место.longitude}&current=temperature_2m,weather_code,relative_humidity_2m,wind_speed_10m`
      + `&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=6`,
      { signal:AbortSignal.timeout(10000) }).then(r => r.json());
    const дни = ['Вс','Пн','Вт','Ср','Чт','Пт','Сб'];
    const итог = { ts:Date.now(), 'город':место.name, t:Math.round(w.current.temperature_2m),
      'знак':знак(w.current.weather_code), 'слово':слово(w.current.weather_code),
      'влажность':w.current.relative_humidity_2m, 'ветер':w.current.wind_speed_10m,
      'дни':w.daily.time.slice(1, 6).map((д, i) => ({ 'день':дни[new Date(д).getDay()],
        'знак':знак(w.daily.weather_code[i + 1]), max:Math.round(w.daily.temperature_2m_max[i + 1]),
        min:Math.round(w.daily.temperature_2m_min[i + 1]) })) };
    ПОГОДА.set(где, итог);
    return итог;
  },
  async 'sys.native.list'(){
    if (!ALLOW_LAUNCH) return { list:[] };
    const каталоги = String(process.env.PATH || '').split(':').filter(Boolean);
    const есть = имя => каталоги.some(к => existsSync(join(к, имя)));
    return { list:Object.keys(НАТИВНЫЕ).filter(id => есть(НАТИВНЫЕ[id])) };
  },
  async 'sys.native'({ программа, путь, раздел, найти }){
    if (!ALLOW_LAUNCH) throw new Error('запуск программ выключен: запустите агент с ключом --allow-launch');
    const имя = НАТИВНЫЕ[программа];
    if (!имя) throw new Error('нет такой программы: ' + программа);
    /* Параметрам — раздел, который открыть: только латиница, иначе это
       уже не раздел, а что-то подсунутое */
    const доводы = путь ? [safe(путь)]
      : программа === 'files' && раздел === 'trash' ? ['--trash']
      : раздел && /^[a-z0-9]{1,20}$/.test(String(раздел)) ? [String(раздел)] : [];
    /* Магазину — что искать: те же знаки, что пропускает сам поиск */
    if (найти && /^[\w+.\- а-яё]{2,60}$/i.test(String(найти))) доводы.push('--find=' + String(найти));
    const { spawn } = await import('node:child_process');
    /* Агент поднимается раньше оконного сервера — окружение экрана берём
       у системного слоя, иначе окно не увидит экрана */
    const env = { ...(SYSTEM ? await (await import('./system.mjs')).средаЭкрана() : process.env), GLOWER_ROOT:ROOT };
    await new Promise((готово, беда) => {
      const п = spawn(имя, доводы, { detached:true, stdio:'ignore', env });
      п.once('error', беда);
      п.once('spawn', готово);
      п.unref();
    });
    return { ok:true, программа:имя };
  },
  ...SYS,
  /* Программа Windows из Проводника: путь у него свой, от корня файлов
     агента, а системному слою нужен настоящий. */
  async 'sys.windows.open'({ path }){
    if (!SYS['sys.windows.run']) throw new Error('системный слой выключен: запустите агент с ключом --system');
    return SYS['sys.windows.run']({ 'путь':safe(path) });
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

  /* Обои системы отдаём отдельной дорожкой.

     Картинка на пять мегабайт, пересказанная строкой base64 через шину,
     стоит вдвое дороже и не кешируется вовсе: на слабой машине страница
     от этого встаёт. А так её отдаёт обычный веб-сервер и берёт на себя
     браузер. Наружу уходит только то, что лежит в папках обоев: путь
     проверяется тем же списком, по которому их и нашли. */
  const дорожка = (() => {
    try { return decodeURIComponent(req.url.split('?')[0]); } catch(e){ return req.url.split('?')[0]; }
  })();
  if (дорожка === '/обои'){
    /* Имя довода латиницей — «p», и это не про вкус. Всё, что попадает в
       строку запроса, должно быть закодировано, иначе разбор запроса
       отказывает ещё до нас: сервер отвечает «400» на кириллицу в сыром
       виде, и понять по такому ответу нечего. Путь кодирует страница,
       а имя довода пусть будет тем, что можно набрать руками. */
    const спрос = new URLSearchParams(req.url.split('?')[1] || '');
    const путь = спрос.get('p') || '';
    if (!SYSTEM) return send(res, 403, 'системный слой выключен');
    const m = await import('./system.mjs');
    if (!m.обоиМожно(путь) || путь.includes('..')) return send(res, 403, 'не обои');
    try {
      const данные = await readFile(путь);
      const ext = extname(путь).slice(1).toLowerCase();
      return send(res, 200, данные, {
        'Content-Type':(MIME[ext] || 'application/octet-stream'),
        /* Обои не меняются — пусть браузер держит их у себя. */
        'Cache-Control':'public, max-age=86400' });
    } catch(e){ return send(res, 404, 'нет такого файла'); }
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
  /* Встречающего файла на столе больше нет. Он появился, когда надо было
     показать, что диск настоящий, — а теперь это и так очевидно, и первое,
     что видит человек на своём рабочем столе, не должно быть нашей
     запиской. Пустой стол честнее: пусть кладёт своё. */
}

startWatch();

/* ---------- пробуждение машины ----------

   О том, что машина спала, не знал никто. Со стороны это выглядело так:
   открыл крышку — и система будто не сразу вернулась. Часть беды была в
   связях шины: они остаются открытыми в мёртвый сокет, и браузер узнаёт об
   этом через минуты. Эту часть лечит срок ожидания на той стороне. Вторая
   часть — здесь: сказать вслух, что был сон, чтобы каждый освежил своё.

   Узнаём по разрыву во времени: таймер заведён на секунду, а прошло
   полминуты — значит, между двумя тиками машина спала. Способ грубый, но
   верный на любой машине и не требует ни logind, ни прав. */
{
  const ШАГ = 1000, ПОРОГ = 25000;
  let прежде = Date.now();
  setInterval(async () => {
    const теперь = Date.now();
    const разрыв = теперь - прежде - ШАГ;
    прежде = теперь;
    if (разрыв < ПОРОГ) return;
    const спали = Math.round(разрыв / 1000);
    console.log(`  машина просыпается: сна было около ${спали} с`);
    try { await BUS['ui.say']({ 'тема':'пробуждение', что:{ 'сколько':спали } }); } catch(e){}

    /* Запирать или нет — решает оболочка, а не мы: там лежат настройки
       человека и там же известно, задан ли у него пароль. Наше дело —
       сказать, что был сон. */
  }, ШАГ);
}

/* Слушаем только себя. Агент делает на машине почти всё — ставит программы,
   выключает питание, заводит людей, — и открывать его наружу нельзя ни на
   минуту. Раньше он слушал все сети сразу: в домашней сети до него мог
   дотянуться любой сосед по Wi-Fi, и защищал нас только брандмауэр. */
server.listen(PORT, '127.0.0.1', () => {
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
