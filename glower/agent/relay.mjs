/* ==========================================================================
   Эстафета — продолжить работу на другом компьютере с того же места

   Пока свой ключ Glower вставлен и эстафета для него включена, агент раз в
   полминуты записывает на ключ (.glower-key/relay/), что сейчас открыто:
     — программы — по окнам на экране;
     — файлы, открытые в программах, — их копии (только из домашней папки,
       не больше 15 штук и 100 МБ каждый; из открытого сейфа — никогда);
     — вкладки Firefox — из его же файла сеанса;
     — последние записи буфера обмена (история Win+V).
   Пишет, только когда что-то поменялось, — флешку зря не изнашиваем.

   Ключ вставили в другой компьютер с GlowerOS, где он тоже свой, — панель
   предлагает «Продолжить с места»: файлы ложатся в «Документы/Эстафета» и
   открываются, вкладки открываются в браузере, буфер возвращается,
   программы запускаются.
   ========================================================================== */
import { execFile, spawn } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir, stat, copyFile, rm, readlink, rename } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, basename, extname, isAbsolute, resolve, sep } from 'node:path';
import { promisify } from 'node:util';
import { createHash } from 'node:crypto';
import os from 'node:os';
import { мои, носители, подключи, ключ_на, ПАПКА, тайна_подходит, нужен_токен } from './key.mjs';

const run = promisify(execFile);
const КОНФИГ = () => join(process.env.XDG_CONFIG_HOME || join(os.homedir(), '.config'), 'glower');
const ВИДЕЛИ = () => join(КОНФИГ(), 'relay-seen.json');
const БУФЕР = () => join(process.env.XDG_RUNTIME_DIR || '/run/user/' + (process.getuid ? process.getuid() : 1000), 'glower-clip');
const ФАЙЛОВ = 15, РАЗМЕР = 100 * 1024 * 1024;

/* Оболочка и служебное — не программы человека */
const НЕ_ПРОГРАММЫ = /^(glowershell|glower-panel|glower-lock|glower-login|glower-installer|xdg-desktop-portal.*|org\.freedesktop\..*)$/i;
const БРАУЗЕРЫ = /firefox|chrom|brave|vivaldi|opera|yandex|edge|librewolf|zen/i;

/* ---------- Firefox: сеанс лежит в mozlz4 — LZ4-блок с заголовком ---------- */
export function lz4_блок(вход, размер){
  const выход = Buffer.alloc(размер);
  let i = 0, o = 0;
  while (i < вход.length){
    const метка = вход[i++];
    let длина = метка >> 4;
    if (длина === 15){ let б; do { б = вход[i++]; длина += б; } while (б === 255); }
    вход.copy(выход, o, i, i + длина); i += длина; o += длина;
    if (i >= вход.length) break;
    const сдвиг = вход[i] | (вход[i + 1] << 8); i += 2;
    let совпадение = метка & 15;
    if (совпадение === 15){ let б; do { б = вход[i++]; совпадение += б; } while (б === 255); }
    совпадение += 4;
    if (!сдвиг || сдвиг > o) throw new Error('lz4: неверный сдвиг');
    for (let к = 0; к < совпадение; к++, o++) выход[o] = выход[o - сдвиг];
  }
  return выход.subarray(0, o);
}

export function mozlz4(данные){
  if (данные.length < 12 || данные.subarray(0, 8).toString('latin1') !== 'mozLz40\0') throw new Error('не mozlz4');
  return lz4_блок(данные.subarray(12), данные.readUInt32LE(8));
}

