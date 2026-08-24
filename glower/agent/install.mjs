/* ==========================================================================
   Установка системы на диск

   Самая опасная часть агента: она стирает диск целиком. Поэтому здесь
   особенно жёстко:
     — запускается только один известный сценарий /usr/bin/glower-install,
       никаких строк оболочки и никаких чужих путей;
     — работает лишь при явном ключе запуска --allow-install;
     — имя диска проверяется по списку настоящих дисков машины, а не
       принимается на слово;
     — установка идёт в одном экземпляре, ход виден оболочке построчно.
   ========================================================================== */
import { spawn, execFile } from 'node:child_process';
import { existsSync, realpathSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { promisify } from 'node:util';

const run = promisify(execFile);
const SCRIPT = '/usr/bin/glower-install';

/* Сценарий размечает диски, поэтому требует root, а агент работает от имени
   обычного пользователя. Значит, звать его надо через sudo — в системе для
   этого заранее разрешён беспарольный вызов. Ключ -n важен: если разрешения
   вдруг нет, sudo откажет сразу, а не подвиснет, ожидая пароль в пустоту. */
const RUNNER = process.getuid && process.getuid() === 0
  ? { cmd:SCRIPT, pre:[] }
  : { cmd:'sudo', pre:['-n', SCRIPT] };

/* диски машины: читаем /sys, чтобы не зависеть от чужих программ */
export async function disks(){
  const out = [];
  const { readdir } = await import('node:fs/promises');
  for (const name of await readdir('/sys/block').catch(() => [])){
    if (/^(loop|ram|sr|zram|md|dm-)/.test(name)) continue;
    const base = '/sys/block/' + name;
    const size = +(await readFile(base + '/size', 'utf8').catch(() => '0')) * 512;
    if (!size) continue;
    const removable = (await readFile(base + '/removable', 'utf8').catch(() => '0')).trim() === '1';
    const model = (await readFile(base + '/device/model', 'utf8').catch(() => '')).trim();
    const rot = (await readFile(base + '/queue/rotational', 'utf8').catch(() => '')).trim() === '1';
    out.push({ dev:'/dev/' + name, name, size, removable, model, kind:rot ? 'диск' : 'твердотельный' });
  }
  out.sort((a, b) => b.size - a.size);
  return out;
}

/* носитель, с которого сейчас запущена живая система, — его предлагать нельзя */
async function liveDevice(){
  try {
    const mounts = await readFile('/proc/mounts', 'utf8');
    const line = mounts.split('\n').find(l => / \/run\/live\/medium | \/cdrom /.test(l));
    return line ? line.split(' ')[0] : null;
  } catch(e){ return null; }
}

/* Уже установленная система опознаётся по метке раздела — её ставит сам
   установщик. Никаких внешних программ для этого не нужно. */
function installedRoot(){
  const link = '/dev/disk/by-label/GlowerOS';
  if (!existsSync(link)) return null;
  try { return realpathSync(link); } catch(e){ return link; }
}

export function install(allowInstall){
  let job = null;      // { disk, percent, step, done, error, log }

  return {
    async 'install.can'(){
      const live = existsSync('/run/live/medium/live/filesystem.squashfs')
        || existsSync('/cdrom/live/filesystem.squashfs');
      return {
        allowed:!!allowInstall,
        script:existsSync(SCRIPT),
        live,
        installed:installedRoot(),
        efi:existsSync('/sys/firmware/efi'),
        reason: !allowInstall ? 'установка выключена: агент запущен без ключа --allow-install'
              : !existsSync(SCRIPT) ? 'в системе нет /usr/bin/glower-install'
              : !live ? 'установка возможна только из живой системы GlowerOS'
              : null
      };
    },

    async 'install.disks'(){
      const live = await liveDevice();
      const list = await disks();
      return {
        list:list.map(d => Object.assign({}, d, {
          live: !!(live && live.startsWith(d.dev))
        })),
        liveDevice:live
      };
    },

    /* проверка без записи: сценарий сам расскажет, что собирается делать */
    async 'install.plan'({ disk }){
      if (!allowInstall) throw new Error('установка выключена: запустите агент с ключом --allow-install');
      await checkDisk(disk);
      const { stdout } = await run(RUNNER.cmd, [...RUNNER.pre, '--disk', disk, '--dry-run'], { timeout:15000 });
      return { plan:stdout };
    },

    async 'install.start'({ disk, password, hostname, tz, repair }){
      if (!allowInstall) throw new Error('установка выключена: запустите агент с ключом --allow-install');
      if (job && !job.done) throw new Error('установка уже идёт');
      await checkDisk(disk);

      const args = ['--disk', disk, '--yes'];
      if (repair) args.push('--repair');
      if (password) args.push('--pass', String(password));
      if (hostname) args.push('--hostname', String(hostname).replace(/[^\w.-]/g, '') || 'GlowerOS');
      if (tz) args.push('--tz', String(tz).replace(/[^\w/+-]/g, ''));

      job = { disk, percent:0, step:'Начинаю', done:false, error:null, log:'' };
      const p = spawn(RUNNER.cmd, [...RUNNER.pre, ...args], { stdio:['ignore', 'pipe', 'pipe'] });

      let tail = '';
      p.stdout.on('data', d => {
        tail += d;
        const lines = tail.split('\n'); tail = lines.pop();
        for (const l of lines){
          job.log += l + '\n';
          const m = l.match(/^ШАГ (\d+) (.+)$/);
          if (m){ job.percent = +m[1]; job.step = m[2]; }
        }
      });
      p.stderr.on('data', d => { job.log += d; job.error = String(d).trim().split('\n').pop(); });
      p.on('exit', code => {
        job.done = true;
        if (code === 0){ job.percent = 100; job.step = 'Готово'; job.ok = true; }
        else if (!job.error) job.error = 'установка прервалась, код ' + code;
      });
      return { started:true, disk };
    },

    async 'install.state'(){
      if (!job) return { running:false };
      return { running:!job.done, ok:!!job.ok, percent:job.percent, step:job.step,
        error:job.error, disk:job.disk, log:job.log.slice(-4000) };
    }
  };

  async function checkDisk(disk){
    const list = await disks();
    const d = list.find(x => x.dev === disk);
    if (!d) throw new Error('такого диска на машине нет: ' + disk);
    const live = await liveDevice();
    if (live && live.startsWith(d.dev))
      throw new Error('это носитель, с которого запущена система — на него ставить нельзя');
    return d;
  }
}
