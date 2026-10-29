/* ==========================================================================
   «Защита»: проверка файлов на вирусы

   Проверяет ClamAV — обычный антивирус Linux с открытой базой. Работает он
   только по требованию: человек попросил проверить, скачанный пакет ставят,
   программу Windows открывают. Службы, которая сидит в памяти постоянно, нет:
   у ClamAV она занимает около гигабайта, а старой машине это половина памяти.
   Цена — каждая проверка сперва секунд двадцать загружает базу.

   Базу скачивает freshclam раз в сутки, пока «Защита» включена. Выключенная
   «Защита» не делает ничего вовсе: не обновляется, не проверяет, не держит
   таймеров с работой — кто бережёт каждую секунду процессора, просит именно
   этого.

   Официальный сервер баз ClamAV из России файлов не отдаёт (ответ 403), а
   у части людей и заблокирован. Поэтому есть запасной путь — зеркало, которое
   держит Microsoft: те же подписанные файлы ClamAV, freshclam проверяет их
   подпись сам. Получилось через какой-то путь — в следующий раз начинаем с
   него.

   К базе ClamAV добавлены сигнатуры из DymAV (linux/clamav): они не вместо
   неё, а вдобавок. Без базы ClamAV «Защита» честно говорит, что не работает:
   три десятка своих сигнатур — не антивирус.
   ========================================================================== */
import { spawn, execFile } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir, rename, copyFile, unlink, rm, stat, lstat, chmod, open } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);
const ДОМ = os.homedir();
const НАСТРОЙКИ = join(ДОМ, '.config/glower/protection.json');
const СВОЯ_БАЗА = join(ДОМ, '.local/share/glower/clamav');
const ОБЩАЯ_БАЗА = '/var/lib/clamav';
export const КАРАНТИН = join(ДОМ, '.local/share/glower/quarantine');
const ВРЕМЕННАЯ = join(ДОМ, '.cache/glower/av');
const НАШИ = join(dirname(fileURLToPath(import.meta.url)), '..', 'linux', 'clamav');

const ЗЕРКАЛА = [
  { id:'clamav', строки:['DatabaseMirror database.clamav.net'] },
  { id:'microsoft', строки:['PrivateMirror https://packages.microsoft.com/clamav', 'ScriptedUpdates no'] },
];
const СУТКИ = 24 * 3600 * 1000;

/* ---------- настройки ---------- */

let настройки = null;
async function н(){
  if (настройки) return настройки;
  try { настройки = JSON.parse(await readFile(НАСТРОЙКИ, 'utf8')); }
  catch(e){ настройки = {}; }
  if (typeof настройки.вкл !== 'boolean') настройки.вкл = true;
  return настройки;
}
async function сохрани(){
  await mkdir(dirname(НАСТРОЙКИ), { recursive:true });
  await writeFile(НАСТРОЙКИ, JSON.stringify(настройки, null, 2));
}

async function есть(cmd){
  try { await run('which', [cmd]); return true; } catch(e){ return false; }
}

/* ---------- база ---------- */

const ФАЙЛ_БАЗЫ = /\.(cvd|cld|cud)$/;

async function файлы_базы(папка){
  try { return (await readdir(папка)).filter(ф => ФАЙЛ_БАЗЫ.test(ф)); }
  catch(e){ return []; }
}

/* Годная база — та, где есть main или daily: bytecode один ничего не ловит */
async function где_база(){
  for (const п of [ОБЩАЯ_БАЗА, СВОЯ_БАЗА]){
    const ф = await файлы_базы(п);
    if (ф.some(х => /^(main|daily)\./.test(х))) return п;
  }
  return null;
}

/* Заголовок файла базы: «ClamAV-VDB:29 Oct 2026 12-09 +0000:27805:…» —
   там дата выпуска и номер. По ним и видно, насколько база свежая. */
async function заголовок(файл){
  let ф;
  try {
    ф = await open(файл, 'r');
    const б = Buffer.alloc(512);
    await ф.read(б, 0, 512, 0);
    const ч = б.toString('latin1').split(':');
    if (ч[0] !== 'ClamAV-VDB') return null;
    const [д, мес, год, вр, пояс] = ч[1].split(' ');
    const когда = Date.parse(`${д} ${мес} ${год} ${(вр || '').replace('-', ':')} ${пояс || ''}`);
    return { когда:Number.isFinite(когда) ? когда : null, номер:Number(ч[2]) || null, сигнатур:Number(ч[3]) || 0 };
  } catch(e){ return null; }
  finally { await ф?.close().catch(() => {}); }
}

