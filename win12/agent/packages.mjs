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

/* ---------- Flathub ----------
   В репозиториях Ubuntu нет доброй половины привычных программ: Telegram,
   Firefox, Spotify оттуда ушли. Их место — Flathub, и он одинаково работает
   на любом дистрибутиве. Правила те же: фиксированные команды, проверенный
   по образцу идентификатор, установка под общим ключом. */
const APPID = /^[A-Za-z][A-Za-z0-9._-]{2,127}$/;
const проверьId = id => {
  const s = String(id || '');
  if (!APPID.test(s)) throw new Error('недопустимый идентификатор программы: ' + s);
  return s;
};

async function flatpak(args, timeout = 60000){
  const { stdout } = await run('flatpak', args, { timeout, maxBuffer:16 << 20,
    env:{ ...process.env, LC_ALL:'C' } });
  return stdout;
}

/* строки flatpak приходят через табуляцию */
const колонки = out => out.trim().split('\n').filter(Boolean).map(l => l.split('\t'));

import { existsSync, statfsSync } from 'node:fs';

/* живая система держит всё в памяти — это меняет и место, и советы человеку */
const живая = () => existsSync('/run/live/medium') || existsSync('/cdrom/live');

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
    p.on('exit', async code => {
      if (code === 0){
        job.done = true; job.percent = 100; job.step = 'Готово'; job.ok = true; job.error = null;
        return;
      }
      /* «Unable to fetch» значит, что списки устарели: в репозитории пакеты
         уже другие. Это чинится обновлением, и незачем гонять человека —
         обновляемся сами и пробуем ещё раз, но только один. */
      const устарело = /Unable to fetch|Failed to fetch|404\s+Not Found/i.test(job.log + ' ' + (job.error || ''));
      if (устарело && !job.повтор && action !== 'update'){
        job.повтор = true;
        job.step = 'Обновляю списки и пробую снова';
        job.percent = 5;
        try {
          await run('sudo', ['-n', 'apt-get', 'update'],
            { timeout:600000, env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' } });
        } catch(e){}
        const прежний = { ...job };
        запусти(action, name, args);
        job.повтор = true;
        job.log = прежний.log;
        return;
      }
      job.done = true;
      if (!job.error) job.error = 'apt завершился с кодом ' + code;
    });
    return { started:true, name, action };
  };

  /* Flathub мог не подключиться при сборке образа — например, если у машины
     сборки не было сети. Подключаем при первой надобности, это делается раз. */
  const этоFlathub = async () => {
    try {
      if (/flathub/.test(await flatpak(['remotes'], 15000))) return;
      await run('sudo', ['-n', 'flatpak', 'remote-add', '--if-not-exists', '--system', 'flathub',
        'https://dl.flathub.org/repo/flathub.flatpakrepo'], { timeout:60000 });
    } catch(e){ throw new Error('не удалось подключить Flathub: ' + (e.message || e)); }
  };

  /* работы flatpak идут тем же путём, что и apt: одна за раз, ход виден */
  const запустиFlatpak = (action, name, args) => {
    job = { name, action, source:'flatpak', percent:2, step:'Начинаю', done:false, ok:false,
            error:null, log:'', слышно:Date.now(), pid:null };
    const p = spawn('sudo', ['-n', 'flatpak', ...args], {
      stdio:['ignore', 'pipe', 'pipe'], detached:true,
      env:{ ...process.env, LC_ALL:'C' }
    });
    job.pid = p.pid;
    const принять = кусок => {
      const t = String(кусок);
      job.слышно = Date.now();
      job.log = (job.log + t).slice(-8000);
      /* flatpak сам печатает проценты — берём их, а не выдумываем свои */
      const m = t.match(/(\d{1,3})%/g);
      if (m){
        const п = parseInt(m[m.length - 1], 10);
        if (п >= 0 && п <= 100) job.percent = Math.max(job.percent, п);
      }
      if (/Installing/i.test(t)) job.step = 'Скачиваю и ставлю';
      else if (/Uninstalling|Removing/i.test(t)) job.step = 'Удаляю';
      else if (/Updating appstream|Updating metadata/i.test(t)) job.step = 'Читаю списки Flathub';
    };
    p.stdout.on('data', принять);
    p.stderr.on('data', d => { принять(d); const t = String(d).trim(); if (t) job.error = t.split('\n').pop(); });
    p.on('exit', code => {
      job.done = true;
      if (code === 0){ job.percent = 100; job.step = 'Готово'; job.ok = true; job.error = null; }
      else if (!job.error) job.error = 'flatpak завершился с кодом ' + code;
    });
    return { started:true, name, action, source:'flatpak' };
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
      let flat = false, flathub = false;
      try { await run('which', ['flatpak']); flat = true; } catch(e){}
      if (flat){
        try { flathub = /flathub/.test(await flatpak(['remotes'], 15000)); } catch(e){}
      }

      /* Живая система держит всё в памяти: поставленное туда занимает
         оперативку и исчезает при выключении. Про это надо говорить заранее,
         а не после того, как dpkg упал от нехватки места. */
      const { statSync } = await import('node:fs');
      const live = живая();
      let free = null;
      try { const st2 = statfsSync('/'); free = st2.bavail * st2.bsize; } catch(e){}
      let listsAge = null;
      try { listsAge = Math.round((Date.now() - statSync('/var/lib/apt/lists/partial').mtimeMs) / 1000); }
      catch(e){
        try { listsAge = Math.round((Date.now() - statSync('/var/lib/apt/lists').mtimeMs) / 1000); }
        catch(e2){}
      }

      return {
        allowed:!!allowPackages, apt:apt_, sudo, lists, live, free, listsAge,
        flatpak:flat, flathub,
        busy:!!(job && !job.done),
        reason: !apt_ ? 'на машине нет apt' : !sudo ? 'у системы нет права ставить программы'
              : !allowPackages ? 'установка программ выключена: нужен ключ --allow-packages' : null
      };
    },

    /* поиск сразу по двум источникам: репозитории Ubuntu и Flathub */
    async 'pkg.search'({ query, limit }){
      const q = String(query || '').trim();
      if (q.length < 2) return { list:[] };
      if (!/^[\w+.\- а-яё]{2,60}$/i.test(q)) throw new Error('в запросе есть лишние знаки');

      let ubuntu = [];
      try {
        const out = await cache(['search', '--names-only', q]);
        ubuntu = out.trim().split('\n').filter(Boolean).map(l => {
          const i = l.indexOf(' - ');
          return { source:'apt', name:l.slice(0, i), about:l.slice(i + 3) };
        }).filter(x => x.name && NAME.test(x.name));
      } catch(e){ /* репозитории могут быть недоступны — Flathub от этого не страдает */ }

      let flathub = [];
      try {
        const out = await flatpak(['search', '--columns=application,name,version,description', q], 90000);
        if (!/No matches found/i.test(out)){
          flathub = колонки(out).map(([id, name, version, about]) => ({
            source:'flatpak', name:id, title:name, candidate:version, about:about || ''
          })).filter(x => x.name && APPID.test(x.name));
        }
      } catch(e){ /* flatpak может быть не установлен — это не ошибка поиска */ }

      /* сначала то, что названо ровно как искали: человек ищет «telegram», а не «php-telegram» */
      const точно = x => (x.title || x.name).toLowerCase().includes(q.toLowerCase()) ? 0 : 1;
      const list = [...flathub, ...ubuntu].sort((a, b) => точно(a) - точно(b))
        .slice(0, limit || 40);
      return { list };
    },

    /* что известно про программу: версия, размер, установлена ли */
    async 'pkg.info'({ name, source }){
      if (source === 'flatpak'){
        const id = проверьId(name);
        let установлена = null, версия = null, размер = null, о = '', заголовок = '';
        try {
          const own = await flatpak(['info', id], 20000);
          установлена = (own.match(/Version:\s*(.+)/) || [])[1] || 'установлена';
        } catch(e){}
        try {
          const rem = await flatpak(['remote-info', 'flathub', id], 60000);
          заголовок = (rem.match(/^\s*Name:\s*(.+)$/m) || [])[1] || '';
          версия = (rem.match(/Version:\s*(.+)/) || [])[1] || null;
          о = (rem.match(/Description:\s*(.+)/) || [])[1] || '';
          const dl = (rem.match(/Download:\s*([\d.]+)\s*(\w+)/) || []);
          if (dl[1]){
            const k = { bytes:1, kB:1e3, KB:1024, MB:1048576, GB:1073741824 }[dl[2]] || 1;
            размер = Math.round(parseFloat(dl[1]) * k);
          }
        } catch(e){}
        return { source:'flatpak', name:id, title:заголовок, installed:установлена,
                 candidate:версия, size:размер, about:о, snap:false };
      }

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
        source:'apt', name:n, version:(размеры.get(n) || {}).version || '',
        size:(размеры.get(n) || {}).size || 0
      }));

      try {
        const out = await flatpak(['list', '--app', '--columns=application,name,version'], 30000);
        колонки(out).forEach(([id, title, version]) =>
          list.push({ source:'flatpak', name:id, title, version:version || '', size:0 }));
      } catch(e){}

      return { list };
    },

    async 'pkg.update'({ source } = {}){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');
      if (source === 'flatpak'){
        await этоFlathub();
        return запустиFlatpak('update', '', ['update', '--appstream', '-y', '--noninteractive', '--system']);
      }
      return запусти('update', '', ['update']);
    },

    async 'pkg.install'({ name, source }){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');

      if (source === 'flatpak'){
        const id = проверьId(name);
        await этоFlathub();
        return запустиFlatpak('install', id,
          ['install', '-y', '--noninteractive', '--system', 'flathub', id]);
      }

      const n = проверьИмя(name);
      const про = await this['pkg.info']({ name:n });

      /* Место кончается тихо, а падает потом громко: dpkg возвращает код 1, и
         человеку остаётся гадать. Считаем заранее. */
      if (про.size){
        try {
          const st2 = statfsSync('/');
          const свободно = st2.bavail * st2.bsize;
          const нужно = про.size * 1.3 + 100 * 1024 * 1024;   // с запасом на распаковку
          if (свободно < нужно){
            const гб = b => (b / 1073741824).toFixed(1) + ' ГБ';
            throw new Error('не хватит места: программе нужно около ' + гб(нужно) +
              ', а свободно ' + гб(свободно) +
              (живая() ? '. Система работает из памяти — установите её на диск, ' +
               'и места станет столько же, сколько на диске.' : '.'));
          }
        } catch(e){ if (/не хватит места/.test(e.message)) throw e; }
      }

      if (про.snap)
        throw new Error('«' + n + '» в репозиториях Ubuntu — не сама программа, а заглушка, ' +
          'которая ставит её через Snap. Snap в GlowerOS не работает, поэтому установка ' +
          'зависла бы. Поищите программу под другим именем.');
      return запусти('install', n, ['install', '-y', '--no-install-recommends', n]);
    },

    async 'pkg.remove'({ name, source }){
      нужноРазрешение();
      if (source === 'flatpak'){
        const id = проверьId(name);
        if (job && !job.done) throw new Error('уже идёт другая работа');
        return запустиFlatpak('remove', id, ['uninstall', '-y', '--noninteractive', '--system', id]);
      }

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
        error:job.error, name:job.name, action:job.action, source:job.source || 'apt',
        log:job.log.slice(-3000),
        молчит: job.done ? 0 : Math.round((Date.now() - job.слышно) / 1000) };
    }
  };
}
