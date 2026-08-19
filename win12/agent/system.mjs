/* ==========================================================================
   Системный модуль агента: связь с настоящим Linux

   Всё, что здесь есть, выполняется на живой машине, поэтому правила жёсткие:
   — никаких строк, уходящих в оболочку: только execFile со списком аргументов;
   — каждая команда берётся из белого списка, произвольное исполнение невозможно;
   — опасное (выключение, перезагрузка) требует отдельного ключа запуска;
   — чего на машине нет, о том честно сообщается, а не подделывается.
   ========================================================================== */
import { execFile } from 'node:child_process';
import { readFile, readdir } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);

/* какие программы вообще есть на машине */
const have = {};
export async function has(cmd){
  if (cmd in have) return have[cmd];
  try { await run('which', [cmd]); have[cmd] = true; }
  catch(e){ have[cmd] = false; }
  return have[cmd];
}

/* единственная точка запуска: только фиксированные команды */
const ALLOWED = new Set([
  'which', 'systemctl', 'loginctl', 'wpctl', 'pactl', 'amixer', 'brightnessctl',
  'nmcli', 'ip', 'xdg-open', 'gio', 'setxkbmap', 'localectl', 'free', 'uptime'
]);

async function call(cmd, args = []){
  if (!ALLOWED.has(cmd)) throw new Error('команда не разрешена: ' + cmd);
  const { stdout } = await run(cmd, args, { timeout:5000, maxBuffer:1024 * 1024 });
  return stdout;
}

/* ---------- питание ---------- */
export function power(allowPower){
  return {
    async 'sys.power'({ action }){
      if (!allowPower) throw new Error('управление питанием выключено: запустите агент с ключом --allow-power');
      const map = { poweroff:'poweroff', reboot:'reboot', suspend:'suspend', lock:'lock' };
      const a = map[action];
      if (!a) throw new Error('неизвестное действие: ' + action);
      if (a === 'lock'){
        if (await has('loginctl')) { await call('loginctl', ['lock-session']); return { ok:true, via:'loginctl' }; }
        throw new Error('на машине нет loginctl — блокировать нечем');
      }
      await call('systemctl', [a]);
      return { ok:true, via:'systemctl ' + a };
    }
  };
}

/* ---------- звук ---------- */
export const sound = {
  async 'sys.volume.get'(){
    if (await has('wpctl')){
      const out = await call('wpctl', ['get-volume', '@DEFAULT_AUDIO_SINK@']);
      const m = out.match(/([\d.]+)/);
      return { volume: m ? Math.round(+m[1] * 100) : null, muted:/MUTED/.test(out), via:'wireplumber' };
    }
    if (await has('amixer')){
      const out = await call('amixer', ['get', 'Master']);
      const m = out.match(/\[(\d+)%\]/);
      return { volume: m ? +m[1] : null, muted:/\[off\]/.test(out), via:'alsa' };
    }
    return { volume:null, muted:null, via:null, reason:'на машине нет ни wpctl, ни amixer' };
  },
  async 'sys.volume.set'({ volume }){
    const v = Math.max(0, Math.min(100, Math.round(+volume)));
    if (await has('wpctl')){ await call('wpctl', ['set-volume', '@DEFAULT_AUDIO_SINK@', (v / 100).toFixed(2)]); return { ok:true, volume:v }; }
    if (await has('amixer')){ await call('amixer', ['set', 'Master', v + '%']); return { ok:true, volume:v }; }
    throw new Error('менять громкость нечем: нет wpctl и amixer');
  }
};

/* ---------- яркость ---------- */
const BL = '/sys/class/backlight';
export const backlight = {
  async 'sys.brightness.get'(){
    if (!existsSync(BL)) return { value:null, reason:'у машины нет управляемой подсветки' };
    const dirs = await readdir(BL).catch(() => []);
    if (!dirs.length) return { value:null, reason:'подсветка не найдена' };
    const d = join(BL, dirs[0]);
    const cur = +(await readFile(join(d, 'brightness'), 'utf8')).trim();
    const max = +(await readFile(join(d, 'max_brightness'), 'utf8')).trim();
    return { value: Math.round(cur / max * 100), device:dirs[0] };
  },
  async 'sys.brightness.set'({ value }){
    if (!await has('brightnessctl')) throw new Error('нет brightnessctl — менять яркость нечем');
    const v = Math.max(1, Math.min(100, Math.round(+value)));
    await call('brightnessctl', ['set', v + '%']);
    return { ok:true, value:v };
  }
};