/* Открытые вкладки из сеанса Firefox: окна → вкладки → текущая запись */
export function вкладки_firefox(сеанс){
  const итог = [];
  for (const окно of сеанс.windows || []){
    for (const вкладка of окно.tabs || []){
      const записи = вкладка.entries || [];
      const з = записи[Math.max(0, Math.min(записи.length, вкладка.index || записи.length) - 1)];
      if (з && /^https?:\/\//.test(з.url || '') && !итог.some(в => в.url === з.url))
        итог.push({ url:з.url, title:String(з.title || '').slice(0, 200) });
    }
  }
  return итог.slice(0, 60);
}

async function файл_сеанса_firefox(){
  const дом = os.homedir();
  const корни = [join(дом, '.mozilla/firefox'), join(дом, 'snap/firefox/common/.mozilla/firefox'), join(дом, '.var/app/org.mozilla.firefox/.mozilla/firefox')];
  let лучший = null;
  for (const к of корни){
    let профили = [];
    try { профили = await readdir(к); } catch(e){ continue; }
    for (const п of профили){
      const ф = join(к, п, 'sessionstore-backups', 'recovery.jsonlz4');
      try { const с = await stat(ф); if (!лучший || с.mtimeMs > лучший.время) лучший = { путь:ф, время:с.mtimeMs }; } catch(e){}
    }
  }
  return лучший && Date.now() - лучший.время < 10 * 60 * 1000 ? лучший.путь : null;   // Firefox закрыт давно — вкладок нет
}

/* ---------- Окна и файлы ---------- */
async function окна(env){
  try {
    const { stdout } = await run('glower-toplevels', [], { env, timeout:4000 });
    return JSON.parse(stdout).filter(о => о.appId && !НЕ_ПРОГРАММЫ.test(о.appId));
  } catch(e){ return []; }
}

/* Ярлык программы по её имени окна: имя.desktop или StartupWMClass */
async function ярлык_для(appId){
  const папки = [join(os.homedir(), '.local/share/applications'), '/usr/share/applications', '/var/lib/flatpak/exports/share/applications',
    join(os.homedir(), '.local/share/flatpak/exports/share/applications')];
  const нижн = appId.toLowerCase();
  for (const п of папки){
    if (existsSync(join(п, appId + '.desktop'))) return appId + '.desktop';
  }
  for (const п of папки){
    let имена = [];
    try { имена = (await readdir(п)).filter(и => и.endsWith('.desktop')); } catch(e){ continue; }
    for (const и of имена){
      if (и.toLowerCase() === нижн + '.desktop') return и;
      try {
        const т = await readFile(join(п, и), 'utf8');
        const к = (т.match(/^StartupWMClass=(.*)$/m) || [])[1];
        if (к && к.trim().toLowerCase() === нижн) return и;
      } catch(e){}
    }
  }
  return null;
}

/* Файлы, которые сейчас открыты в программах: из командных строк процессов
   человека — так их получают и Блокнот, и LibreOffice, и проигрыватели */
export function файлы_из_строки(доводы, рабочая, дом, нельзя){
  const итог = [];
  for (const д of доводы.slice(1)){
    if (!д || д.startsWith('-') || /^[a-z]+:\/\//i.test(д)) continue;
    const путь = isAbsolute(д) ? д : resolve(рабочая || дом, д);
    if (!путь.startsWith(дом + sep)) continue;
    const внутри = путь.slice(дом.length + 1);
    if (внутри.split(sep).some(ч => ч.startsWith('.'))) continue;   // настройки, кэш, .glower-key
    if (нельзя.some(н => путь === н || путь.startsWith(н + sep))) continue;
    итог.push(путь);
  }
  return итог;
}

async function открытые_файлы(нельзя){
  const дом = os.homedir(), моё = process.getuid ? process.getuid() : -1;
  const итог = [];
  let процессы = [];
  try { процессы = (await readdir('/proc')).filter(п => /^\d+$/.test(п)); } catch(e){ return []; }
  for (const pid of процессы){
    try {
      if ((await stat('/proc/' + pid)).uid !== моё) continue;
      const доводы = (await readFile(`/proc/${pid}/cmdline`)).toString('utf8').split('\0').filter(Boolean);
      if (!доводы.length || /node$|server\.mjs|glower-panel|\/sh$|bash$/.test(доводы[0])) continue;
      const рабочая = await readlink(`/proc/${pid}/cwd`).catch(() => дом);
      for (const путь of файлы_из_строки(доводы, рабочая, дом, нельзя)){
        if (итог.some(ф => ф.путь === путь)) continue;
        try {
          const с = await stat(путь);
          if (!с.isFile() || с.size > РАЗМЕР || /\.(desktop|sh|py|mjs|js|exe)$/i.test(путь)) continue;
          итог.push({ путь, размер:с.size, время:Math.round(с.mtimeMs), программа:basename(доводы[0]) });
        } catch(e){}
        if (итог.length >= ФАЙЛОВ) return итог;
      }
    } catch(e){}
  }
  return итог;
}

export function relay({ root, allowLaunch, средаЭкрана } = {}){
  const ДОКУМЕНТЫ = join(root || os.homedir(), 'Документы');
  let пишем = false, прошлое = new Map();   // id ключа → отпечаток записанного

  /* Что сейчас открыто — для записи на ключ */
  async function снимок(где){
    const env = средаЭкрана ? await средаЭкрана() : process.env;
    const нельзя = [где, join(env.XDG_RUNTIME_DIR || '', 'glower-safe')].filter(Boolean);
    const [о, ф] = await Promise.all([окна(env), открытые_файлы(нельзя)]);
    const программы = [];
    for (const w of о){
      if (программы.some(п => п.appId === w.appId)) continue;
      программы.push({ appId:w.appId, title:String(w.title || '').slice(0, 120), ярлык:await ярлык_для(w.appId) });
    }
    let вкладки = [];
    const сеанс = await файл_сеанса_firefox();
    if (сеанс){ try { вкладки = вкладки_firefox(JSON.parse(mozlz4(await readFile(сеанс)).toString('utf8'))); } catch(e){} }
    let буфер = [];
    try {
      буфер = (await readdir(БУФЕР())).filter(и => /^[\w.-]+\.(txt|png)$/.test(и)).sort().reverse().slice(0, 10);
    } catch(e){}
    return { программы, файлы:ф, вкладки, буфер };
  }

  /* Записать эстафету на ключ, если что-то поменялось */
  async function запиши(н, к){
    const с = await снимок(н.где);
    const папка = join(н.где, ПАПКА, 'relay');
    const отпечаток = createHash('sha256').update(JSON.stringify({ п:с.программы.map(п => п.appId), ф:с.файлы.map(ф => [ф.путь, ф.размер, ф.время]), в:с.вкладки.map(в => в.url), б:с.буфер })).digest('hex');
    if (прошлое.get(к.id) === отпечаток) return { изменено:false };
    await mkdir(join(папка, 'files'), { recursive:true });
    await mkdir(join(папка, 'clipboard'), { recursive:true });
    // Файлы: имя на ключе — по порядку, настоящее имя — в описи
    const файлы = [];
    let н_ = 0;
    for (const ф of с.файлы){
      const имя = String(++н_).padStart(2, '0') + (extname(ф.путь) || '').replace(/[^.\w]/g, '').slice(0, 10);
      try { await copyFile(ф.путь, join(папка, 'files', имя)); файлы.push({ имя:basename(ф.путь), файл:имя, программа:ф.программа }); } catch(e){}
    }
    for (const и of await readdir(join(папка, 'files')).catch(() => []))
      if (!файлы.some(ф => ф.файл === и)) await rm(join(папка, 'files', и), { force:true });
    for (const и of await readdir(join(папка, 'clipboard')).catch(() => [])) await rm(join(папка, 'clipboard', и), { force:true });
    for (const и of с.буфер) await copyFile(join(БУФЕР(), и), join(папка, 'clipboard', и)).catch(() => {});
    const опись = { формат:1, компьютер:os.hostname(), когда:new Date().toISOString(),
      программы:с.программы, файлы, вкладки:с.вкладки, буфер:с.буфер };
    await writeFile(join(папка, 'relay.json.new'), JSON.stringify(опись, null, 1));
    await rename(join(папка, 'relay.json.new'), join(папка, 'relay.json'));
    await run('sync', [], { timeout:30000 }).catch(() => {});
    прошлое.set(к.id, отпечаток);
    return { изменено:true };
  }

  /* Свои вставленные ключи: { н, к, м } */
  async function свои(){
    const список = await мои();
    const итог = [];
    if (!список.length) return итог;
    for (const н of await носители()){
      н.где = await подключи(н);
      const к = await ключ_на(н.где);
      const м = к && список.find(м => м.id === к.id && тайна_подходит(к.секрет, м.хэш));
      if (м && !н.толькоЧтение) итог.push({ н, к, м });
    }
    return итог;
  }

  // Раз в полминуты — эстафета на все свои ключи, где она включена
  const таймер = setInterval(async () => {
    if (пишем) return;
    пишем = true;
    try { for (const { н, к, м } of await свои()) if (м.эстафета) await запиши(н, к).catch(() => {}); }
    finally { пишем = false; }
  }, 30000);
  if (таймер.unref) таймер.unref();

  async function виделиЛи(){ try { return JSON.parse(await readFile(ВИДЕЛИ(), 'utf8')); } catch(e){ return {}; } }

  return {
    /* Есть ли на ключе эстафета с другого компьютера, которую здесь ещё не продолжали */
    async 'relay.state'({ dev } = {}){
      const найдено = (await свои()).find(с => !dev || с.н.dev === dev);
      if (!найдено) return { есть:false };
      let опись;
      try { опись = JSON.parse(await readFile(join(найдено.н.где, ПАПКА, 'relay', 'relay.json'), 'utf8')); }
      catch(e){ return { есть:false, ключ:найдено.к.id }; }
      const видели = (await виделиЛи())[найдено.к.id];
      const чужая = опись.компьютер !== os.hostname();
      return { есть:true, ключ:найдено.к.id, dev:найдено.н.dev, компьютер:опись.компьютер, когда:опись.когда,
        новая:чужая && видели !== опись.когда,
        программ:(опись.программы || []).length, файлов:(опись.файлы || []).length,
        вкладок:(опись.вкладки || []).length, буфер:(опись.буфер || []).length };
    },

    /* Записать прямо сейчас — «Передать эстафету» */
    async 'relay.capture'({ dev } = {}){
      const найдено = (await свои()).find(с => с.н.dev === dev);
      if (!найдено) throw new Error('вставьте свой ключ Glower');
      прошлое.delete(найдено.к.id);
      await запиши(найдено.н, найдено.к);
      return { ok:true };
    },

    /* Продолжить: файлы — в «Документы/Эстафета» и открыть, вкладки — в
       браузер, буфер — назад в историю. Программы запускает панель: ей
       известно, что на этом компьютере установлено */
    async 'relay.continue'({ dev, токен } = {}){
      if (!allowLaunch) throw new Error('запуск программ выключен');
      const найдено = (await свои()).find(с => с.н.dev === dev);
      if (!найдено) throw new Error('вставьте свой ключ Glower');
      нужен_токен(токен, найдено.к.id);
      const папка = join(найдено.н.где, ПАПКА, 'relay');
      const опись = JSON.parse(await readFile(join(папка, 'relay.json'), 'utf8'));
      const env = средаЭкрана ? await средаЭкрана() : process.env;
      // Нет такой программы — spawn сообщает об этом событием, и без
      // обработчика оно роняет весь агент
      const открой = (что, доводы) => { try { const п = spawn(что, доводы, { env, detached:true, stdio:'ignore' }); п.on('error', () => {}); п.unref(); } catch(e){} };

      const куда = join(ДОКУМЕНТЫ, 'Эстафета');
      await mkdir(куда, { recursive:true });
      const открыто = [];
      for (const ф of опись.файлы || []){
        const имя = basename(String(ф.имя || '')).replace(/^\.+/, '') || ф.файл;
        if (!/^[\w.-]+$/.test(String(ф.файл))) continue;
        let цель = join(куда, имя);
        // Такой файл уже есть и он другой — рядом, с пометкой компьютера
        if (existsSync(цель)) цель = join(куда, basename(имя, extname(имя)) + ' (' + String(опись.компьютер || 'ключ').replace(/[^\w.-]/g, '_') + ')' + extname(имя));
        try { await copyFile(join(папка, 'files', ф.файл), цель); открыто.push(цель); открой('xdg-open', [цель]); } catch(e){}
      }
      const адреса = (опись.вкладки || []).map(в => String(в.url)).filter(у => /^https?:\/\/[^\s]+$/.test(у)).slice(0, 60);
      if (адреса.length){
        let браузер = '';
        try { браузер = (await run('xdg-settings', ['get', 'default-web-browser'], { env, timeout:5000 })).stdout.trim(); } catch(e){}
        if (/firefox/i.test(браузер) || (!браузер && existsSync('/usr/bin/firefox'))) открой('firefox', ['--new-window', ...адреса]);
        else for (const у of адреса) открой('xdg-open', [у]);
      }
      // Буфер: записи — в историю Win+V, самую свежую — сразу в буфер
      await mkdir(БУФЕР(), { recursive:true });
      for (const и of опись.буфер || []){
        if (!/^[\w.-]+\.(txt|png)$/.test(и)) continue;
        await copyFile(join(папка, 'clipboard', и), join(БУФЕР(), и)).catch(() => {});
      }
      const свежая = (опись.буфер || [])[0];
      if (свежая && /\.txt$/.test(свежая)){
        try {
          const п = spawn('wl-copy', [], { env, stdio:['pipe', 'ignore', 'ignore'], detached:true });
          п.on('error', () => {}); п.stdin.on('error', () => {});
          п.stdin.end(await readFile(join(папка, 'clipboard', свежая)));
          п.unref();
        } catch(e){}
      }
      const видели = await виделиЛи();
      видели[найдено.к.id] = опись.когда;
      await mkdir(КОНФИГ(), { recursive:true });
      await writeFile(ВИДЕЛИ(), JSON.stringify(видели));
      // Программы, у которых здесь есть ярлык, и не браузер — вкладки его уже открыли
      // …и не та, что сама откроется вместе со своим файлом
      const с_файлами = (опись.файлы || []).map(ф => String(ф.программа || '').toLowerCase()).filter(Boolean);
      const программы = (опись.программы || []).filter(п => п.ярлык && /^[^/\\]+\.desktop$/.test(п.ярлык) && !(адреса.length && БРАУЗЕРЫ.test(п.appId))
          && !(открыто.length && с_файлами.some(ф => String(п.appId).toLowerCase().includes(ф) || ф.includes(String(п.appId).toLowerCase()))))
        .map(п => п.ярлык);
      return { ok:true, файлы:открыто, вкладок:адреса.length, программы, компьютер:опись.компьютер };
    },
  };
}
