/* ==========================================================================
   Ключ Glower — обычная флешка, которая становится личным ключом от GlowerOS

   Флешку не форматируем: на ней появляется папка .glower-key, остальное
   остаётся как было. В папке:
     key.json      — имя ключа и его тайна (256 случайных бит);
     profile.json  — «мой стол»: тема, акцент, обои, док, виджеты, мышь,
                     список программ из Flathub;
     wallpaper.*   — свои обои, если они не из набора системы;
     backup/<компьютер>/ — копия папок человека.

   Компьютер помнит свои ключи в ~/.config/glower/keys.json — не тайну, а
   её отпечаток SHA-256. Что умеет ключ:
     — вход без пароля: замок открывается, когда вставлен свой ключ;
       по желанию — замок сам закрывается, когда ключ вынули;
     — «мой стол с собой»: на любом компьютере с GlowerOS применить свой
       стол и поставить свои программы, а потом вернуть как было;
     — копия файлов: Документы, Рабочий стол, Изображения — на ключ,
       по желанию сама при каждом подключении, и обратно одной кнопкой.
   ========================================================================== */
import { execFile, spawn } from 'node:child_process';
import { readFile, writeFile, mkdir, copyFile, readdir, stat, rm, rename } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, extname } from 'node:path';
import { promisify } from 'node:util';
import { randomBytes, createHash, timingSafeEqual, randomUUID } from 'node:crypto';
import os from 'node:os';

const run = promisify(execFile);
const КОНФИГ = () => join(process.env.XDG_CONFIG_HOME || join(os.homedir(), '.config'), 'glower');
const МОИ_КЛЮЧИ = () => join(КОНФИГ(), 'keys.json');
export const ПАПКА = '.glower-key';

/* Что из настроек — «мой стол». Остальное (часовой пояс, раскладка значков
   на столе, отметка о пройденном мастере) у каждого компьютера своё */
export const СТОЛ = ['accent', 'accentCustom', 'theme', 'transparency', 'wallpaper', 'wallShuffle',
  'dockApps', 'deskWidgets', 'showDeskWidgets', 'showDesktopIcons', 'clock24', 'showSeconds',
  'city', 'desks', 'nightLight', 'focusLen', 'userEmoji'];

/* Что ещё спрашивает мастер первичной настройки. Это едет на ключе, но
   применяется только мастером на свежей системе: на чужом компьютере
   «Применить мой стол» не меняет ни языка, ни имени хозяина */
const ЯЗЫКИ = { 'ru_RU.UTF-8':'ru', 'en_US.UTF-8':'en', 'uk_UA.UTF-8':'uk', 'be_BY.UTF-8':'be', 'kk_KZ.UTF-8':'kk', 'de_DE.UTF-8':'de',
  'fr_FR.UTF-8':'fr', 'es_ES.UTF-8':'es', 'it_IT.UTF-8':'it', 'pt_BR.UTF-8':'pt', 'pl_PL.UTF-8':'pl', 'tr_TR.UTF-8':'tr' };

export function только_систему(с){
  const итог = {};
  if (!с || typeof с !== 'object') return итог;
  if (Object.values(ЯЗЫКИ).includes(с.язык)) итог.язык = с.язык;
  if (Array.isArray(с.раскладки)){
    const р = с.раскладки.map(String).filter(x => /^[a-z]{2,8}$/.test(x)).slice(0, 4);
    if (р.length) итог.раскладки = р;
  }
  if (typeof с.userName === 'string' && с.userName.trim()) итог.userName = с.userName.trim().slice(0, 60);
  if (typeof с.tz === 'string' && /^([A-Za-z_]+(\/[A-Za-z0-9_+-]+){0,2})?$/.test(с.tz)) итог.tz = с.tz;
  if (typeof с.clock24 === 'boolean') итог.clock24 = с.clock24;
  return итог;
}

/* Какие папки копировать: здесь — по именам папок человека, на флешке —
   по-английски (флешку вставят и в чужой компьютер, и в Windows) */
export const ПАПКИ_КОПИИ = [['Документы', 'Documents'], ['Рабочий стол', 'Desktop'], ['Изображения', 'Pictures']];

export const отпечаток = тайна => createHash('sha256').update(String(тайна)).digest('hex');

export function тайна_подходит(тайна, хэш){
  const а = Buffer.from(отпечаток(тайна), 'hex'), б = Buffer.from(String(хэш || ''), 'hex');
  return а.length === б.length && а.length > 0 && timingSafeEqual(а, б);
}

