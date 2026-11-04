/* ==========================================================================
   Игры: игровой режим и счётчик кадров

   Игровой режим включает сама панель: она видит окна и узнаёт игру — окно
   Steam-игры (steam_app_…), программу Windows (.exe) или незнакомую
   программу во весь экран. Тогда:
     — gamemode (Feral) переводит процессор на полную частоту и поднимает
       приоритет: держим его включённым, пока идёт игра, через
       «gamemoderun sleep» — сама игра для этого ничего делать не должна;
     — «Защита» не начинает обновлять базу и проверять, система не ищет
       обновления — всё это ждёт, пока игра не закроется;
     — уведомления молчат (это делает панель).
   Закрыли игру — всё возвращается как было.

   Счётчик кадров — MangoHud. Включается для всех игр сразу: и для
   установленных в систему (переменная MANGOHUD=1 в сеансе), и для Steam и
   прочего из Flathub (общая настройка flatpak). Нужен перезапуск игры.
   ========================================================================== */
import { spawn, execFile } from 'node:child_process';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);
const ДОМ = os.homedir();
const СРЕДА_СЕАНСА = join(ДОМ, '.config/glower/session-env');
const MANGOHUD_CONF = join(ДОМ, '.config/MangoHud/MangoHud.conf');

let игра = false;
let держит = null;   // gamemoderun sleep — пока он жив, gamemode включён

/* Идёт ли сейчас игра — для «Защиты» и обновлений: им ждать */
export function играИдёт(){ return игра; }

async function есть(cmd){
  try { await run('which', [cmd]); return true; } catch(e){ return false; }
}

async function режим(вкл){
  игра = !!вкл;
  // gamemoderun есть не везде (в Ubuntu 24.04 его нет), а сама служба умеет
  // то же: «gamemoded -r» просит режим и держит его, пока жив
  const как = await есть('gamemoderun') ? ['gamemoderun', ['sleep', '86400']]
            : await есть('gamemoded') ? ['gamemoded', ['-r']] : null;
  if (вкл && !держит && как){
    держит = spawn(как[0], как[1], { stdio:'ignore', detached:true });
    держит.on('exit', () => { держит = null; });
    держит.unref();
  }
  if (!вкл && держит){
    try { process.kill(-держит.pid, 'SIGTERM'); } catch(e){ try { держит.kill(); } catch(_){} }
    держит = null;
  }
}

async function fps_вкл(){
  try { return /^MANGOHUD=1$/m.test(await readFile(СРЕДА_СЕАНСА, 'utf8')); } catch(e){ return false; }
}

/* Строка в файле переменных сеанса: остальное в нём не трогаем */
async function переменная_сеанса(имя, значение){
  let строки = [];
  try { строки = (await readFile(СРЕДА_СЕАНСА, 'utf8')).split('\n').filter(Boolean); } catch(e){}
  строки = строки.filter(с => !с.startsWith(имя + '='));
  if (значение !== null) строки.push(`${имя}=${значение}`);
  await mkdir(dirname(СРЕДА_СЕАНСА), { recursive:true });
  await writeFile(СРЕДА_СЕАНСА, строки.join('\n') + (строки.length ? '\n' : ''));
}

/* Всё, что ускоряет игры, кроме самого игрового режима, — один раз при
   запуске агента, тихо и без вопросов:
     — свой gamemode.ini: игре приоритет выше остальных программ, встроенной
       видеокарте — полная частота, пока идёт игра;
     — человек — в группе gamemode: без неё поднять приоритет нельзя
       (заработает со следующего входа);
     — Prism Launcher из Flathub: OpenGL в отдельном потоке (glthread).
       Системным играм это даёт /usr/share/drirc.d/50-glower-games.conf, а
       Mesa внутри Flatpak этот файл не видит. */
const GAMEMODE_INI = join(ДОМ, '.config/gamemode.ini');
export const GRUB_PERF = '/etc/default/grub.d/90-glower-perf.cfg';
export const МИТИГАЦИИ = '# GlowerOS: защиты процессора от Spectre и подобных выключены по просьбе человека\n'
  + '# (Параметры → Игры). Удалить файл и выполнить update-grub — вернуть как было\n'
  + 'GRUB_CMDLINE_LINUX_DEFAULT="$GRUB_CMDLINE_LINUX_DEFAULT mitigations=off"\n';
const PRISM = 'org.prismlauncher.PrismLauncher';
export async function подготовь_ускорение({ запусти = run } = {}){
  const сделано = [];
  if (!existsSync(GAMEMODE_INI)){
    try {
      await mkdir(dirname(GAMEMODE_INI), { recursive:true });
      await writeFile(GAMEMODE_INI, await readFile(new URL('./gamemode.ini', import.meta.url), 'utf8'));
      сделано.push('gamemode.ini');
    } catch(e){}
  }
  try {
    const группы = await readFile('/etc/group', 'utf8');
    const строка = группы.split('\n').find(с => с.startsWith('gamemode:'));
    const я = os.userInfo().username;
    if (строка && !строка.split(':')[3].split(',').includes(я)){
      await запусти('sudo', ['-n', 'usermod', '-aG', 'gamemode', я], { timeout:15000 });
      сделано.push('группа gamemode');
    }
  } catch(e){}
  try {
    const { stdout } = await запусти('flatpak', ['list', '--app', '--columns=application'], { timeout:15000 });
    if (String(stdout).split('\n').includes(PRISM)){
      await запусти('flatpak', ['override', '--user', '--env=mesa_glthread=true', PRISM], { timeout:15000 });
      сделано.push('glthread для Prism Launcher');
    }
  } catch(e){}
  return сделано;
}

