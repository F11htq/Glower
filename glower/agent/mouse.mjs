/* ==========================================================================
   Мышь, тачпад и указатель — как «Мышь» в параметрах Windows

   Хранится в ~/.config/glower/mouse.json. Применяется туда, где каждую
   настройку ищут:
     — скорость, точность, левша, прокрутка, тачпад — раздел <libinput>
       в rc.xml личной папки сеанса: его читают и labwc, и glower-wm;
     — тема и размер указателя — XCURSOR_THEME/XCURSOR_SIZE в environment
       той же папки (сервер и окна X11) и cursor-theme/cursor-size в
       gsettings (программы GTK под Wayland);
     — скорость двойного щелчка — gsettings, её спрашивают сами программы.
   После записи оба сервера получают SIGHUP и перечитывают настройки — как
   с раскладками. Папки сеанса нет (вход по-старому, под X) — настройки
   запомнены и применятся со следующего входа.
   ========================================================================== */
import { execFile } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir, lstat, unlink, realpath } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);
const ФАЙЛ = () => join(os.homedir(), '.config/glower/mouse.json');
const ПАПКА_СЕАНСА = () => join(process.env.XDG_RUNTIME_DIR || '/run/user/' + (process.getuid ? process.getuid() : 1000), 'glower-labwc');

/* Как в Windows по умолчанию */
export const ПО_УМОЛЧАНИЮ = {
  левша:false,          // основная кнопка — левая
  скорость:0,           // -1…1 (в Windows — 10 из 20)
  точность:true,        // «Повышенная точность установки указателя»
  строки:3,             // строк за щелчок колеса
  обратная:false,       // колесо мыши — как обычно
  двойной:400,          // мс
  тач_касание:true,
  тач_обратная:true,    // «Движение вниз прокручивает вниз», как у тачпада в Windows
  тач_без_набора:true,
  тач_два_пальца:false, // правый щелчок — нажатием в правом нижнем углу, как в Windows
  тач_скорость:0,
  курсор:'Adwaita',
  размер:24,
};

async function прочти(){
  try { return { ...ПО_УМОЛЧАНИЮ, ...JSON.parse(await readFile(ФАЙЛ(), 'utf8')) }; }
  catch(e){ return { ...ПО_УМОЛЧАНИЮ }; }
}

const да = в => в ? 'yes' : 'no';
const число = (в, мин, макс, по) => { const ч = Number(в); return Number.isFinite(ч) ? Math.min(макс, Math.max(мин, ч)) : по; };

/* Раздел <libinput> для rc.xml — в виде labwc */
export function раздел_libinput(н){
  return [
    '<libinput>',
    '  <device category="non-touch">',
    `    <pointerSpeed>${число(н.скорость, -1, 1, 0).toFixed(2)}</pointerSpeed>`,
    `    <accelProfile>${н.точность ? 'adaptive' : 'flat'}</accelProfile>`,
    `    <leftHanded>${да(н.левша)}</leftHanded>`,
    `    <naturalScroll>${да(н.обратная)}</naturalScroll>`,
    `    <scrollFactor>${(число(н.строки, 1, 15, 3) / 3).toFixed(2)}</scrollFactor>`,
    '  </device>',
    '  <device category="touchpad">',
    `    <pointerSpeed>${число(н.тач_скорость, -1, 1, 0).toFixed(2)}</pointerSpeed>`,
    `    <leftHanded>${да(н.левша)}</leftHanded>`,
    `    <naturalScroll>${да(н.тач_обратная)}</naturalScroll>`,
    `    <tap>${да(н.тач_касание)}</tap>`,
    `    <disableWhileTyping>${да(н.тач_без_набора)}</disableWhileTyping>`,
    `    <clickMethod>${н.тач_два_пальца ? 'clickfinger' : 'buttonAreas'}</clickMethod>`,
    '  </device>',
    '</libinput>',
  ].join('\n');
}

/* rc.xml с новым разделом: прежний <libinput> убираем, новый — перед концом */
export function вставь_раздел(rc, раздел){
  const без = String(rc).replace(/\s*<libinput>[\s\S]*?<\/libinput>/g, '');
  const к = без.lastIndexOf('</labwc_config>');
  if (к < 0) return без + '\n' + раздел + '\n';
  return без.slice(0, к) + раздел + '\n' + без.slice(к);
}

/* Строки environment: свои заменяем, чужие (раскладки) не трогаем */
export function среда_с(текст, пары){
  const имена = Object.keys(пары);
  return String(текст).split('\n').filter(с => с && !имена.some(и => с.startsWith(и + '=')))
    .concat(имена.map(и => `${и}=${пары[и]}`)).join('\n') + '\n';
}

