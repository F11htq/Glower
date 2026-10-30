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

export function games(){
  // Агент перезапустился посреди игры — gamemode не должен остаться включённым навсегда
  process.on('exit', () => { if (держит) try { process.kill(-держит.pid, 'SIGTERM'); } catch(e){} });
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