/* ---------- сеть ---------- */
export const net = {
  async 'sys.net'(){
    if (await has('nmcli')){
      const dev = await call('nmcli', ['-t', '-f', 'DEVICE,TYPE,STATE,CONNECTION', 'device']);
      const list = dev.trim().split('\n').filter(Boolean).map(l => {
        const [device, type, state, connection] = l.split(':');
        return { device, type, state, connection };
      });
      return { via:'NetworkManager', devices:list };
    }
    if (await has('ip')){
      const out = await call('ip', ['-o', 'link']);
      const list = out.trim().split('\n').map(l => {
        const m = l.match(/^\d+:\s+([^:]+):.*state (\w+)/);
        return m ? { device:m[1], state:m[2].toLowerCase(), type:'', connection:'' } : null;
      }).filter(Boolean);
      return { via:'ip', devices:list };
    }
    return { via:null, devices:[], reason:'на машине нет ни nmcli, ни ip' };
  }
};

/* ---------- процессы: читаем /proc, ничего не выдумываем ---------- */
export const procs = {
  async 'sys.procs'(){
    const dirs = (await readdir('/proc')).filter(d => /^\d+$/.test(d));
    const tick = 100;                     // USER_HZ на обычном ядре
    const up = +(await readFile('/proc/uptime', 'utf8')).split(' ')[0];
    const page = 4096;
    const out = [];
    for (const pid of dirs){
      try {
        const stat = await readFile(`/proc/${pid}/stat`, 'utf8');
        const name = stat.slice(stat.indexOf('(') + 1, stat.lastIndexOf(')'));
        const rest = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
        const utime = +rest[11], stime = +rest[12], start = +rest[19];
        const rss = +rest[21] * page;
        const life = up - start / tick;
        out.push({ pid:+pid, name,
          cpu: life > 0 ? +(((utime + stime) / tick) / life * 100).toFixed(1) : 0,
          mem: rss, lifetime: Math.round(life) });
      } catch(e){}
    }
    out.sort((a, b) => b.mem - a.mem);
    return { total:out.length, list:out.slice(0, 60),
      mem:{ total:os.totalmem(), free:os.freemem() }, load:os.loadavg() };
  }
};

/* ---------- установленные программы: читаем .desktop ---------- */
const APP_DIRS = ['/usr/share/applications', '/usr/local/share/applications',
  join(os.homedir(), '.local/share/applications')];

export function apps(allowLaunch){
  return {
    async 'sys.apps'(){
      const list = [];
      for (const dir of APP_DIRS){
        if (!existsSync(dir)) continue;
        for (const f of await readdir(dir).catch(() => [])){
          if (!f.endsWith('.desktop')) continue;
          try {
            const t = await readFile(join(dir, f), 'utf8');
            const get = k => (t.match(new RegExp('^' + k + '=(.*)$', 'm')) || [])[1];
            if (get('NoDisplay') === 'true' || get('Hidden') === 'true') continue;
            const name = get('Name');
            if (!name) continue;
            list.push({ id:f, name, comment:get('Comment') || '',
              icon:get('Icon') || '', categories:(get('Categories') || '').split(';').filter(Boolean) });
          } catch(e){}
        }
      }
      list.sort((a, b) => a.name.localeCompare(b.name));
      return { total:list.length, list, canLaunch:!!allowLaunch };
    },

    async 'sys.launch'({ id }){
      if (!allowLaunch) throw new Error('запуск программ выключен: запустите агент с ключом --allow-launch');
      if (!/^[\w.+-]+\.desktop$/.test(String(id))) throw new Error('неверный идентификатор программы');
      const dir = APP_DIRS.find(d => existsSync(join(d, id)));
      if (!dir) throw new Error('такой программы на машине нет: ' + id);
      if (await has('gio')){ await call('gio', ['launch', join(dir, id)]); return { ok:true, via:'gio' }; }
      throw new Error('нет gio — запускать программы нечем');
    }
  };
}

