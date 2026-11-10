/* ==========================================================================
   Папки человека на его языке

   Рабочий стол, Документы, Изображения, Музыка, Загрузки и Видео живут в
   ~/GlowerOS. Раньше они всегда назывались по-русски — на любом языке
   системы: надписи в наших программах переводились, а в терминале, в окне
   «Сохранить как» Firefox и в любой сторонней программе человек видел
   кириллицу.

   Теперь новые папки получают имена на языке системы, а где какая —
   записано в ~/.config/user-dirs.dirs: по этому файлу их находят и наши
   программы, и все остальные (Firefox сохраняет в «Загрузки» человека).

   Уже созданные папки не переименовываем: в них файлы человека, и на них
   могут вести ярлыки и пути. Единственное исключение — пустая папка с
   нашим стандартным именем на другом языке: такую при смене языка (или
   сразу после первой настройки) переименовываем, ей терять нечего.
   ========================================================================== */
import { readFile, writeFile, mkdir, readdir, rename } from 'node:fs/promises';
import { existsSync, readFileSync, statSync, watch } from 'node:fs';
import { join, basename, dirname, sep } from 'node:path';
import os from 'node:os';

export const ВИДЫ = ['DESKTOP', 'DOCUMENTS', 'PICTURES', 'MUSIC', 'DOWNLOAD', 'VIDEOS'];

/* Как эти папки зовутся на каждом языке — как у xdg-user-dirs */
export const ИМЕНА = {
  ru: ['Рабочий стол', 'Документы', 'Изображения', 'Музыка', 'Загрузки', 'Видео'],
  en: ['Desktop', 'Documents', 'Pictures', 'Music', 'Downloads', 'Videos'],
  uk: ['Стільниця', 'Документи', 'Зображення', 'Музика', 'Завантаження', 'Відео'],
  be: ['Працоўны стол', 'Дакументы', 'Відарысы', 'Музыка', 'Сцягнутае', 'Відэа'],
  kk: ['Жұмыс үстелі', 'Құжаттар', 'Суреттер', 'Музыка', 'Жүктемелер', 'Бейнелер'],
  de: ['Schreibtisch', 'Dokumente', 'Bilder', 'Musik', 'Downloads', 'Videos'],
  fr: ['Bureau', 'Documents', 'Images', 'Musique', 'Téléchargements', 'Vidéos'],
  es: ['Escritorio', 'Documentos', 'Imágenes', 'Música', 'Descargas', 'Vídeos'],
  it: ['Scrivania', 'Documenti', 'Immagini', 'Musica', 'Scaricati', 'Video'],
  pt: ['Área de trabalho', 'Documentos', 'Imagens', 'Música', 'Downloads', 'Vídeos'],
  pl: ['Pulpit', 'Dokumenty', 'Obrazy', 'Muzyka', 'Pobrane', 'Wideo'],
  tr: ['Masaüstü', 'Belgeler', 'Resimler', 'Müzik', 'İndirilenler', 'Videolar'],
};

/* Папки, которые заводят сами наши программы, — тоже на языке человека */
export const ПОДПАПКИ = {
  'Эстафета': { en:'Relay', uk:'Естафета', be:'Эстафета', kk:'Эстафета', de:'Staffel', fr:'Relais', es:'Relevo',
    it:'Staffetta', pt:'Revezamento', pl:'Sztafeta', tr:'Bayrak' },
};

const конфиг = () => process.env.XDG_CONFIG_HOME || join(os.homedir(), '.config');
export const ФАЙЛ_ПАПОК = () => join(конфиг(), 'user-dirs.dirs');

/* Язык — тот же, что у панели: ~/.config/glower/locale, иначе LANG */
export function язык_из(локаль){
  const к = String(локаль || '').trim().slice(0, 2).toLowerCase();
  return ИМЕНА[к] ? к : 'ru';
}
export function язык(){
  let л = '';
  try { л = readFileSync(join(конфиг(), 'glower', 'locale'), 'utf8').trim(); } catch(e){}
  return язык_из(л || process.env.LANG || '');
}

export function подпапка(русское, яз = язык()){
  return (ПОДПАПКИ[русское] && ПОДПАПКИ[русское][яз]) || русское;
}

