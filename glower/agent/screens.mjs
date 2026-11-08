/* ==========================================================================
   Один режим экрана — от включения до рабочего стола

   Монитор гаснет и заново ловит сигнал всякий раз, когда меняется режим
   экрана. При включении экран по очереди показывают три программы:
   заставка загрузки (Plymouth, режим выбирает ядро), экран входа и
   рабочий стол (glower-wm). Ядро и экран входа брали «предпочтительный»
   режим монитора, рабочий стол — выбранный человеком в Параметрах
   (например, 75 Гц вместо 60), и монитор мигал дважды.

   Выбор человека glower-wm хранит в ~/.config/glower/screens. Отсюда его
   копируем туда, где его видят все:
     — /etc/glower/screens — для экрана входа (домашняя папка ему закрыта);
     — ключ ядра video=РАЗЪЁМ:ШxВ@Гц (/etc/default/grub.d/91-glower-video.cfg)
       — чтобы и заставка загрузки шла в том же режиме.
   Если выбора нет, glower-wm сам держит тот режим, что уже стоит, — и
   ничего не переключает.

   Сверяемся при запуске и когда файл выбора меняется.
   ========================================================================== */
import { execFile, spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { existsSync, watch } from 'node:fs';
import { join, dirname } from 'node:path';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);
const СВОЙ = join(process.env.XDG_CONFIG_HOME || join(os.homedir(), '.config'), 'glower', 'screens');
export const ОБЩИЙ = process.env.GLOWER_SCREENS_SYS || '/etc/glower/screens';
export const GRUB_VIDEO = '/etc/default/grub.d/91-glower-video.cfg';

/* Строки «РАЗЪЁМ ШxВ@мГц масштаб» — только с режимом и только разумные */
export function разбери(текст){
  const итог = [];
  for (const с of String(текст).split('\n')){
    const м = с.trim().match(/^([A-Za-z][A-Za-z0-9-]*-\d+)\s+(\d{3,5})x(\d{3,5})@(\d{4,6})(?:\s+([0-9.]+))?$/);
    if (!м) continue;
    итог.push({ имя:м[1], ш:+м[2], в:+м[3], мгц:+м[4], масштаб:м[5] ? +м[5] : 1 });
  }
  return итог;
}

export function собери_общий(экраны){
  return экраны.map(э => `${э.имя} ${э.ш}x${э.в}@${э.мгц} ${э.масштаб}`).join('\n') + (экраны.length ? '\n' : '');
}

/* video=HDMI-A-1:1920x1080@75 — ядро берёт режим с этой частотой,
   округлённой до целых герц (74,97 → 75), как и считает её само */
export function ключи_ядра(экраны){
  return экраны.map(э => `video=${э.имя}:${э.ш}x${э.в}@${Math.round(э.мгц / 1000)}`).join(' ');
}

export function собери_grub(экраны){
  return '# GlowerOS: режим экрана, выбранный в Параметрах → Дисплей, — с самого\n'
    + '# включения, чтобы монитор не мигал, переключаясь. Удалить файл и\n'
    + '# выполнить update-grub — вернуть как было\n'
    + `GRUB_CMDLINE_LINUX_DEFAULT="$GRUB_CMDLINE_LINUX_DEFAULT ${ключи_ядра(экраны)}"\n`;
}

async function запиши_под_sudo(путь, текст){
  await run('sudo', ['-n', 'mkdir', '-p', dirname(путь)], { timeout:15000 });
  const п = spawn('sudo', ['-n', 'tee', путь], { stdio:['pipe', 'ignore', 'pipe'] });
  п.stdin.end(текст);
  const код = await new Promise(р => п.on('close', р));
  if (код !== 0) throw new Error('не вышло записать ' + путь);
}

export function screens(){
  let идёт = null, итог = null;

  async function сверь(){
    const свои = разбери(await readFile(СВОЙ, 'utf8').catch(() => ''));
    // Выбора нет — нечего и передавать: glower-wm держит тот режим, что стоит
    if (!свои.length) return { изменено:false, причина:'режим не выбирали' };
    const общий = собери_общий(свои);
    const был = await readFile(ОБЩИЙ, 'utf8').catch(() => '');
    const grub = собери_grub(свои);
    const был_grub = await readFile(GRUB_VIDEO, 'utf8').catch(() => '');
    if (был === общий && был_grub === grub) return { изменено:false };
    if (был !== общий) await запиши_под_sudo(ОБЩИЙ, общий);
    // Загрузчик — только там, где он наш: на стенде и в контейнере его нет
    if (был_grub !== grub && existsSync('/usr/sbin/update-grub')){
      await запиши_под_sudo(GRUB_VIDEO, grub);
      await run('sudo', ['-n', 'update-grub'], { timeout:180000 })
        .catch(e => { throw new Error('update-grub: ' + String(e.stderr || e.message).trim().split('\n').pop()); });
    }
    return { изменено:true, ядро:ключи_ядра(свои) };
  }

  function сверь_раз(){
    if (идёт) return идёт;
    идёт = сверь().then(р => (итог = р), е => (итог = { ошибка:String(е.message || е) }))
      .finally(() => { идёт = null; });
    return идёт;
  }

  // При запуске — чуть погодя, чтобы не мешать входу; потом — по изменению
  const т = setTimeout(() => сверь_раз(), 20000);
  if (т.unref) т.unref();
  let ждём = null;
  try {
    const н = watch(dirname(СВОЙ), (_, имя) => {
      if (имя !== 'screens') return;
      clearTimeout(ждём);
      ждём = setTimeout(() => сверь_раз(), 3000);
      if (ждём.unref) ждём.unref();
    });
    н.on('error', () => {});
    if (н.unref) н.unref();
  } catch(e){}

  return {
    async 'sys.screens.sync'(){ await сверь_раз(); return итог || {}; }
  };
}