/* ---------- что вообще умеет эта машина ---------- */
export async function capabilities(flags){
  return {
    host:os.hostname(), user:os.userInfo().username,
    platform:process.platform, release:os.release(), arch:os.arch(),
    desktop:process.env.XDG_CURRENT_DESKTOP || process.env.DESKTOP_SESSION || null,
    session:process.env.XDG_SESSION_TYPE || null,
    tools:{
      systemctl: await has('systemctl'), loginctl: await has('loginctl'),
      wpctl: await has('wpctl'), amixer: await has('amixer'),
      brightnessctl: await has('brightnessctl'), nmcli: await has('nmcli'),
      gio: await has('gio'), xdgOpen: await has('xdg-open')
    },
    backlight: existsSync(BL) && (await readdir(BL).catch(() => [])).length > 0,
    allow:{ power:!!flags.power, launch:!!flags.launch, open:!!flags.open }
  };
}

/* ---------- железо: то, что машина говорит о себе сама ---------- */
const readOr = async (p, d = '') => readFile(p, 'utf8').then(s => s.trim()).catch(() => d);

export const hardware = {
  async 'sys.hardware'(){
    const cpuinfo = await readOr('/proc/cpuinfo');
    const model = (cpuinfo.match(/^model name\s*:\s*(.+)$/m) || [])[1]
      || (cpuinfo.match(/^Model\s*:\s*(.+)$/m) || [])[1] || null;
    const osrel = await readOr('/etc/os-release');
    const pretty = (osrel.match(/^PRETTY_NAME="?([^"\n]+)"?$/m) || [])[1] || null;

    /* имя машины: DMI есть не везде, поэтому мягко */
    const vendor = await readOr('/sys/class/dmi/id/sys_vendor');
    const product = await readOr('/sys/class/dmi/id/product_name');

    return {
      cpu:{ model, cores:os.cpus().length, arch:os.arch() },
      mem:{ total:os.totalmem(), free:os.freemem() },
      kernel:os.release(), distro:pretty, uptime:Math.round(os.uptime()),
      machine:[vendor, product].filter(Boolean).join(' ') || null,
      host:os.hostname()
    };
  },

  /* батарея — из /sys, а не из Battery API страницы */
  async 'sys.battery'(){
    const base = '/sys/class/power_supply';
    if (!existsSync(base)) return { present:false, reason:'машина не сообщает о питании' };
    const names = await readdir(base).catch(() => []);
    let bat = null, ac = null;
    for (const n of names){
      const type = await readOr(join(base, n, 'type'));
      if (type === 'Battery' && !bat){
        bat = { name:n,
          level:+(await readOr(join(base, n, 'capacity'), '')) || null,
          status:(await readOr(join(base, n, 'status'))) || null };
      }
      if (type === 'Mains' && !ac) ac = { name:n, online:(await readOr(join(base, n, 'online'))) === '1' };
    }
    if (!bat && !ac) return { present:false, reason:'ни батареи, ни сетевого адаптера не видно' };
    return { present:!!bat, battery:bat, ac, charging: bat ? bat.status === 'Charging' : !!(ac && ac.online) };
  },

  /* устройства: камеры, звуковые карты, Bluetooth — из /sys и /proc */
  async 'sys.devices'(){
    const cams = [];
    const v4l = '/sys/class/video4linux';
    for (const n of (existsSync(v4l) ? await readdir(v4l).catch(() => []) : []))
      cams.push({ id:n, name:(await readOr(join(v4l, n, 'name'))) || n });

    const cards = [];
    const asound = await readOr('/proc/asound/cards');
    for (const line of asound.split('\n')){
      const m = line.match(/^\s*\d+\s+\[[^\]]+\]:\s*(.+)$/);
      if (m) cards.push(m[1].trim());
    }

    const bt = [];
    const btDir = '/sys/class/bluetooth';
    for (const n of (existsSync(btDir) ? await readdir(btDir).catch(() => []) : [])){
      if (!/^hci\d+$/.test(n)) continue;
      bt.push({ id:n, address:(await readOr(join(btDir, n, 'address'))) || null });
    }

    return { cameras:cams, sound:cards, bluetooth:bt };
  }
};
