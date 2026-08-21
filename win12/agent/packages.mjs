/* ==========================================================================
   Программы машины: поиск, установка, удаление

   Система перестаёт быть закрытой коробкой: рядом со своими приложениями
   можно ставить настоящие линуксовые программы из репозиториев Ubuntu.
   Работает это через apt, и правила здесь такие же строгие, как везде в
   системном слое:
     — никаких строк, уходящих в оболочку: только execFile со списком доводов;
     — имя пакета проверяется по образцу, ключи подставить нельзя;
     — установка требует отдельного ключа запуска --allow-packages;
     — то, без чего система не живёт, удалить нельзя ни при каких условиях;
     — идёт ровно одна работа за раз, её ход виден оболочке построчно.
   ========================================================================== */
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';

const run = promisify(execFile);

/* имя пакета: буквы, цифры и то, что допускает Debian; ключей не пропустим */
const NAME = /^[a-z0-9][a-z0-9+._-]{0,120}$/;
const проверьИмя = n => {
  const s = String(n || '');
  if (!NAME.test(s)) throw new Error('недопустимое имя пакета: ' + s);
  return s;
};

/* без этого система перестанет быть системой */
const НЕЛЬЗЯ_УДАЛЯТЬ = [
  /^linux-image/, /^linux-modules/, /^systemd/, /^init$/, /^bash$/, /^dash$/,
  /^coreutils$/, /^libc6$/, /^dpkg$/, /^apt$/, /^sudo$/, /^nodejs$/, /^chromium/,
  /^cage$/, /^seatd$/, /^network-manager$/, /^grub/, /^live-boot/, /^xserver-xorg/
];

