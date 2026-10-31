/* ==========================================================================
   Сейф двух половин

   Папка с файлами, которую можно открыть, только когда вместе ключ Glower и
   этот компьютер. Шифр собирается из двух половин по 256 бит: одна лежит
   на ключе (.glower-key/safe-<компьютер>.half), другая — на компьютере
   (~/.config/glower/safe/<ключ>.half). Из них HKDF-SHA256 выводит ключ
   AES-256-GCM. Украдут флешку — на ней нет ни файлов, ни второй половины;
   украдут компьютер — нет первой.

   Зашифрованные файлы лежат на компьютере (~/.local/share/glower/safe/
   <ключ>/): каждый файл — отдельный кусок со случайным именем, а какие
   были имена и папки, знает только зашифрованная опись.

   Открытый сейф расшифровывается в оперативную память ($XDG_RUNTIME_DIR —
   это tmpfs): на диск открытые файлы не попадают никогда. Изменения
   шифруются обратно каждые полминуты и при закрытии. Вынули ключ — сейф
   сам сохраняется, открытые копии стираются, шифр забывается.

   Файл куска: «GLSAFE1» · 12 байт IV · шифротекст · 16 байт метки GCM.
   ========================================================================== */
import { createCipheriv, createDecipheriv, randomBytes, hkdfSync } from 'node:crypto';
import { createReadStream, createWriteStream, existsSync } from 'node:fs';
import { readFile, writeFile, mkdir, readdir, stat, rm, rename, utimes, open as открой_файл } from 'node:fs/promises';
import { join, dirname, relative, sep } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { once } from 'node:events';
import os from 'node:os';
import { мои, носители, подключи, ключ_на, ПАПКА, тайна_подходит, нужен_токен } from './key.mjs';

const МЕТКА = Buffer.from('GLSAFE1');
const КОНФИГ = () => join(process.env.XDG_CONFIG_HOME || join(os.homedir(), '.config'), 'glower');
const ДАННЫЕ = () => join(process.env.XDG_DATA_HOME || join(os.homedir(), '.local/share'), 'glower', 'safe');
const ОТКРЫТЫЙ = () => join(process.env.XDG_RUNTIME_DIR || join(os.tmpdir(), 'glower-' + (process.getuid ? process.getuid() : 0)), 'glower-safe');
const ИМЯ_ПОЛОВИНЫ = () => 'safe-' + (os.hostname() || 'pc').replace(/[^\w.-]/g, '_') + '.half';
const ID = /^[0-9a-f-]{36}$/;

/* Ключ шифра из двух половин */
export function шифр_из(половина_ключа, половина_пк){
  return Buffer.from(hkdfSync('sha256', Buffer.concat([половина_ключа, половина_пк]), Buffer.from('glower-safe-v1'), Buffer.from('aes-256-gcm'), 32));
}

/* Небольшое — целиком в памяти: опись */
export function зашифруй(шифр, данные){
  const iv = randomBytes(12);
  const c = createCipheriv('aes-256-gcm', шифр, iv);
  const тело = Buffer.concat([c.update(данные), c.final()]);
  return Buffer.concat([МЕТКА, iv, тело, c.getAuthTag()]);
}
export function расшифруй(шифр, кусок){
  if (кусок.length < МЕТКА.length + 28 || !кусок.subarray(0, МЕТКА.length).equals(МЕТКА)) throw new Error('это не кусок сейфа');
  const iv = кусок.subarray(МЕТКА.length, МЕТКА.length + 12);
  const метка = кусок.subarray(кусок.length - 16);
  const d = createDecipheriv('aes-256-gcm', шифр, iv);
  d.setAuthTag(метка);
  return Buffer.concat([d.update(кусок.subarray(МЕТКА.length + 12, кусок.length - 16)), d.final()]);
}

/* Большое — потоком, чтобы фильм в сейфе не съел всю память */
async function зашифруй_файл(шифр, откуда, куда){
  const iv = randomBytes(12);
  const c = createCipheriv('aes-256-gcm', шифр, iv);
  const выход = createWriteStream(куда + '.new', { mode:0o600 });
  const пиши = async кусок => { if (кусок.length && !выход.write(кусок)) await once(выход, 'drain'); };
  try {
    await пиши(Buffer.concat([МЕТКА, iv]));
    for await (const ч of createReadStream(откуда)) await пиши(c.update(ч));
    await пиши(c.final());
    await пиши(c.getAuthTag());
    await new Promise((готово, беда) => { выход.on('error', беда); выход.end(готово); });
  } catch(e){ выход.destroy(); await rm(куда + '.new', { force:true }); throw e; }
  await rename(куда + '.new', куда);
}
async function расшифруй_файл(шифр, откуда, куда){
  const р = (await stat(откуда)).size;
  if (р < МЕТКА.length + 28) throw new Error('кусок сейфа повреждён');
  const ф = await открой_файл(откуда, 'r');
  const голова = Buffer.alloc(МЕТКА.length + 12), метка = Buffer.alloc(16);
  await ф.read(голова, 0, голова.length, 0);
  await ф.read(метка, 0, 16, р - 16);
  await ф.close();
  if (!голова.subarray(0, МЕТКА.length).equals(МЕТКА)) throw new Error('это не кусок сейфа');
  const d = createDecipheriv('aes-256-gcm', шифр, голова.subarray(МЕТКА.length));
  d.setAuthTag(метка);
  await pipeline(createReadStream(откуда, { start:голова.length, end:р - 17 }), d, createWriteStream(куда, { mode:0o600 }));
}

