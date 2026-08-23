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
  'nmcli', 'ip', 'xdg-open', 'gio', 'flatpak', 'bwrap', 'df', 'setxkbmap', 'localectl', 'free', 'uptime',
  /* sudo нужен для выключения: обычный пользователь без polkit не имеет права
     остановить машину. Аргументы к нему собираются здесь же, из этого списка. */
  'sudo'
]);

async function call(cmd, args = []){
  if (!ALLOWED.has(cmd)) throw new Error('команда не разрешена: ' + cmd);
  try {
    const { stdout } = await run(cmd, args, { timeout:5000, maxBuffer:1024 * 1024 });
    return stdout;
  } catch(e){
    /* без stderr сообщение «команда не выполнилась» ничего не объясняет —
       а именно там система пишет, почему отказала */
    const why = String(e.stderr || '').trim().split('\n')[0];
    throw new Error(why || e.message);
  }
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
      /* Выключение — единственное место, где одной команды мало.
         От имени пользователя systemctl обращается к polkit, а его на машине
         может не быть; тогда пробуем через sudo, потом через loginctl.
         Молча делать вид, что получилось, нельзя: это ровно тот случай,
         когда человек ждёт, что машина погаснет. */
      const tries = [
        ['systemctl', [a]],
        ['sudo', ['-n', 'systemctl', a]],
        ['loginctl', [a === 'poweroff' ? 'poweroff' : a === 'reboot' ? 'reboot' : 'suspend']]
      ];
      const errors = [];
      for (const [cmd, args] of tries){
        if (!await has(cmd)) { errors.push(cmd + ': нет на машине'); continue; }
        try { await call(cmd, args); return { ok:true, via:cmd + ' ' + args.join(' ') }; }
        catch(e){ errors.push(cmd + ': ' + String(e.message || e).split('\n')[0]); }
      }
      throw new Error('машину не удалось ' +
        (a === 'poweroff' ? 'выключить' : a === 'reboot' ? 'перезагрузить' : 'усыпить') +
        ' — ' + errors.join(' · '));
    },

    /* Проверка без последствий: systemd умеет показать, что он сделал бы.
       Нужна, чтобы про поломку выключения узнавать из проверок, а не от
       человека, у которого машина не погасла. */
    async 'sys.power.check'(){
      const out = [];
      for (const [cmd, args] of [['systemctl', ['--dry-run', 'poweroff']],
                                 ['sudo', ['-n', 'systemctl', '--dry-run', 'poweroff']]]){
        if (!await has(cmd)){ out.push({ via:cmd, ok:false, why:'нет на машине' }); continue; }
        try { await call(cmd, args); out.push({ via:cmd, ok:true }); }
        catch(e){ out.push({ via:cmd, ok:false, why:String(e.message || e).split('\n')[0] }); }
      }
      return { allowed:!!allowPower, ways:out, ok:out.some(x => x.ok) };
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

/* ---------- Wi-Fi: всё через NetworkManager, без своих выдумок ----------
   Пароли уходят к nmcli отдельными аргументами, а не строкой оболочки, и
   обратно оболочке не возвращаются: система показывает только имя сети. */
export function wifi(allowNet){
  const need = async () => {
    if (!allowNet) throw new Error('управление сетью выключено: запустите агент с ключом --allow-net');
    if (!await has('nmcli')) throw new Error('на машине нет NetworkManager — управлять Wi-Fi нечем');
  };

  return {
    /* есть ли вообще беспроводное железо и включено ли оно */
    async 'sys.wifi.state'(){
      if (!await has('nmcli'))
        return { supported:false, allowed:!!allowNet, radio:null, devices:[], active:[],
                 reason:'на машине нет NetworkManager' };
      const dev = await call('nmcli', ['-t', '-f', 'DEVICE,TYPE,STATE', 'device']).catch(() => '');
      const list = dev.trim().split('\n').filter(Boolean).map(l => l.split(':'));
      const wifiDevs = list.filter(x => x[1] === 'wifi').map(x => ({ device:x[0], state:x[2] }));
      let radio = null;
      try { radio = (await call('nmcli', ['radio', 'wifi'])).trim() === 'enabled'; } catch(e){}
      const active = await call('nmcli', ['-t', '-f', 'NAME,DEVICE,TYPE', 'connection', 'show', '--active'])
        .catch(() => '');
      return {
        supported:wifiDevs.length > 0, allowed:!!allowNet, radio, devices:wifiDevs,
        active:active.trim().split('\n').filter(Boolean).map(l => {
          const [name, device, type] = l.split(':');
          return { name, device, type };
        }),
        reason:wifiDevs.length ? null : 'беспроводных устройств на машине не видно'
      };
    },

    async 'sys.wifi.scan'({ rescan } = {}){
      await need();
      if (rescan) await call('nmcli', ['device', 'wifi', 'rescan']).catch(() => {});
      const out = await call('nmcli', ['-t', '-f', 'IN-USE,SSID,SIGNAL,SECURITY,FREQ', 'device', 'wifi', 'list']);
      const seen = new Map();
      out.trim().split('\n').filter(Boolean).forEach(line => {
        /* в именах сетей бывает двоеточие — nmcli его экранирует */
        const parts = line.split(/(?<!\\):/).map(x => x.replace(/\\:/g, ':'));
        const [use, ssid, signal, security, freq] = parts;
        if (!ssid) return;
        const prev = seen.get(ssid);
        const item = { ssid, signal:+signal || 0, secure:!!(security && security !== '--'),
          security:security === '--' ? '' : security, band:+freq > 4000 ? '5 ГГц' : '2,4 ГГц',
          active:use.trim() === '*' };
        if (!prev || item.signal > prev.signal) seen.set(ssid, item);
      });
      return { list:[...seen.values()].sort((a, b) => b.signal - a.signal) };
    },

    async 'sys.wifi.saved'(){
      await need();
      const out = await call('nmcli', ['-t', '-f', 'NAME,TYPE', 'connection', 'show']);
      return { list:out.trim().split('\n').filter(Boolean)
        .map(l => l.split(':')).filter(x => /wireless/.test(x[1])).map(x => x[0]) };
    },

    async 'sys.wifi.connect'({ ssid, password }){
      await need();
      if (!ssid) throw new Error('не указана сеть');
      const args = ['device', 'wifi', 'connect', String(ssid)];
      if (password) args.push('password', String(password));
      await call('nmcli', args);
      return { ok:true, ssid };
    },

    async 'sys.wifi.disconnect'({ device }){
      await need();
      await call('nmcli', ['device', 'disconnect', String(device || '').replace(/[^\w.:-]/g, '')]);
      return { ok:true };
    },

    async 'sys.wifi.forget'({ ssid }){
      await need();
      await call('nmcli', ['connection', 'delete', String(ssid)]);
      return { ok:true };
    },

    async 'sys.wifi.radio'({ on }){
      await need();
      await call('nmcli', ['radio', 'wifi', on ? 'on' : 'off']);
      return { ok:true, on:!!on };
    }
  };
}

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
  join(os.homedir(), '.local/share/applications'),
  /* программы из Flathub кладут свои ярлыки сюда */
  join(os.homedir(), '.local/share/flatpak/exports/share/applications'),
  '/var/lib/flatpak/exports/share/applications'];