async function о_базе(){
  const папка = await где_база();
  if (!папка) return null;
  const ф = await файлы_базы(папка);
  let когда = null, номер = null, сигнатур = 0, байт = 0;
  for (const имя of ф){
    const з = await заголовок(join(папка, имя));
    try { байт += (await stat(join(папка, имя))).size; } catch(e){}
    if (!з) continue;
    сигнатур += з.сигнатур;
    if (/^daily\./.test(имя)){ когда = з.когда; номер = з.номер; }
  }
  return { папка, когда, номер, сигнатур, байт };
}

function доводы_баз(папка){
  const д = ['-d', папка];
  for (const ф of ['glower.ndb', 'glower.yar']){
    if (existsSync(join(НАШИ, ф))) д.push('-d', join(НАШИ, ф));
  }
  return д;
}

/* ---------- обновление ---------- */

let обновляю = null;
let sudo_можно = null;

async function можно_sudo(){
  if (sudo_можно !== null) return sudo_можно;
  try { await run('sudo', ['-n', 'true'], { timeout:5000 }); sudo_можно = true; }
  catch(e){ sudo_можно = false; }
  return sudo_можно;
}

async function есть_clamav_пользователь(){
  try { return /^clamav:/m.test(await readFile('/etc/passwd', 'utf8')); } catch(e){ return false; }
}

function выполни(cmd, args, срок){
  return new Promise(готово => {
    const п = spawn(cmd, args, { stdio:['ignore', 'pipe', 'pipe'] });
    let вывод = '';
    const копи = ч => { вывод = (вывод + ч).slice(-20000); };
    п.stdout.on('data', копи);
    п.stderr.on('data', копи);
    const таймер = setTimeout(() => п.kill('SIGTERM'), срок);
    п.on('error', е => { clearTimeout(таймер); готово({ код:-1, вывод:String(е.message) }); });
    п.on('close', код => { clearTimeout(таймер); готово({ код, вывод }); });
  });
}