/* XDG_DOCUMENTS_DIR="$HOME/GlowerOS/Документы" → { DOCUMENTS: '/home/…/Документы' } */
export function разбери(текст, дом){
  const итог = {};
  for (const с of String(текст).split('\n')){
    const м = с.match(/^\s*XDG_([A-Z]+)_DIR\s*=\s*"(.*)"\s*$/);
    if (!м) continue;
    итог[м[1]] = м[2].replace(/^\$HOME(?=\/|$)/, дом).replace(/\\"/g, '"');
  }
  return итог;
}

/* Переписываем только свои строки, чужие (шаблоны, общая папка) — как были */
export function собери(текст, папки, дом){
  const строки = String(текст || '').split('\n').filter(с => с.trim() !== '');
  const путь = п => п === дом || п.startsWith(дом + sep) ? '$HOME' + п.slice(дом.length) : п;
  for (const [вид, п] of Object.entries(папки)){
    const новая = `XDG_${вид}_DIR="${путь(п).replace(/"/g, '\\"')}"`;
    const i = строки.findIndex(с => new RegExp(`^\\s*XDG_${вид}_DIR\\s*=`).test(с));
    if (i >= 0) строки[i] = новая; else строки.push(новая);
  }
  if (!строки.some(с => с.startsWith('#'))) строки.unshift('# Папки человека — их находят все программы. Пишет GlowerOS (agent/folders.mjs)');
  return строки.join('\n') + '\n';
}

const папка_ли = п => { try { return statSync(п).isDirectory(); } catch(e){ return false; } };
async function пустая(п){ try { return (await readdir(п)).length === 0; } catch(e){ return false; } }
const внутри = (п, корень) => п === корень || п.startsWith(корень + sep);

/* Готовит папки в root и возвращает { ВИД: путь }. писать — записать
   user-dirs.dirs (только в настоящем сеансе, а не в проверках) */
export async function подготовь_папки({ root, дом = os.homedir(), яз = язык(), писать = true }){
  const файл = ФАЙЛ_ПАПОК();
  const текст = await readFile(файл, 'utf8').catch(() => '');
  const было = разбери(текст, дом);
  const итог = {};
  await mkdir(root, { recursive:true });
  for (const [i, вид] of ВИДЫ.entries()){
    const нужное = join(root, ИМЕНА[яз][i]);
    const свои_имена = Object.values(ИМЕНА).map(н => н[i]);
    // Записанная папка годится, если она наша (в ~/GlowerOS) и есть на диске:
    // ~/Desktop от Ubuntu или «$HOME» по умолчанию — не наши
    let п = было[вид] && внутри(было[вид], root) && было[вид] !== root && папка_ли(было[вид]) ? было[вид] : null;
    // Не записана — ищем уже существующую с нашим именем на любом языке
    // (старые системы: «Документы»), сначала на своём языке
    if (!п) п = [нужное, ...свои_имена.map(и => join(root, и))].find(папка_ли) || null;
    // Пустая папка с нашим именем на другом языке — переименовать
    if (п && п !== нужное && dirname(п) === root && свои_имена.includes(basename(п)) && !existsSync(нужное) && await пустая(п)){
      try { await rename(п, нужное); п = нужное; } catch(e){}
    }
    if (!п){ await mkdir(нужное, { recursive:true }); п = нужное; }
    итог[вид] = п;
  }
  if (писать){
    const новый = собери(текст, итог, дом);
    if (новый !== текст){
      await mkdir(dirname(файл), { recursive:true });
      await writeFile(файл, новый);
    }
  }
  return итог;
}

/* Где сейчас папка — для агента, синхронно: из user-dirs.dirs, иначе
   старое русское имя, иначе имя на языке системы */
export function особая(вид, root){
  const дом = os.homedir();
  let п = null;
  try { п = разбери(readFileSync(ФАЙЛ_ПАПОК(), 'utf8'), дом)[вид]; } catch(e){}
  if (п && папка_ли(п) && п !== дом) return п;
  const i = ВИДЫ.indexOf(вид);
  const русская = join(root, ИМЕНА.ru[i]);
  return папка_ли(русская) ? русская : join(root, ИМЕНА[язык()][i]);
}

/* Язык сменили — пустые папки переименовать */
export function следи_за_языком(root){
  try {
    const н = watch(join(конфиг(), 'glower'), (_, имя) => {
      if (имя === 'locale') подготовь_папки({ root }).catch(() => {});
    });
    н.on('error', () => {});
    if (н.unref) н.unref();
  } catch(e){}
}