/* Темы указателя, что есть в системе */
async function темы(){
  const корни = ['/usr/share/icons', join(os.homedir(), '.icons'), join(os.homedir(), '.local/share/icons')];
  const есть = new Set();
  for (const к of корни){
    let ф = [];
    try { ф = await readdir(к); } catch(e){ continue; }
    for (const имя of ф) if (existsSync(join(к, имя, 'cursors'))) есть.add(имя);
  }
  const ИМЕНА = { 'Adwaita':'Стандартный', 'DMZ-White':'Белый', 'DMZ-Black':'Чёрный',
    'Bibata-Modern-Classic':'Bibata — чёрный', 'Bibata-Modern-Ice':'Bibata — белый', 'Bibata-Modern-Amber':'Bibata — янтарный',
    'Bibata-Original-Classic':'Bibata классический — чёрный', 'Bibata-Original-Ice':'Bibata классический — белый',
    'breeze_cursors':'Breeze', 'Breeze_Light':'Breeze — светлый', 'Yaru':'Yaru' };
  return [...есть].filter(и => !/^(default|hicolor|locolor)$/.test(и)).sort((а, б) => (а === 'Adwaita' ? -1 : б === 'Adwaita' ? 1 : а.localeCompare(б)))
    .map(id => ({ id, имя:ИМЕНА[id] || id.replace(/[-_]/g, ' ') }));
}

async function примени(н){
  const итог = { сразу:false };
  const папка = ПАПКА_СЕАНСА();
  // rc.xml: в личной папке сеанса это ссылка на общий файл системы — читаем
  // общий и кладём на её место свой, с разделом мыши
  try {
    const rc = join(папка, 'rc.xml');
    const текст = await readFile(await realpath(rc), 'utf8');
    if ((await lstat(rc)).isSymbolicLink()) await unlink(rc);
    await writeFile(rc, вставь_раздел(текст, раздел_libinput(н)));
    const env = join(папка, 'environment');
    if (existsSync(env)){
      const было = await readFile(env, 'utf8');
      await writeFile(env, среда_с(было, { XCURSOR_THEME:н.курсор, XCURSOR_SIZE:String(н.размер) }));
    }
    for (const имя of ['labwc', 'glower-wm']){
      const { stdout } = await run('pgrep', ['-u', String(process.getuid ? process.getuid() : ''), '-x', имя]).catch(() => ({ stdout:'' }));
      for (const pid of String(stdout).split('\n').map(Number).filter(Boolean)){
        try { process.kill(pid, 'SIGHUP'); итог.сразу = true; } catch(e){}
      }
    }
  } catch(e){ /* сеанс не под Wayland — со следующего входа */ }
  // Программам GTK — курсор и двойной щелчок
  const gs = async (схема, ключ, значение) => run('gsettings', ['set', схема, ключ, значение], { timeout:5000 }).catch(() => {});
  await gs('org.gnome.desktop.interface', 'cursor-theme', н.курсор);
  await gs('org.gnome.desktop.interface', 'cursor-size', String(н.размер));
  await gs('org.gnome.desktop.peripherals.mouse', 'double-click', String(Math.round(число(н.двойной, 150, 1200, 400))));
  return итог;
}

export function mouse(){
  return {
    async 'sys.mouse'(){
      return { настройки:await прочти(), темы:await темы() };
    },

    async 'sys.mouse.set'(изменения = {}){
      const н = await прочти();
      for (const [к, в] of Object.entries(изменения)){
        if (!(к in ПО_УМОЛЧАНИЮ)) continue;
        const нужен = typeof ПО_УМОЛЧАНИЮ[к];
        if (нужен === 'boolean') н[к] = !!в;
        else if (нужен === 'number') н[к] = Number(в);
        else if (к === 'курсор' && /^[\w.+-]{1,64}$/.test(String(в))) н[к] = String(в);
      }
      н.скорость = число(н.скорость, -1, 1, 0);
      н.тач_скорость = число(н.тач_скорость, -1, 1, 0);
      н.строки = Math.round(число(н.строки, 1, 15, 3));
      н.двойной = Math.round(число(н.двойной, 150, 1200, 400));
      н.размер = Math.round(число(н.размер, 16, 96, 24));
      await mkdir(join(os.homedir(), '.config/glower'), { recursive:true });
      await writeFile(ФАЙЛ(), JSON.stringify(н, null, 2));
      return { настройки:н, ...(await примени(н)) };
    },

    /* При входе: личная папка сеанса собрана заново из общих файлов —
       возвращаем в неё настройки человека */
    async 'sys.mouse.apply'(){
      return await примени(await прочти());
    },
  };
}