/* Что сказать человеку, если freshclam не справился */
function почему(вывод){
  const с = String(вывод);
  if (/403|Forbidden|blocked/i.test(с)) return 'сервер баз отказал';
  if (/Can't connect|Couldn't connect|resolve|Could not resolve|timed out|Timeout/i.test(с)) return 'нет связи с сервером баз';
  if (/No space|ENOSPC/i.test(с)) return 'на диске не хватает места для базы';
  if (/Permission denied|Can't (create|open)/i.test(с)) return 'нет права записать базу';
  if (/locked by another process|lock/i.test(с)) return 'базу уже обновляет другая программа';
  const стр = с.split('\n').map(х => х.trim()).filter(х => /ERROR/.test(х)).pop();
  return стр ? стр.replace(/^ERROR:\s*/, '') : 'обновление не удалось';
}

async function обнови(){
  if (обновляю) return обновляю;
  обновляю = (async () => {
    const нас = await н();
    нас.попытка = Date.now();
    if (!await есть('freshclam')) throw new Error('в системе нет ClamAV');
    const общая = await можно_sudo();
    const папка = общая ? ОБЩАЯ_БАЗА : СВОЯ_БАЗА;
    if (!общая) await mkdir(папка, { recursive:true });
    await mkdir(ВРЕМЕННАЯ, { recursive:true });
    const владелец = общая ? (await есть_clamav_пользователь() ? 'clamav' : 'root') : os.userInfo().username;
    // Сперва — путь, который сработал в прошлый раз
    const порядок = [...ЗЕРКАЛА].sort((а, б) => (б.id === нас.зеркало) - (а.id === нас.зеркало));
    let беда = '';
    for (const з of порядок){
      const conf = join(ВРЕМЕННАЯ, `freshclam-${з.id}.conf`);
      await writeFile(conf, [
        `DatabaseDirectory ${папка}`,
        `DatabaseOwner ${владелец}`,
        'Foreground yes',
        'MaxAttempts 2',
        'ConnectTimeout 20',
        ...з.строки, ''
      ].join('\n'));
      const { код, вывод } = общая
        ? await выполни('sudo', ['-n', 'freshclam', `--config-file=${conf}`, '--stdout'], 30 * 60000)
        : await выполни('freshclam', [`--config-file=${conf}`, '--stdout'], 30 * 60000);
      if (код === 0){
        нас.обновлено = Date.now();
        нас.зеркало = з.id;
        delete нас.ошибка;
        await сохрани();
        return await о_базе();
      }
      беда = почему(вывод);
    }
    нас.ошибка = беда;
    await сохрани();
    throw new Error(беда);
  })();
  try { return await обновляю; }
  finally { обновляю = null; }
}

/* Сами раз в сутки — только когда включено и только в установленной
   системе: в живом сеансе с флешки база легла бы в оперативную память и
   пропала бы при выключении. Проверяем раз в час, а качаем — если прошли
   сутки с удачного раза и три часа с неудачного. */
function живой_сеанс(){
  return existsSync('/run/live/medium') || existsSync('/run/glower-try');
}
async function пора(){
  const нас = await н();
  if (!нас.вкл || обновляю || живой_сеанс()) return;
  if (!await есть('freshclam')) return;
  const теперь = Date.now();
  if (теперь - (нас.обновлено || 0) < СУТКИ) return;
  if (теперь - (нас.попытка || 0) < 3 * 3600 * 1000) return;
  обнови().catch(е => console.error('Защита: база не обновилась:', е.message));
}

/* ---------- проверка ---------- */

/* Сколько файлов впереди — чтобы показывать проценты. Считаем, пока ClamAV
   грузит базу: эти двадцать секунд всё равно уходят на ожидание. */
const МИМО = /^\/(proc|sys|dev|run)(\/|$)/;
async function сосчитай(пути, стоп){
  let всего = 0;
  const стопка = [...пути];
  while (стопка.length && !стоп()){
    const п = стопка.pop();
    if (МИМО.test(п) || п.startsWith(КАРАНТИН) || п === ОБЩАЯ_БАЗА) continue;
    let с;
    try { с = await lstat(п); } catch(e){ continue; }
    if (с.isFile()){ всего++; continue; }
    if (!с.isDirectory()) continue;
    let ф;
    try { ф = await readdir(п, { withFileTypes:true }); } catch(e){ continue; }
    for (const х of ф){
      if (х.isFile()) всего++;
      else if (х.isDirectory()) стопка.push(join(п, х.name));
    }
    if (всего > 3000000) break;
  }
  return всего;
}

function доводы_проверки(папка){
  const экран = с => с.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return [
    '--stdout', '--no-summary', '-r',
    '--max-filesize=200M', '--max-scansize=400M',
    '--exclude-dir=^/(proc|sys|dev|run)(/|$)',
    `--exclude-dir=^${экран(КАРАНТИН)}`,
    `--exclude-dir=^${экран(ОБЩАЯ_БАЗА)}`,
    ...доводы_баз(папка),
  ];
}

/* Строка вывода clamscan: «путь: OK», «путь: Имя FOUND», «путь: Empty file» */
export function разбери_строку(стр){
  const и = стр.lastIndexOf(': ');
  if (и <= 0) return null;
  const путь = стр.slice(0, и), что = стр.slice(и + 2).trim();
  if (что.endsWith(' FOUND')) return { путь, угроза:что.slice(0, -6).replace(/\.UNOFFICIAL$/, '') };
  return { путь, угроза:null };
}

const ПРОГРАММЫ_БЫСТРОЙ = () => {
  const п = [join(ДОМ, 'Загрузки'), join(ДОМ, 'Downloads'), join(ДОМ, 'Рабочий стол'), join(ДОМ, 'Desktop'),
    join(ДОМ, '.config/autostart'), join(ДОМ, '.local/bin'), join(ДОМ, '.local/share/applications'),
    join(ДОМ, '.wine/drive_c/users'), '/tmp', '/var/tmp'];
  return п.filter(х => existsSync(х));
};

let работа = null;
let номер_работы = 0;

function снимок(р){
  if (!р) return { идёт:false };
  return {
    id:р.id, вид:р.вид, идёт:р.идёт, стадия:р.стадия, начато:р.начато, кончено:р.кончено || null,
    всего:р.всего, проверено:р.проверено, текущий:р.текущий, найдено:р.найдено,
    ошибка:р.ошибка || null, остановлена:!!р.остановлена,
  };
}

async function начни(пути, вид){
  if (работа?.идёт) throw new Error('проверка уже идёт');
  const нас = await н();
  if (!нас.вкл) throw new Error('«Защита» выключена');
  if (!await есть('clamscan')) throw new Error('в системе нет ClamAV');
  const папка = await где_база();
  if (!папка) throw new Error('база вирусов ещё не загружена — сначала обновите её');
  if (!пути.length) throw new Error('проверять нечего');
  const р = {
    id:++номер_работы, вид, пути, идёт:true, стадия:'база', начато:Date.now(),
    всего:0, проверено:0, текущий:'', найдено:[], процесс:null,
  };
  работа = р;
  сосчитай(пути, () => !р.идёт).then(в => { р.всего = Math.max(в, р.проверено); });
  // nice: проверка не должна отнимать машину у человека
  const п = spawn('nice', ['-n', '15', 'clamscan', ...доводы_проверки(папка), '--', ...пути],
    { stdio:['ignore', 'pipe', 'pipe'], detached:true });
  р.процесс = п;
  let хвост = '', ошибки = '';
  п.stdout.on('data', ч => {
    хвост += ч;
    const строки = хвост.split('\n');
    хвост = строки.pop();
    for (const с of строки){
      const з = разбери_строку(с);
      if (!з) continue;
      р.стадия = 'проверка';
      р.проверено++;
      р.текущий = з.путь;
      if (з.угроза) р.найдено.push({ путь:з.путь, угроза:з.угроза });
    }
  });
  п.stderr.on('data', ч => { ошибки = (ошибки + ч).slice(-4000); });
  п.on('error', е => { р.идёт = false; р.ошибка = String(е.message); });
  п.on('close', код => {
    р.идёт = false;
    р.кончено = Date.now();
    р.процесс = null;
    if (р.всего < р.проверено) р.всего = р.проверено;
    // 0 — чисто, 1 — нашлось; 2 — ошибки, но часть могла и пройти
    if (код === 2 && !р.проверено && !р.остановлена){
      р.ошибка = (ошибки.split('\n').find(х => /ERROR/.test(х)) || 'ClamAV не смог начать проверку').replace(/^.*ERROR:\s*/, '');
    }
    if (!р.остановлена && р.вид !== 'файлы'){
      нас.последняя = { когда:р.кончено, вид:р.вид, проверено:р.проверено, найдено:р.найдено.length };
      сохрани().catch(() => {});
    }
  });
  return снимок(р);
}

function останови(){
  if (работа?.идёт && работа.процесс){
    работа.остановлена = true;
    try { process.kill(-работа.процесс.pid, 'SIGTERM'); } catch(e){ try { работа.процесс.kill('SIGTERM'); } catch(_){} }
  }
}

/* Проверка одного файла перед установкой или запуском. По очереди: каждая
   проверка — это гигабайт памяти на загрузку базы, две разом старой машине
   не по силам. Чистый ответ помним, пока файл и база те же: второй запуск
   той же программы не ждёт ещё двадцать секунд. */
const ЧИСТЫЕ = new Map();
let очередь = Promise.resolve();

async function проверь_файл(путь){
  const нас = await н();
  if (!нас.вкл) return { выключена:true };
  if (!await есть('clamscan')) return { нет_clamav:true };
  const папка = await где_база();
  if (!папка) return { нет_базы:true };
  const с = await stat(путь);
  if (!с.isFile()) throw new Error('это не файл');
  const база = await о_базе();
  const ключ = `${путь}|${с.size}|${с.mtimeMs}|${база?.номер}`;
  if (ЧИСТЫЕ.has(ключ)) return { чисто:true, из_памяти:true };
  const сделай = async () => {
    const цели = [путь];
    let распак = null;
    // ClamAV не открывает .deb сам: внутри ar-архив, которого он не знает.
    // Распаковываем — вместе со сценариями установки, где вирусу и место
    if (/\.deb$/i.test(путь) && await есть('dpkg-deb')){
      await mkdir(ВРЕМЕННАЯ, { recursive:true });
      распак = join(ВРЕМЕННАЯ, `deb-${process.pid}-${Date.now()}`);
      const { код } = await выполни('dpkg-deb', ['-R', путь, распак], 5 * 60000);
      if (код === 0) цели.push(распак);
    }
    try {
      const { код, вывод } = await выполни('nice', ['-n', '10', 'clamscan', ...доводы_проверки(папка), '--', ...цели], 10 * 60000);
      const найдено = вывод.split('\n').map(разбери_строку).filter(х => х?.угроза)
        .map(х => ({ угроза:х.угроза, где:распак && х.путь.startsWith(распак) ? х.путь.slice(распак.length) : '' }));
      if (найдено.length) return { чисто:false, угроза:найдено[0].угроза, где:найдено[0].где, всего:найдено.length };
      if (код === 0){ ЧИСТЫЕ.set(ключ, true); return { чисто:true }; }
      throw new Error((вывод.split('\n').find(х => /ERROR/.test(х)) || 'проверка не удалась').replace(/^.*ERROR:\s*/, ''));
    } finally {
      if (распак) await rm(распак, { recursive:true, force:true }).catch(() => {});
    }
  };
  const итог = очередь.then(сделай, сделай);
  очередь = итог.catch(() => {});
  return итог;
}

/* ---------- карантин ----------
   Файл уезжает в скрытую папку и теряет все права: ни открыть, ни запустить.
   Рядом — где он лежал и что в нём нашли, чтобы можно было вернуть. */

async function перенеси(откуда, куда){
  try { await rename(откуда, куда); }
  catch(e){
    if (e.code !== 'EXDEV') throw e;
    await copyFile(откуда, куда);
    await unlink(откуда);
  }
}

function понятно(e){
  if (e.code === 'EACCES' || e.code === 'EPERM') return 'нет прав на этот файл — он системный';
  if (e.code === 'ENOENT') return 'файла уже нет';
  return String(e.message || e);
}

async function в_карантин(путь, угроза){
  const с = await lstat(путь).catch(e => { throw new Error(понятно(e)); });
  if (!с.isFile()) throw new Error('в карантин кладутся только файлы');
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const папка = join(КАРАНТИН, id);
  await mkdir(папка, { recursive:true, mode:0o700 });
  try { await перенеси(путь, join(папка, 'файл')); }
  catch(e){ await rm(папка, { recursive:true, force:true }); throw new Error(понятно(e)); }
  await chmod(join(папка, 'файл'), 0o000).catch(() => {});
  await writeFile(join(папка, 'о файле.json'), JSON.stringify({ путь, угроза:угроза || '', когда:Date.now(), режим:с.mode & 0o7777, размер:с.size }, null, 2));
  return { id };
}

async function список_карантина(){
  let ф = [];
  try { ф = await readdir(КАРАНТИН); } catch(e){ return []; }
  const list = [];
  for (const id of ф){
    try {
      const о = JSON.parse(await readFile(join(КАРАНТИН, id, 'о файле.json'), 'utf8'));
      list.push({ id, путь:о.путь, имя:basename(о.путь), угроза:о.угроза, когда:о.когда, размер:о.размер });
    } catch(e){}
  }
  return list.sort((а, б) => б.когда - а.когда);
}

function надёжный_id(id){
  if (!/^[0-9]+-[a-z0-9]+$/.test(String(id))) throw new Error('нет такой записи в карантине');
  return join(КАРАНТИН, id);
}

async function верни(id){
  const папка = надёжный_id(id);
  const о = JSON.parse(await readFile(join(папка, 'о файле.json'), 'utf8'));
  let куда = о.путь;
  // На месте уже лежит другой файл — не затираем, кладём рядом
  if (existsSync(куда)){
    const т = куда.lastIndexOf('.');
    const [имя, хвост] = т > куда.lastIndexOf('/') ? [куда.slice(0, т), куда.slice(т)] : [куда, ''];
    let н_ = 2;
    while (existsSync(`${имя} (${н_})${хвост}`)) н_++;
    куда = `${имя} (${н_})${хвост}`;
  }
  await mkdir(dirname(куда), { recursive:true });
  await chmod(join(папка, 'файл'), 0o600);
  try { await перенеси(join(папка, 'файл'), куда); }
  catch(e){ await chmod(join(папка, 'файл'), 0o000).catch(() => {}); throw new Error(понятно(e)); }
  await chmod(куда, о.режим || 0o644).catch(() => {});
  await rm(папка, { recursive:true, force:true });
  // Вернули сами — значит, доверяют: при запуске заново не спрашиваем
  try { const с = await stat(куда); const б = await о_базе(); ЧИСТЫЕ.set(`${куда}|${с.size}|${с.mtimeMs}|${б?.номер}`, true); } catch(e){}
  return { путь:куда };
}

/* Пакет clamav-freshclam Ubuntu заводит свою службу: она висит в памяти
   всегда и раз в час ходит за базой. Нам она не нужна — базу обновляем мы,
   раз в сутки и только пока «Защита» включена, — а выключенная «Защита»
   с работающей службой была бы обманом. Да и базу они делили бы, мешая
   друг другу. Гасим и не даём подняться снова. */
async function заглуши_службу(){
  try {
    const { stdout } = await run('systemctl', ['is-enabled', 'clamav-freshclam.service'], { timeout:5000 }).catch(e => ({ stdout:String(e.stdout || '') }));
    if (!/^(enabled|static|alias|indirect)/m.test(stdout) && !/active/.test(stdout)) return;
    if (!await можно_sudo()) return;
    await run('sudo', ['-n', 'systemctl', 'disable', '--now', 'clamav-freshclam.service'], { timeout:30000 });
    await run('sudo', ['-n', 'systemctl', 'mask', 'clamav-freshclam.service'], { timeout:30000 });
  } catch(e){}
}

/* ---------- методы агента ---------- */

export function protection(){
  setTimeout(() => { заглуши_службу(); }, 60000).unref();
  setTimeout(() => { пора(); setInterval(пора, 3600 * 1000).unref(); }, 5 * 60000).unref();
  return {
    async 'av.state'(){
      const нас = await н();
      const clamav = await есть('clamscan');
      const база = clamav ? await о_базе() : null;
      return {
        вкл:нас.вкл, clamav, freshclam:await есть('freshclam'),
        база, обновлено:нас.обновлено || null, ошибка:нас.ошибка || null,
        обновляется:!!обновляю, последняя:нас.последняя || null,
        проверка:снимок(работа), в_карантине:(await список_карантина()).length,
        живой:живой_сеанс(),
      };
    },

    async 'av.set'({ вкл }){
      const нас = await н();
      нас.вкл = !!вкл;
      await сохрани();
      if (!нас.вкл) останови();
      return { вкл:нас.вкл };
    },

    async 'av.update'(){
      if (!(await н()).вкл) throw new Error('«Защита» выключена');
      await заглуши_службу();
      return { база:await обнови() };
    },

    /* Убрать базу — для тех, кто «Защиту» выключил насовсем: это
       две-три сотни мегабайт на диске */
    async 'av.purge'(){
      if ((await н()).вкл) throw new Error('сначала выключите «Защиту»');
      await rm(СВОЯ_БАЗА, { recursive:true, force:true });
      const общие = await файлы_базы(ОБЩАЯ_БАЗА);
      if (общие.length && await можно_sudo()){
        const всё = [...общие, ...(await readdir(ОБЩАЯ_БАЗА).catch(() => [])).filter(ф => ф === 'freshclam.dat')];
        await run('sudo', ['-n', 'rm', '-f', '--', ...всё.map(ф => join(ОБЩАЯ_БАЗА, ф))]).catch(() => {});
      }
      const нас = await н();
      delete нас.обновлено;
      await сохрани();
      return { база:await о_базе() };
    },

    async 'av.scan'({ вид, пути }){
      let цели;
      if (вид === 'полная') цели = ['/'];
      else if (вид === 'быстрая') цели = ПРОГРАММЫ_БЫСТРОЙ();
      else {
        вид = 'файлы';
        цели = (Array.isArray(пути) ? пути : []).map(String).filter(х => х.startsWith('/') && existsSync(х));
      }
      return await начни(цели, вид);
    },

    async 'av.scan.state'(){ return снимок(работа); },

    async 'av.scan.stop'(){ останови(); return снимок(работа); },

    async 'av.check'({ путь }){
      if (!путь || !String(путь).startsWith('/')) throw new Error('нужен полный путь к файлу');
      return await проверь_файл(String(путь));
    },

    async 'av.quarantine'({ путь, угроза }){
      return await в_карантин(String(путь || ''), угроза);
    },

    async 'av.quarantine.list'(){ return { list:await список_карантина() }; },

    async 'av.quarantine.restore'({ id }){ return await верни(id); },

    async 'av.quarantine.delete'({ id }){
      const папка = надёжный_id(id);
      await rm(папка, { recursive:true, force:true });
      return { ok:true };
    },

    /* Удалить найденное сразу, мимо карантина */
    async 'av.delete'({ путь }){
      const п = String(путь || '');
      if (!п.startsWith('/')) throw new Error('нужен полный путь к файлу');
      const с = await lstat(п).catch(e => { throw new Error(понятно(e)); });
      if (!с.isFile()) throw new Error('удаляются только файлы');
      await unlink(п).catch(e => { throw new Error(понятно(e)); });
      return { ok:true };
    },
  };
}