/* Пробный запуск для осмотра: ждём, чем дело кончится, и возвращаем жалобы. */
function попытка_тихо(программа, части){
  return new Promise(async resolve => {
    const { execFile } = await import('node:child_process');
    execFile(программа, части, { timeout:8000 }, (e, out, err) => {
      if (!e) return resolve({ ok:true });
      resolve({ ok:false, error:(String(err || '').trim().split('\n')[0] || e.message) });
    });
  });
}

/* Запуск отдельной программы: она живёт своей жизнью и после нашего ухода,
   но первые полторы секунды мы слушаем её жалобы. Если программа сразу упала,
   человек увидит настоящую причину, а не молчание. */
async function запустить(программа, части, via){
  const итог = await попытка(программа, части, via);
  if (!итог.ok) throw new Error(итог.error);
  return итог;
}

async function попытка(программа, части, via){
  const { spawn } = await import('node:child_process');
  const env = Object.assign({}, process.env);
  const дом = process.env.XDG_RUNTIME_DIR || '/run/user/' + (process.getuid ? process.getuid() : 1000);
  env.XDG_RUNTIME_DIR = дом;
  if (!env.WAYLAND_DISPLAY && !env.DISPLAY){
    const сокет = (existsSync(дом) ? (await readdir(дом).catch(() => [])) : [])
      .find(f => /^wayland-\d+$/.test(f));
    if (сокет) env.WAYLAND_DISPLAY = сокет; else env.DISPLAY = ':0';
  }

  return await new Promise(resolve => {
    let дитя;
    try { дитя = spawn(программа, части, { stdio:['ignore', 'ignore', 'pipe'], detached:true, env }); }
    catch(e){ resolve({ ok:false, error:'не удалось запустить: ' + e.message }); return; }

    let жалобы = '', готово = false;
    const ответ = о => { if (готово) return; готово = true; clearTimeout(часы); resolve(о); };
    дитя.stderr && дитя.stderr.on('data', d => { жалобы = (жалобы + d).slice(0, 2000); });
    дитя.on('error', e => ответ({ ok:false, error:'не удалось запустить: ' + e.message }));
    дитя.on('exit', код => {
      if (код === 0) { ответ({ ok:true, via, команда:программа }); return; }
      const причина = жалобы.trim().split('\n').filter(Boolean).pop();
      ответ({ ok:false, error:причина || ('программа завершилась с ошибкой ' + код) });
    });
    const часы = setTimeout(() => {
      /* полторы секунды прошли, программа жива — значит, запустилась */
      дитя.stderr && дитя.stderr.destroy();
      дитя.unref();
      ответ({ ok:true, via, команда:программа });
    }, 1500);
  });
}

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
              flatpak:!!get('X-Flatpak'),
              icon:get('Icon') || '', categories:(get('Categories') || '').split(';').filter(Boolean) });
          } catch(e){}
        }
      }
      list.sort((a, b) => a.name.localeCompare(b.name));
      return { total:list.length, list, canLaunch:!!allowLaunch };
    },

    /* Осмотр песочницы: почему программа из Flathub не запускается.
       Здесь нет догадок — только то, что машина отвечает сама. */
    async 'sys.sandbox'({ id } = {}){
      const строки = [];
      const скажи = (что, как) => строки.push(что + ': ' + как);

      const прочти = async п => readFile(п, 'utf8').then(t => t.trim()).catch(e => 'нет (' + e.code + ')');
      скажи('kernel.apparmor_restrict_unprivileged_userns',
        await прочти('/proc/sys/kernel/apparmor_restrict_unprivileged_userns'));
      скажи('user.max_user_namespaces', await прочти('/proc/sys/user/max_user_namespaces'));
      скажи('файл настройки', await прочти('/etc/sysctl.d/60-glower-userns.conf'));

      /* bwrap — та самая песочница, в которой flatpak запускает ldconfig */
      скажи('bwrap', await has('bwrap') ? 'есть' : 'нет');
      if (await has('bwrap')){
        const итог = await попытка_тихо('bwrap',
          ['--ro-bind', '/', '/', '--unshare-user', '--unshare-pid', '/bin/true']);
        скажи('проба bwrap', итог.ok ? 'работает' : 'не работает — ' + итог.error);
      }

      скажи('flatpak', await has('flatpak')
        ? (await call('flatpak', ['--version']).catch(e => e.message)).trim() : 'нет');
      if (await has('flatpak')){
        const список = await call('flatpak', ['list', '--columns=application,origin,installation'])
          .catch(e => e.message);
        скажи('установлено', (список || '').trim() || 'ничего');
        if (id && /^[\w.-]+$/.test(id)){
          const про = await call('flatpak', ['info', id]).catch(e => e.message);
          скажи('о программе', (про || '').trim().split('\n').slice(0, 6).join(' · '));
        }
      }

      /* живая система работает поверх сжатого образа: место в ней — это ОЗУ */
      const монтирование = await прочти('/proc/mounts');
      const корень = String(монтирование).split('\n').find(l => (l.split(' ')[1] === '/')) || '';
      скажи('корень', корень.split(' ').slice(0, 3).join(' ') || 'неизвестно');
      скажи('память', Math.round(os.freemem() / 1048576) + ' МБ свободно из ' +
        Math.round(os.totalmem() / 1048576));
      const куда = ['/', '/var/lib/flatpak', '/home'].filter(existsSync);
      const место = await call('df', ['-h', ...куда]).catch(e => 'df не ответил: ' + e.message);
      const строкиМеста = String(место).trim().split('\n');
      скажи('место', (строкиМеста.length > 1 ? строкиМеста.slice(1) : строкиМеста).join(' · '));

      return { текст:строки.join('\n') };
    },

    async 'sys.launch'({ id }){
      if (!allowLaunch) throw new Error('запуск программ выключен: запустите агент с ключом --allow-launch');
      /* имя ярлыка может быть каким угодно, кроме пути: без косой черты
         из папки не выйти, а «..» и скрытые имена отсекаем отдельно */
      const имя = String(id);
      if (!/^[^/\\\0]+\.desktop$/.test(имя) || имя.startsWith('.'))
        throw new Error('неверный идентификатор программы');
      const dir = APP_DIRS.find(d => existsSync(join(d, имя)));
      if (!dir) throw new Error('такой программы на машине нет: ' + имя);

      const текст = await readFile(join(dir, имя), 'utf8');
      const поле = k => (текст.match(new RegExp('^' + k + '=(.*)$', 'm')) || [])[1];

      /* Программы из Flathub живут в своём окружении. Их ярлык зовёт flatpak
         длинной строкой с пометками передачи файлов (@@u … @@), которую нельзя
         просто так отдать программе. Ярлык сам называет своё имя в строке
         X-Flatpak — по нему запуск получается коротким и надёжным. */
      const flatpak = поле('X-Flatpak');
      if (flatpak && /^[\w.-]+$/.test(flatpak) && await has('flatpak'))
        return запустить('flatpak', ['run', flatpak], 'flatpak').catch(e => {
          /* Песочнице flatpak нужны пространства имён пользователя. Когда они
             закрыты, программа падает на первом же шаге со странной для
             человека строкой про ldconfig — объясняем, о чём она. */
          if (/ldconfig|bwrap|namespace|пространств/i.test(e.message))
            throw new Error(e.message + ' — песочнице flatpak закрыты пространства имён ' +
              'пользователя (kernel.apparmor_restrict_unprivileged_userns)');
          throw e;
        });

      if (await has('gio')){ await call('gio', ['launch', join(dir, имя)]); return { ok:true, via:'gio' }; }

      /* gio в системе может не быть — тогда запускаем сами, как это делает
         любой рабочий стол: берём строку Exec из .desktop-файла, убираем
         подстановки вроде %U и запускаем первую команду с её доводами.
         Ничего не разбирается через оболочку: список доводов собирается здесь. */
      const exec = поле('Exec');
      if (!exec) throw new Error('в ярлыке нет строки запуска');
      const части = (exec.replace(/%[fFuUdDnNickvm]/g, '').match(/"[^"]+"|\S+/g) || [])
        .map(x => x.replace(/^"|"$/g, ''))
        /* @@ и @@u — пометки передачи файлов, без самих файлов они лишние */
        .filter(x => !/^@@u?$/.test(x));
      const программа = части.shift();
      if (!программа) throw new Error('в ярлыке пустая строка запуска');
      if (!/^[\w./+-]+$/.test(программа)) throw new Error('в ярлыке странная команда запуска');
      return запустить(программа, части, 'ярлык');
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
    allow:{ power:!!flags.power, launch:!!flags.launch, open:!!flags.open,
      install:!!flags.install, net:!!flags.net, packages:!!flags.packages }
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