/* Только то, что относится к столу, — и ничего чужого с флешки */
export function только_стол(настройки){
  const итог = {};
  for (const к of СТОЛ) if (настройки && к in настройки) итог[к] = настройки[к];
  return итог;
}

export async function мои(){
  try { const с = JSON.parse(await readFile(МОИ_КЛЮЧИ(), 'utf8')); return Array.isArray(с) ? с : []; }
  catch(e){ return []; }
}
async function запомни(список){
  await mkdir(КОНФИГ(), { recursive:true });
  await writeFile(МОИ_КЛЮЧИ(), JSON.stringify(список, null, 2), { mode:0o600 });
}

/* Съёмные носители с файловой системой — как в «Носителях» */
export async function носители(){
  let данные;
  try {
    данные = JSON.parse((await run('lsblk', ['-J', '-b', '-o', 'PATH,TYPE,FSTYPE,LABEL,MOUNTPOINT,RM,HOTPLUG,SIZE,MODEL,RO'], { timeout:5000 })).stdout);
  } catch(e){ return []; }
  const итог = [];
  const обойди = (узлы, родитель) => {
    for (const у of узлы || []){
      const съёмный = !!(у.rm || у.hotplug) || !!(родитель && родитель.съёмный);
      const з = { dev:у.path, вид:у.type, файловая:у.fstype || '', метка:у.label || '', где:у.mountpoint || '',
        размер:Number(у.size) || 0, модель:(у.model || (родитель && родитель.модель) || '').trim(), съёмный,
        толькоЧтение:у.ro === true || у.ro === '1' };
      if (съёмный && з.файловая && !['swap', 'crypto_LUKS'].includes(з.файловая) && (у.type === 'part' || (у.type === 'disk' && !(у.children || []).length))
          && з.где !== '/' && !з.где.startsWith('/boot')) итог.push(з);
      обойди(у.children, з);
    }
  };
  обойди(данные.blockdevices, null);
  return итог;
}

/* Подключить, если ещё не подключён. Пробуем один раз на носитель — замок
   спрашивает каждую секунду, и монтировать по кругу незачем */
const пробовали = new Set();
export async function подключи(н, заново = false){
  if (н.где) return н.где;
  const метка = н.dev + ':' + н.размер;
  if (!заново && пробовали.has(метка)) return '';
  пробовали.add(метка);
  try {
    const { stdout } = await run('udisksctl', ['mount', '-b', н.dev, '--no-user-interaction'], { timeout:15000 });
    return ((String(stdout).match(/at (\S+)/) || [])[1] || '').replace(/\.$/, '');
  } catch(e){ return ''; }
}

export async function ключ_на(где){
  if (!где) return null;
  try {
    const к = JSON.parse(await readFile(join(где, ПАПКА, 'key.json'), 'utf8'));
    if (!к || typeof к.id !== 'string' || typeof к.секрет !== 'string') return null;
    return к;
  } catch(e){ return null; }
}

async function найди(dev, заново = true){
  const н = (await носители()).find(н => н.dev === dev);
  if (!н) throw new Error('такой флешки система не видит: ' + dev);
  н.где = await подключи(н, заново);
  if (!н.где) throw new Error('флешку не вышло подключить');
  return н;
}

async function свой_ключ(dev){
  const н = await найди(dev);
  const к = await ключ_на(н.где);
  if (!к) throw new Error('на этой флешке нет ключа Glower');
  return { н, к };
}

/* Что лежит в папке: сколько файлов и байт — для копии */
async function объём(путь){
  let файлов = 0, байт = 0;
  const обойди = async п => {
    let имена = [];
    try { имена = await readdir(п, { withFileTypes:true }); } catch(e){ return; }
    for (const и of имена){
      const полный = join(п, и.name);
      if (и.isDirectory()) await обойди(полный);
      else if (и.isFile()){ файлов++; try { байт += (await stat(полный)).size; } catch(e){} }
    }
  };
  await обойди(путь);
  return { файлов, байт };
}

/* «  1,234,567  45%  1.20MB/s  0:00:12» — ход rsync --info=progress2 */
export function ходRsync(строка){
  const м = String(строка).match(/([\d,.]+)\s+(\d{1,3})%\s+([\d.,]+\S*\/s)?\s*(\d+:\d{2}:\d{2})?/);
  return м ? { байт:Number(м[1].replace(/[,.]/g, '')), процент:Math.min(100, Number(м[2])), скорость:м[3] || null } : null;
}