async function apt(args, opts = {}){
  const { stdout } = await run('apt-get', args, { timeout:opts.timeout || 60000, maxBuffer:8 << 20,
    env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' } });
  return stdout;
}
async function cache(args){
  const { stdout } = await run('apt-cache', args, { timeout:30000, maxBuffer:16 << 20,
    env:{ ...process.env, LC_ALL:'C' } });
  return stdout;
}

export function packages(allowPackages){
  let job = null;    // { name, action, percent, step, done, ok, error, log }

  const нужноРазрешение = () => {
    if (!allowPackages)
      throw new Error('установка программ выключена: запустите агент с ключом --allow-packages');
  };

  /* Долгая работа apt: ход показываем по его же сообщениям. Точных процентов
     apt не даёт, поэтому считаем по узнаваемым шагам — честнее, чем рисовать
     ровную полоску, которая ничего не значит. */
  const запусти = (action, name, args) => {
    job = { name, action, percent:2, step:'Начинаю', done:false, ok:false, error:null, log:'',
            слышно:Date.now(), pid:null };
    const p = spawn('sudo', ['-n', 'apt-get',
      '-o', 'Dpkg::Use-Pty=0', '-o', 'Dpkg::Options::=--force-confold', ...args], {
      stdio:['ignore', 'pipe', 'pipe'],
      detached:true,     // своя группа процессов: иначе работу нечем остановить
      env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' }
    });
    job.pid = p.pid;
    let tail = '';
    const шаг = line => {
      job.log = (job.log + line + '\n').slice(-8000);
      job.слышно = Date.now();
      if (/^Get:/.test(line)){ job.percent = Math.min(60, job.percent + 4); job.step = 'Скачиваю'; }
      else if (/^Unpacking/.test(line)){ job.percent = Math.max(job.percent, 65); job.step = 'Распаковываю'; }
      else if (/^Setting up/.test(line)){ job.percent = Math.max(job.percent, 80); job.step = 'Настраиваю'; }
      else if (/^Removing/.test(line)){ job.percent = Math.max(job.percent, 60); job.step = 'Удаляю'; }
      else if (/^Processing triggers/.test(line)){ job.percent = Math.max(job.percent, 90); job.step = 'Завершаю'; }
      else if (/^Reading|^Building/.test(line)){ job.step = 'Читаю списки'; }
    };
    p.stdout.on('data', d => {
      tail += d;
      const lines = tail.split('\n'); tail = lines.pop();
      lines.forEach(шаг);
    });
    p.stderr.on('data', d => {
      const t = String(d).trim();
      job.слышно = Date.now();
      if (t) job.error = t.split('\n').pop();
      job.log = (job.log + t + '\n').slice(-8000);
    });
    p.on('exit', code => {
      job.done = true;
      if (code === 0){ job.percent = 100; job.step = 'Готово'; job.ok = true; job.error = null; }
      else if (!job.error) job.error = 'apt завершился с кодом ' + code;
    });
    return { started:true, name, action };
  };

  return {
    /* умеет ли машина ставить программы и что для этого есть */
    async 'pkg.state'(){
      let apt_ = false, sudo = false;
      try { await run('which', ['apt-get']); apt_ = true; } catch(e){}
      try { await run('sudo', ['-n', 'true']); sudo = true; } catch(e){}
      /* Списки пакетов в образе вычищены, чтобы он не пух. Пока их не
         обновили, поиск честно ничего не найдёт — и оболочка должна об
         этом знать, а не показывать пустоту как «ничего не найдено». */
      let lists = false;
      try {
        const { readdirSync } = await import('node:fs');
        lists = readdirSync('/var/lib/apt/lists').some(f => /_Packages(\.|$)/.test(f));
      } catch(e){}
      return {
        allowed:!!allowPackages, apt:apt_, sudo, lists,
        busy:!!(job && !job.done),
        reason: !apt_ ? 'на машине нет apt' : !sudo ? 'у системы нет права ставить программы'
              : !allowPackages ? 'установка программ выключена: нужен ключ --allow-packages' : null
      };
    },

    /* поиск по репозиториям машины */
    async 'pkg.search'({ query, limit }){
      const q = String(query || '').trim();
      if (q.length < 2) return { list:[] };
      if (!/^[\w+.\- а-яё]{2,60}$/i.test(q)) throw new Error('в запросе есть лишние знаки');
      let out = '';
      try { out = await cache(['search', '--names-only', q]); }
      catch(e){ throw new Error('поиск не удался: ' + (e.message || e)); }
      const list = out.trim().split('\n').filter(Boolean).slice(0, limit || 40).map(l => {
        const i = l.indexOf(' - ');
        return { name:l.slice(0, i), about:l.slice(i + 3) };
      }).filter(x => x.name && NAME.test(x.name));
      return { list };
    },

    /* что известно про пакет: версия, размер, установлен ли */
    async 'pkg.info'({ name }){
      const n = проверьИмя(name);
      const policy = await cache(['policy', n]).catch(() => '');
      const show = await cache(['show', n]).catch(() => '');
      const поле = (t, k) => ((t.match(new RegExp('^' + k + ':\\s*(.+)$', 'm')) || [])[1] || '').trim();
      const installed = (policy.match(/Installed:\s*(.+)/) || [])[1];
      /* В Ubuntu часть «программ» — пустые заглушки, которые тянут snapd и
         ставят настоящее приложение из Snap. У нас Snap не работает, а установка
         такой заглушки просто зависает. Опознаём их и говорим прямо. */
      const pre = поле(show, 'Pre-Depends') + ' ' + поле(show, 'Depends');
      const snap = /\bsnapd\b/.test(pre) ||
        /transitional package/i.test(поле(show, 'Description-en') + поле(show, 'Description'));

      return {
        name:n, snap,
        installed: installed && installed !== '(none)' ? installed.trim() : null,
        candidate:(policy.match(/Candidate:\s*(.+)/) || [])[1] || null,
        size:+поле(show, 'Installed-Size') * 1024 || null,
        download:+поле(show, 'Size') || null,
        about:поле(show, 'Description-ru') || поле(show, 'Description-en') || поле(show, 'Description') || '',
        home:поле(show, 'Homepage') || ''
      };
    },

    /* что человек ставил сам — это и показываем как «установленное» */
    async 'pkg.installed'(){
      let manual = '';
      try { const { stdout } = await run('apt-mark', ['showmanual'], { maxBuffer:8 << 20 }); manual = stdout; }
      catch(e){ return { list:[], reason:'не удалось получить список' }; }
      const { stdout } = await run('dpkg-query',
        ['-W', '-f=${Package}\\t${Version}\\t${Installed-Size}\\n'], { maxBuffer:16 << 20 });
      const размеры = new Map(stdout.trim().split('\n').map(l => {
        const [n, v, s] = l.split('\t'); return [n, { version:v, size:+s * 1024 || 0 }];
      }));
      const list = manual.trim().split('\n').filter(Boolean).map(n => ({
        name:n, version:(размеры.get(n) || {}).version || '', size:(размеры.get(n) || {}).size || 0
      }));
      return { list };
    },

    async 'pkg.update'(){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');
      return запусти('update', '', ['update']);
    },

    async 'pkg.install'({ name }){
      нужноРазрешение();
      const n = проверьИмя(name);
      if (job && !job.done) throw new Error('уже идёт другая работа');
      const про = await this['pkg.info']({ name:n });
      if (про.snap)
        throw new Error('«' + n + '» в репозиториях Ubuntu — не сама программа, а заглушка, ' +
          'которая ставит её через Snap. Snap в GlowerOS не работает, поэтому установка ' +
          'зависла бы. Поищите программу под другим именем.');
      return запусти('install', n, ['install', '-y', '--no-install-recommends', n]);
    },

    async 'pkg.remove'({ name }){
      нужноРазрешение();
      const n = проверьИмя(name);
      if (НЕЛЬЗЯ_УДАЛЯТЬ.some(re => re.test(n)))
        throw new Error('без этой программы система работать не будет: ' + n);
      if (job && !job.done) throw new Error('уже идёт другая работа');
      return запусти('remove', n, ['remove', '-y', '--auto-remove', n]);
    },

    /* Остановить работу. Просто убить нельзя: apt запущен от root через sudo,
       поэтому и сигнал шлём через sudo — команда фиксированная, номер группы
       процессов числовой. */
    async 'pkg.cancel'(){
      нужноРазрешение();
      if (!job || job.done) return { ok:true, running:false };
      const pgid = String(job.pid);
      try { await run('sudo', ['-n', 'kill', '-TERM', '-' + pgid], { timeout:5000 }); } catch(e){}
      await new Promise(r => setTimeout(r, 3000));
      if (job && !job.done){
        try { await run('sudo', ['-n', 'kill', '-KILL', '-' + pgid], { timeout:5000 }); } catch(e){}
      }
      /* прерванный dpkg оставляет систему на середине — приводим в порядок */
      try { await run('sudo', ['-n', 'dpkg', '--configure', '-a'], { timeout:120000 }); } catch(e){}
      if (job){ job.done = true; job.ok = false; job.error = 'работа остановлена'; }
      return { ok:true, running:false };
    },

    async 'pkg.job'(){
      if (!job) return { running:false };
      return { running:!job.done, ok:job.ok, percent:job.percent, step:job.step,
        error:job.error, name:job.name, action:job.action, log:job.log.slice(-3000),
        молчит: job.done ? 0 : Math.round((Date.now() - job.слышно) / 1000) };
    }
  };
}