export function games(){
  // Агент перезапустился посреди игры — gamemode не должен остаться включённым навсегда
  process.on('exit', () => { if (держит) try { process.kill(-держит.pid, 'SIGTERM'); } catch(e){} });
  const т = setTimeout(() => { подготовь_ускорение().catch(() => {}); }, 20000);
  if (т.unref) т.unref();
  return {
    async 'game.mode'({ вкл }){
      await режим(вкл);
      return { игра, gamemode:!!держит };
    },

    async 'game.state'(){
      const [gm1, gm2, mangohud, vulkaninfo] = await Promise.all([есть('gamemoderun'), есть('gamemoded'), есть('mangohud'), есть('vulkaninfo')]);
      const gamemode = gm1 || gm2;
      let карта = 0;
      try { карта = Number((await readFile('/proc/sys/vm/max_map_count', 'utf8')).trim()) || 0; } catch(e){}
      let vulkan32 = false, nvidia = null;
      try {
        // Архитектуру — явно: без неё dpkg-query отвечает только про 64-битный пакет
        const { stdout } = await run('dpkg-query', ['-W', '-f=${Package}:${Architecture} ${Status}\n', 'mesa-vulkan-drivers:i386', 'nvidia-driver-*'])
          .catch(e => ({ stdout:String(e.stdout || '') }));
        vulkan32 = /mesa-vulkan-drivers:i386 install ok installed/.test(stdout);
        nvidia = (stdout.split('\n').find(с => /^nvidia-driver-\d+.* install ok installed$/.test(с)) || '').split(':')[0] || null;
      } catch(e){}
      let steam = false;
      try { steam = /com\.valvesoftware\.Steam/.test((await run('flatpak', ['list', '--app', '--columns=application'], { timeout:10000 })).stdout); } catch(e){}
      const готово = gamemode && mangohud && vulkan32 && карта >= 1048576;
      return { игра, gamemode, mangohud, vulkaninfo, vulkan32, nvidia, steam, карта, готово,
        fps:await fps_вкл() };
    },

    /* Максимум для игр: состояние и защиты процессора.

       Защиты ядра от Spectre, Meltdown и родни на Intel до 11-го поколения
       отнимают у программ, упирающихся в процессор, от 5 до 15 % (а то и
       больше): каждое переключение в ядро и обратно дороже. Отключить —
       выбор человека, только по его просьбе: в обмен вредоносная программа
       или страница теоретически сможет читать чужую память. Ключ ядра
       mitigations=off пишем отдельным файлом в /etc/default/grub.d —
       убрать его значит удалить файл. Работает после перезагрузки. */
    async 'game.perf'(){
      const ядро = await readFile('/proc/cmdline', 'utf8').catch(() => '');
      const сейчас_выкл = /(^|\s)mitigations=off(\s|$)/.test(ядро);
      const файл = existsSync(GRUB_PERF);
      let режим = '';
      try { режим = (await run('gamemoded', ['-s'], { timeout:5000 })).stdout.trim(); } catch(e){ режим = String(e.stdout || '').trim(); }
      const частота = await readFile('/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor', 'utf8').then(т => т.trim()).catch(() => '');
      return { защиты_выкл:сейчас_выкл, защиты_выкл_после_перезагрузки:файл,
        перезагрузка:сейчас_выкл !== файл, игровой_режим:/is active/.test(режим), частота, игра };
    },

    async 'game.perf.mitigations'({ выкл }){
      if (выкл){
        await run('sudo', ['-n', 'mkdir', '-p', dirname(GRUB_PERF)], { timeout:15000 });
        const п = spawn('sudo', ['-n', 'tee', GRUB_PERF], { stdio:['pipe', 'ignore', 'pipe'] });
        п.stdin.end(МИТИГАЦИИ);
        const код = await new Promise(r => п.on('close', r));
        if (код !== 0) throw new Error('не вышло записать настройку загрузки');
      } else {
        await run('sudo', ['-n', 'rm', '-f', GRUB_PERF], { timeout:15000 });
      }
      await run('sudo', ['-n', 'update-grub'], { timeout:180000 })
        .catch(e => { throw new Error('update-grub: ' + String(e.stderr || e.message).trim().split('\n').pop()); });
      return await this['game.perf']();
    },

    /* Счётчик кадров во всех играх */
    async 'game.fps'({ вкл }){
      if (!existsSync(MANGOHUD_CONF)){
        // Коротко и в углу: кадры, время кадра, загрузка процессора и видеокарты.
        // Shift+F12 прячет и показывает счётчик прямо в игре
        await mkdir(dirname(MANGOHUD_CONF), { recursive:true });
        await writeFile(MANGOHUD_CONF, [
          'position=top-left', 'fps', 'frametime', 'cpu_stats', 'gpu_stats', 'ram',
          'font_size=20', 'background_alpha=0.4', 'round_corners=8',
          'toggle_hud=Shift_R+F12', ''].join('\n'));
      }
      await переменная_сеанса('MANGOHUD', вкл ? '1' : null);
      // Steam и всё из Flathub: общая настройка для всех программ оттуда
      if (await есть('flatpak')){
        const доводы = вкл ? ['override', '--user', '--env=MANGOHUD=1', '--filesystem=xdg-config/MangoHud:ro']
                           : ['override', '--user', '--unset-env=MANGOHUD'];
        await run('flatpak', доводы, { timeout:15000 }).catch(() => {});
      }
      return { fps:await fps_вкл() };
    },
  };
}