/* Спасательные ключи этого компьютера: отпечатки — в /var/lib/glower,
   их читает меню спасения при загрузке (glower-rescue), ещё до входа */
const СПАСЕНИЕ = '/var/lib/glower/rescue-keys';
async function спасательный(тайна){
  try { return (await readFile(СПАСЕНИЕ, 'utf8')).split('\n').includes(отпечаток(тайна)); } catch(e){ return false; }
}

export function key({ root, allowLaunch } = {}){
  const ДОМ = root || os.homedir();
  let копия = null;   // { dev, куда, процент, шаг, done, ok, error }

  const есть = async cmd => { try { await run('which', [cmd]); return true; } catch(e){ return false; } };

  /* Скопировать папки с ключа или на ключ. rsync — с ходом и без лишнего:
     только новое и изменённое, ничего не удаляет ни там, ни здесь */
  async function перенеси(пары, имя, dev){
    if (копия && !копия.done) throw new Error('копирование уже идёт');
    копия = { dev, имя, процент:0, шаг:'Считаю файлы', done:false, ok:false, error:null, начато:Date.now() };
    const rsync = await есть('rsync');
    (async () => {
      try {
        let всего = 0, сделано = 0;
        for (const [откуда] of пары) всего += (await объём(откуда)).байт;
        for (const [откуда, куда, подпись] of пары){
          if (!existsSync(откуда)) continue;
          await mkdir(куда, { recursive:true });
          копия.шаг = подпись;
          const до = сделано;
          const размер = (await объём(откуда)).байт;
          await new Promise((готово, беда) => {
            const п = rsync
              ? spawn('rsync', ['-rt', '--update', '--info=progress2', '--no-inc-recursive', '--modify-window=2', откуда + '/', куда + '/'], { stdio:['ignore', 'pipe', 'pipe'] })
              : spawn('cp', ['-r', '-u', '--preserve=timestamps', откуда + '/.', куда + '/'], { stdio:['ignore', 'pipe', 'pipe'] });
            let жалоба = '';
            п.stdout.on('data', d => {
              const х = ходRsync(String(d).split('\r').pop());
              if (х && всего > 0) копия.процент = Math.min(99, Math.round(100 * (до + размер * х.процент / 100) / всего));
            });
            п.stderr.on('data', d => { жалоба += d; });
            п.on('error', беда);
            п.on('close', код => код === 0 ? готово() : беда(new Error((жалоба.trim().split('\n').pop() || 'код ' + код))));
          });
          сделано += размер;
          if (всего > 0) копия.процент = Math.min(99, Math.round(100 * сделано / всего));
        }
        await run('sync', [], { timeout:120000 }).catch(() => {});
        Object.assign(копия, { done:true, ok:true, процент:100, шаг:'Готово', конец:Date.now() });
      } catch(e){
        Object.assign(копия, { done:true, ok:false, error:/No space left/i.test(e.message) ? 'на ключе кончилось место' : e.message });
      }
    })();
    return { started:true };
  }

  const латиницей = с => String(с || '').replace(/[^A-Za-z0-9._-]/g, '_') || 'pc';
  const папка_копии = (где) => join(где, ПАПКА, 'backup', латиницей(os.hostname()) + '-' + латиницей(os.userInfo().username));

  return {
    /* Ключи этого компьютера и вставленные флешки — с ключами и без */
    async 'key.state'({ подключить = false } = {}){
      const список = await мои();
      const флешки = [];
      for (const н of await носители()){
        if (подключить) н.где = await подключи(н);
        const к = await ключ_на(н.где);
        const свой = к ? список.find(м => м.id === к.id && тайна_подходит(к.секрет, м.хэш)) : null;
        let профиль = null, копия_когда = null, эстафета = false, спасение = null;
        if (к){
          try { const п = JSON.parse(await readFile(join(н.где, ПАПКА, 'profile.json'), 'utf8')); профиль = { когда:п.сохранён || null, откуда:п.компьютер || '', программ:(п.программы && п.программы.flatpak || []).length }; } catch(e){}
          try { копия_когда = (await stat(папка_копии(н.где))).mtimeMs; } catch(e){}
          try { const р = JSON.parse(await readFile(join(н.где, ПАПКА, 'relay', 'relay.json'), 'utf8')); эстафета = р.компьютер !== os.hostname(); } catch(e){}
          try { спасение = JSON.parse(await readFile(join(н.где, ПАПКА, 'rescue', 'info.json'), 'utf8')); } catch(e){}
        }
        флешки.push({ dev:н.dev, имя:н.метка || н.модель || н.dev, где:н.где, размер:н.размер, толькоЧтение:н.толькоЧтение,
          ключ:к ? { id:к.id, имя:к.имя || 'Ключ', свой:!!свой } : null, профиль, копия:копия_когда, эстафета,
          спасение:спасение ? { версия:String(спасение.версия || ''), когда:спасение.сохранено || null, здесь:к ? await спасательный(к.секрет) : false } : null });
      }
      const вставлены = new Set(флешки.filter(ф => ф.ключ && ф.ключ.свой).map(ф => ф.ключ.id));
      return {
        ключи:список.map(м => ({ id:м.id, имя:м.имя, вход:!!м.вход, блокировать:!!м.блокировать, копия:!!м.копия, эстафета:!!м.эстафета, вставлен:вставлены.has(м.id) })),
        флешки, вернуть_стол:existsSync(join(КОНФИГ(), 'before-key.json')), можно:!!allowLaunch,
      };
    },

    /* Для замка и ядра панели — быстро и без лишнего: вставлен ли свой
       ключ, которому разрешён вход */
    async 'key.present'(){
      const список = await мои();
      if (!список.length) return { вход:false, есть:[], ключей:0 };
      const есть = [];
      for (const н of await носители()){
        н.где = await подключи(н);
        const к = await ключ_на(н.где);
        const м = к && список.find(м => м.id === к.id && тайна_подходит(к.секрет, м.хэш));
        if (м) есть.push(м.id);
      }
      return { вход:список.some(м => м.вход && есть.includes(м.id)), есть, ключей:список.length,
        блокировать:список.filter(м => м.блокировать).map(м => м.id),
        имена:список.filter(м => м.вход).map(м => м.имя) };
    },

    /* Сделать флешку ключом: папка .glower-key, остальное не трогаем */
    async 'key.create'({ dev, имя } = {}){
      if (!allowLaunch) throw new Error('работа с носителями выключена');
      const н = await найди(dev);
      if (н.толькоЧтение) throw new Error('флешка защищена от записи');
      if (await ключ_на(н.где)) throw new Error('на этой флешке уже есть ключ Glower');
      const к = { формат:1, id:randomUUID(), имя:String(имя || '').trim().slice(0, 40) || 'Мой ключ',
        создан:new Date().toISOString(), секрет:randomBytes(32).toString('hex') };
      await mkdir(join(н.где, ПАПКА), { recursive:true });
      await writeFile(join(н.где, ПАПКА, 'key.json'), JSON.stringify(к, null, 2));
      await writeFile(join(н.где, ПАПКА, 'README.txt'),
        'This is a Glower key. Do not delete the .glower-key folder or copy it to other drives:\n'
        + 'whoever has its contents can unlock computers where sign-in with this key is on.\n'
        + 'The file copy in the backup folder is not encrypted.\n\n'
        + 'Это ключ Glower. Папку .glower-key не удаляйте и не копируйте на чужие флешки:\n'
        + 'кто владеет её содержимым, тот может открыть компьютеры, где для ключа включён вход.\n'
        + 'Копия файлов в папке backup не зашифрована.\n');
      await run('sync', [], { timeout:60000 }).catch(() => {});
      const список = await мои();
      список.push({ id:к.id, имя:к.имя, хэш:отпечаток(к.секрет), вход:true, блокировать:false, копия:false, эстафета:true, добавлен:new Date().toISOString() });
      await запомни(список);
      return { ok:true, id:к.id, имя:к.имя };
    },

    /* Ключ сделан на другом компьютере — признать его своим и здесь */
    async 'key.pair'({ dev } = {}){
      const { к } = await свой_ключ(dev);
      const список = (await мои()).filter(м => м.id !== к.id);
      список.push({ id:к.id, имя:к.имя || 'Ключ', хэш:отпечаток(к.секрет), вход:true, блокировать:false, копия:false, эстафета:true, добавлен:new Date().toISOString() });
      await запомни(список);
      return { ok:true, id:к.id };
    },

    async 'key.forget'({ id } = {}){
      await запомни((await мои()).filter(м => м.id !== id));
      return { ok:true };
    },

    async 'key.set'({ id, вход, блокировать, копия, эстафета } = {}){
      const список = await мои();
      const м = список.find(м => м.id === id);
      if (!м) throw new Error('такого ключа этот компьютер не знает');
      if (вход !== undefined) м.вход = !!вход;
      if (блокировать !== undefined) м.блокировать = !!блокировать;
      if (копия !== undefined) м.копия = !!копия;
      if (эстафета !== undefined) м.эстафета = !!эстафета;
      await запомни(список);
      return { ok:true };
    },

    /* Сохранить свой стол на ключ */
    async 'key.profile.save'({ dev } = {}){
      const { н } = await свой_ключ(dev);
      let настройки = {};
      try { настройки = JSON.parse(await readFile(join(КОНФИГ(), 'settings.json'), 'utf8')); } catch(e){}
      const стол = только_стол(настройки);
      let обои = null;
      const свои = String(стол.wallpaper || '').startsWith('файл:') ? String(стол.wallpaper).slice(5) : '';
      if (свои && existsSync(свои)){
        const расш = (extname(свои) || '.jpg').toLowerCase().replace(/[^.a-z0-9]/g, '');
        обои = 'wallpaper' + расш;
        await copyFile(свои, join(н.где, ПАПКА, обои));
        стол.wallpaper = 'файл-ключа';
      }
      let мышь = null;
      try { мышь = JSON.parse(await readFile(join(КОНФИГ(), 'mouse.json'), 'utf8')); } catch(e){}
      let flatpak = [];
      try {
        flatpak = (await run('flatpak', ['list', '--app', '--columns=application'], { timeout:20000 })).stdout
          .split('\n').map(с => с.trim()).filter(с => /^[\w.-]+\.[\w.-]+$/.test(с));
      } catch(e){}
      let язык = null, раскладки = null;
      try { язык = ЯЗЫКИ[(await readFile(join(os.homedir(), '.config/glower/locale'), 'utf8')).trim()] || null; } catch(e){}
      try { раскладки = (await readFile(join(os.homedir(), '.config/glower/keyboard'), 'utf8')).trim().split(',').filter(Boolean); } catch(e){}
      const система = только_систему({ язык, раскладки, userName:настройки.userName, tz:настройки.tz, clock24:настройки.clock24 });
      const профиль = { формат:1, сохранён:new Date().toISOString(), компьютер:os.hostname(),
        настройки:стол, система, обои, мышь, программы:{ flatpak:[...new Set(flatpak)] } };
      await writeFile(join(н.где, ПАПКА, 'profile.json'), JSON.stringify(профиль, null, 2));
      await run('sync', [], { timeout:60000 }).catch(() => {});
      return { ok:true, программ:профиль.программы.flatpak.length };
    },

    /* Прочитать стол с ключа. Обои кладём рядом с настройками этого
       компьютера: флешку вынут, а стол должен остаться */
    async 'key.profile.read'({ dev } = {}){
      const { н } = await свой_ключ(dev);
      let п;
      try { п = JSON.parse(await readFile(join(н.где, ПАПКА, 'profile.json'), 'utf8')); }
      catch(e){ throw new Error('на ключе ещё нет сохранённого стола'); }
      const настройки = только_стол(п.настройки);
      if (настройки.wallpaper === 'файл-ключа'){
        delete настройки.wallpaper;
        const имя = String(п.обои || '');
        if (/^wallpaper\.[a-z0-9]{1,5}$/.test(имя) && existsSync(join(н.где, ПАПКА, имя))){
          const куда = join(КОНФИГ(), 'обои-с-ключа' + extname(имя));
          await copyFile(join(н.где, ПАПКА, имя), куда);
          настройки.wallpaper = 'файл:' + куда;
        }
      }
      // Программы: каких из списка здесь ещё нет
      let стоят = [];
      try { стоят = (await run('flatpak', ['list', '--app', '--columns=application'], { timeout:20000 })).stdout.split('\n').map(с => с.trim()); } catch(e){}
      const все = (п.программы && Array.isArray(п.программы.flatpak) ? п.программы.flatpak : []).filter(с => /^[\w.-]+\.[\w.-]+$/.test(с));
      return { настройки, система:только_систему(п.система), мышь:п.мышь && typeof п.мышь === 'object' ? п.мышь : null, когда:п.сохранён || null, откуда:п.компьютер || '',
        программы:все, нет:все.filter(id => !стоят.includes(id)) };
    },

    /* Копия папок человека на ключ */
    async 'key.backup'({ dev } = {}){
      const { н } = await свой_ключ(dev);
      const куда = папка_копии(н.где);
      // Копия прежних версий — с русскими именами папок: переименовываем
      for (const [здесь, там] of ПАПКИ_КОПИИ)
        if (existsSync(join(куда, здесь)) && !existsSync(join(куда, там))) await rename(join(куда, здесь), join(куда, там)).catch(() => {});
      return перенеси(ПАПКИ_КОПИИ.map(([здесь, там]) => [join(ДОМ, здесь), join(куда, там), 'Копирую «' + здесь + '»']), 'копия', dev);
    },

    /* Вернуть файлы с ключа: только недостающие и более новые — то, что
       здесь новее, не трогаем */
    async 'key.restore'({ dev } = {}){
      const { н } = await свой_ключ(dev);
      const откуда = папка_копии(н.где);
      if (!existsSync(откуда)) throw new Error('копии с этого компьютера на ключе нет');
      return перенеси(ПАПКИ_КОПИИ.map(([здесь, там]) => [existsSync(join(откуда, там)) ? join(откуда, там) : join(откуда, здесь), join(ДОМ, здесь), 'Возвращаю «' + здесь + '»']), 'возврат', dev);
    },

    async 'key.backup.state'(){
      return копия ? { ...копия, идёт:!копия.done } : { идёт:false };
    },

    /* Сделать ключ спасательным: положить на него пакет нынешней версии
       GlowerOS и сказать загрузке, что этому ключу можно показать меню */
    async 'key.rescue.save'({ dev } = {}){
      const { н, к } = await свой_ключ(dev);
      const env = { ...process.env, LC_ALL:'C' };
      const версия = (await run('dpkg-query', ['-W', '-f=${Version}', 'glower'], { env }).catch(() => ({ stdout:'' }))).stdout.trim();
      if (!/^[0-9][\w.+~-]*$/.test(версия)) throw new Error('не вышло узнать версию GlowerOS');
      // Пакет: из кэша apt, а нет там — скачиваем ту же версию
      let пакет = null;
      for (const и of await readdir('/var/cache/apt/archives').catch(() => []))
        if (и.startsWith('glower_' + версия.replace(/:/g, '%3a') + '_') && и.endsWith('.deb')) пакет = join('/var/cache/apt/archives', и);
      const врем = join(os.tmpdir(), 'glower-rescue-' + process.pid);
      if (!пакет){
        await mkdir(врем, { recursive:true });
        try { await run('apt-get', ['download', 'glower=' + версия], { cwd:врем, env, timeout:300000 }); }
        catch(e){ await rm(врем, { recursive:true, force:true }); throw new Error('пакет GlowerOS ' + версия + ' не скачался — нужна сеть'); }
        const и = (await readdir(врем)).find(и => и.endsWith('.deb'));
        if (!и) throw new Error('пакет GlowerOS не скачался');
        пакет = join(врем, и);
      }
      const папка = join(н.где, ПАПКА, 'rescue');
      await mkdir(папка, { recursive:true });
      await copyFile(пакет, join(папка, 'glower.deb.new'));
      await rename(join(папка, 'glower.deb.new'), join(папка, 'glower.deb'));
      await writeFile(join(папка, 'info.json'), JSON.stringify({ версия, сохранено:new Date().toISOString(), компьютер:os.hostname() }, null, 2));
      await rm(врем, { recursive:true, force:true });
      // Отпечаток — в список, что читает загрузка
      if (!await спасательный(к.секрет)){
        const п = spawn('sudo', ['-n', 'sh', '-c', 'install -d -m 755 /var/lib/glower && cat >> ' + СПАСЕНИЕ + ' && chmod 644 ' + СПАСЕНИЕ], { stdio:['pipe', 'ignore', 'pipe'] });
        let жалоба = '';
        п.stderr.on('data', d => { жалоба += d; });
        п.on('error', () => {});
        п.stdin.on('error', () => {});
        п.stdin.end(отпечаток(к.секрет) + '\n');
        const код = await new Promise(r => п.on('close', r));
        if (код !== 0) throw new Error('не вышло записать ключ в список спасательных: ' + (жалоба.trim() || 'код ' + код));
      }
      await run('sync', [], { timeout:60000 }).catch(() => {});
      return { ok:true, версия };
    },

    /* Убрать ключ с флешки совсем — сама флешка и её файлы остаются */
    async 'key.erase'({ dev } = {}){
      const { н, к } = await свой_ключ(dev);
      await rm(join(н.где, ПАПКА), { recursive:true, force:true });
      await запомни((await мои()).filter(м => м.id !== к.id));
      await run('sync', [], { timeout:60000 }).catch(() => {});
      return { ok:true };
    },
  };
}