async function обойди(корень){
  const файлы = [], папки = [];
  const иди = async п => {
    let имена = [];
    try { имена = await readdir(п, { withFileTypes:true }); } catch(e){ return; }
    for (const и of имена){
      const полный = join(п, и.name);
      const отн = relative(корень, полный).split(sep).join('/');
      if (и.isDirectory()){ папки.push(отн); await иди(полный); }
      else if (и.isFile()){ const с = await stat(полный); файлы.push({ путь:отн, размер:с.size, время:Math.round(с.mtimeMs) }); }
    }
  };
  await иди(корень);
  return { файлы, папки };
}

/* Путь из описи — только внутрь сейфа */
function внутри(корень, отн){
  const п = join(корень, ...String(отн).split('/').filter(ч => ч && ч !== '.' && ч !== '..'));
  if (п !== корень && !п.startsWith(корень + sep)) throw new Error('странный путь в описи сейфа');
  return п;
}

export function safe({ allowLaunch } = {}){
  let открыт = null;   // { id, шифр, опись, следит }

  const папка_сейфа = id => join(ДАННЫЕ(), id);

  /* Свой ключ, вставленный сейчас, с половиной для этого компьютера */
  async function найди_ключ(id){
    const список = await мои();
    for (const н of await носители()){
      н.где = await подключи(н);
      const к = await ключ_на(н.где);
      if (!к || (id && к.id !== id)) continue;
      const м = список.find(м => м.id === к.id && тайна_подходит(к.секрет, м.хэш));
      if (м) return { н, к, м };
    }
    return null;
  }

  async function прочти_опись(id, шифр){
    return JSON.parse(расшифруй(шифр, await readFile(join(папка_сейфа(id), 'index.bin'))).toString('utf8'));
  }
  async function запиши_опись(id, шифр, опись){
    const путь = join(папка_сейфа(id), 'index.bin');
    await writeFile(путь + '.new', зашифруй(шифр, Buffer.from(JSON.stringify(опись))), { mode:0o600 });
    await rename(путь + '.new', путь);
  }

  /* Открытые файлы → куски. Только новое и изменённое; пропавшее —
     убираем и из описи, и с диска */
  async function сохрани(){
    if (!открыт) return { изменено:0 };
    const { id, шифр, опись } = открыт;
    const куски = join(папка_сейфа(id), 'blobs');
    const { файлы, папки } = await обойди(ОТКРЫТЫЙ());
    let изменено = 0;
    const было = опись.файлы;
    const стало = {};
    for (const ф of файлы){
      const старое = было[ф.путь];
      if (старое && старое.размер === ф.размер && старое.время === ф.время){ стало[ф.путь] = старое; continue; }
      const имя = randomBytes(16).toString('hex');
      await зашифруй_файл(шифр, join(ОТКРЫТЫЙ(), ...ф.путь.split('/')), join(куски, имя));
      стало[ф.путь] = { кусок:имя, размер:ф.размер, время:ф.время };
      if (старое) await rm(join(куски, старое.кусок), { force:true });
      изменено++;
    }
    for (const [путь, з] of Object.entries(было)){
      if (!стало[путь]){ await rm(join(куски, з.кусок), { force:true }); изменено++; }
    }
    опись.файлы = стало;
    опись.папки = папки;
    опись.изменён = new Date().toISOString();
    await запиши_опись(id, шифр, опись);
    return { изменено };
  }

  async function закрой(почему = 'закрыт'){
    if (!открыт) return { ok:true };
    clearInterval(открыт.следит);
    let ошибка = null;
    try { await сохрани(); } catch(e){ ошибка = e.message; }
    // Сохранить не вышло — открытые копии не трогаем: стереть их значило бы
    // потерять последние правки. Сейф остаётся открытым до следующей попытки
    if (ошибка){ открыт.следит = setInterval(сторож, 2000); throw new Error('сейф не сохранился: ' + ошибка); }
    await rm(ОТКРЫТЫЙ(), { recursive:true, force:true });
    открыт.шифр.fill(0);
    открыт = null;
    последнее = почему;
    return { ok:true };
  }

  let последнее = null;
  let тик = 0;
  /* Пока сейф открыт: ключ вынули — закрываем; раз в полминуты сохраняем */
  async function сторож(){
    if (!открыт || открыт.занят) return;
    открыт.занят = true;
    try {
      const есть = await найди_ключ(открыт.id);
      if (!есть){ await закрой('ключ вынут').catch(() => {}); return; }
      if (++тик % 15 === 0) await сохрани().catch(() => {});
    } finally { if (открыт) открыт.занят = false; }
  }

  return {
    async 'safe.state'(){
      const список = await мои();
      const сейфы = [];
      for (const м of список){
        if (!ID.test(м.id) || !existsSync(join(папка_сейфа(м.id), 'index.bin'))) continue;
        let размер = 0, кусков = 0;
        try { for (const ф of await readdir(join(папка_сейфа(м.id), 'blobs'))){ кусков++; размер += (await stat(join(папка_сейфа(м.id), 'blobs', ф))).size; } } catch(e){}
        сейфы.push({ id:м.id, имя:м.имя, кусков, размер });
      }
      return { сейфы, открыт:открыт ? { id:открыт.id, где:ОТКРЫТЫЙ() } : null, последнее, можно:!!allowLaunch };
    },

    /* Завести сейф для ключа: половина — на ключ, половина — сюда */
    async 'safe.create'({ id } = {}){
      if (!ID.test(String(id))) throw new Error('непонятный ключ');
      const есть = await найди_ключ(id);
      if (!есть) throw new Error('вставьте этот ключ — без него сейф не завести');
      if (existsSync(join(папка_сейфа(id), 'index.bin'))) throw new Error('сейф для этого ключа уже есть');
      const на_ключе = randomBytes(32), здесь = randomBytes(32);
      await writeFile(join(есть.н.где, ПАПКА, ИМЯ_ПОЛОВИНЫ()), на_ключе.toString('hex'));
      await mkdir(join(КОНФИГ(), 'safe'), { recursive:true, mode:0o700 });
      await writeFile(join(КОНФИГ(), 'safe', id + '.half'), здесь.toString('hex'), { mode:0o600 });
      await mkdir(join(папка_сейфа(id), 'blobs'), { recursive:true, mode:0o700 });
      const шифр = шифр_из(на_ключе, здесь);
      await запиши_опись(id, шифр, { формат:1, создан:new Date().toISOString(), файлы:{}, папки:[] });
      шифр.fill(0);
      return { ok:true };
    },

    /* Открыть: обе половины на месте — расшифровываем в память */
    async 'safe.open'({ id, токен } = {}){
      нужен_токен(токен, id);
      if (открыт){
        if (открыт.id === id) return { ok:true, где:ОТКРЫТЫЙ(), уже:true };
        await закрой();
      }
      if (!ID.test(String(id))) throw new Error('непонятный ключ');
      const есть = await найди_ключ(id);
      if (!есть) throw new Error('вставьте ключ — сейф открывается только вместе с ним');
      let половина_ключа, половина_пк;
      try { половина_ключа = Buffer.from((await readFile(join(есть.н.где, ПАПКА, ИМЯ_ПОЛОВИНЫ()), 'utf8')).trim(), 'hex'); }
      catch(e){ throw new Error('на ключе нет половины шифра для этого компьютера'); }
      try { половина_пк = Buffer.from((await readFile(join(КОНФИГ(), 'safe', id + '.half'), 'utf8')).trim(), 'hex'); }
      catch(e){ throw new Error('на этом компьютере нет второй половины шифра — сейф заводили не здесь'); }
      const шифр = шифр_из(половина_ключа, половина_пк);
      let опись;
      try { опись = await прочти_опись(id, шифр); }
      catch(e){ шифр.fill(0); throw new Error('половины не сходятся — сейф этим ключом не открыть'); }
      await rm(ОТКРЫТЫЙ(), { recursive:true, force:true });
      await mkdir(ОТКРЫТЫЙ(), { recursive:true, mode:0o700 });
      for (const п of опись.папки || []) await mkdir(внутри(ОТКРЫТЫЙ(), п), { recursive:true });
      for (const [путь, з] of Object.entries(опись.файлы || {})){
        const куда = внутри(ОТКРЫТЫЙ(), путь);
        await mkdir(dirname(куда), { recursive:true });
        await расшифруй_файл(шифр, join(папка_сейфа(id), 'blobs', з.кусок), куда);
        await utimes(куда, з.время / 1000, з.время / 1000);
      }
      открыт = { id, шифр, опись, занят:false };
      открыт.следит = setInterval(сторож, 2000);
      if (открыт.следит.unref) открыт.следит.unref();
      последнее = null;
      return { ok:true, где:ОТКРЫТЫЙ(), файлов:Object.keys(опись.файлы || {}).length };
    },

    async 'safe.close'(){ return закрой(); },
    async 'safe.save'(){ return сохрани(); },

    /* Удалить сейф совсем — вместе с файлами в нём */
    async 'safe.delete'({ id, токен } = {}){
      нужен_токен(токен, id);
      if (!ID.test(String(id))) throw new Error('непонятный ключ');
      if (открыт && открыт.id === id){ clearInterval(открыт.следит); открыт.шифр.fill(0); открыт = null; await rm(ОТКРЫТЫЙ(), { recursive:true, force:true }); }
      await rm(папка_сейфа(id), { recursive:true, force:true });
      await rm(join(КОНФИГ(), 'safe', id + '.half'), { force:true });
      return { ok:true };
    },
  };
}
