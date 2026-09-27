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
import { homedir } from 'node:os';
import { promisify } from 'node:util';
import { readFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';

const run = promisify(execFile);

/* имя пакета: буквы, цифры и то, что допускает Debian; ключей не пропустим */
const NAME = /^[a-z0-9][a-z0-9+._-]{0,120}$/;
const проверьИмя = n => {
  const s = String(n || '');
  if (!NAME.test(s)) throw new Error('недопустимое имя пакета: ' + s);
  return s;
};

/* ---------- файлы-установщики ----------

   Человек скачивает программу файлом — .deb с сайта, .flatpakref со страницы
   Flathub — и ждёт, что двойной щелчок её поставит. Так работают все обычные
   системы, и здесь должно быть так же.

   Правила те же, что и везде: путь проверяется, ключей в нём быть не может,
   ставится файл только из тех мест, куда человек кладёт скачанное. */
const МЕСТА_ФАЙЛОВ = () => [homedir(), '/tmp', '/var/tmp', '/media', '/mnt', '/run/media'];

function проверьФайл(путь, расширения){
  const п = String(путь || '');
  if (!п.startsWith('/')) throw new Error('нужен полный путь к файлу');
  if (/[\n\r\0]/.test(п)) throw new Error('в пути к файлу недопустимые знаки');
  if (!existsSync(п)) throw new Error('такого файла нет: ' + п);

  let настоящий;
  try { настоящий = realpathSync(п); } catch(e){ throw new Error('файл не читается: ' + e.message); }
  if (!statSync(настоящий).isFile()) throw new Error('это не файл: ' + п);

  const низ = настоящий.toLowerCase();
  if (!расширения.some(р => низ.endsWith(р)))
    throw new Error('этим система такие файлы не ставит: ' + п);

  /* Ставим только то, что лежит там, куда человек кладёт скачанное. Иначе
     через «установку файла» можно было бы дотянуться куда угодно. */
  if (!МЕСТА_ФАЙЛОВ().some(м => настоящий === м || настоящий.startsWith(м + '/')))
    throw new Error('файл лежит там, откуда система ставить не станет: ' + настоящий);
  return настоящий;
}

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

import { existsSync, statfsSync, statSync, realpathSync, readdirSync } from 'node:fs';

/* живая система держит всё в памяти — это меняет и место, и советы человеку */
const живая = () => existsSync('/run/live/medium') || existsSync('/cdrom/live');

export function packages(allowPackages){
  let job = null;    // { name, action, percent, step, done, ok, error, log }
  let память = null; // что на машине есть: спрошено один раз, а не каждый раз

  const нужноРазрешение = () => {
    if (!allowPackages)
      throw new Error('установка программ выключена: запустите агент с ключом --allow-packages');
  };

  /* Долгая работа apt: ход показываем по его же сообщениям. Точных процентов
     apt не даёт, поэтому считаем по узнаваемым шагам — честнее, чем рисовать
     ровную полоску, которая ничего не значит. */
  /* Прерванная установка — есть ли она.

     Если dpkg оборвать посреди работы (выход из сеанса, выключение,
     пропавшее питание), он оставляет недоделанное в /var/lib/dpkg/updates,
     и дальше apt отказывается делать что-либо вообще: «dpkg was
     interrupted, you must manually run 'sudo dpkg --configure -a'».
     Человек получал это прямо в окне «Не вышло» — и дальше ни обновить,
     ни поставить, ни удалить. Смотрим туда же, куда смотрит сам apt. */
  const прервано = () => {
    try { return readdirSync('/var/lib/dpkg/updates').length > 0; }
    catch(e){ return false; }
  };

  const запусти = (action, name, args) => {
    job = { name, action, percent:2, step:'Начинаю', done:false, ok:false, error:null, log:'',
            слышно:Date.now(), pid:null };

    /* Сперва доводим прерванное — ровно тем, что велит сам apt. Это не
       ломает ничего и ничего не удаляет: dpkg просто заканчивает то, что
       начал. Потом — то, о чём человек просил. */
    if (прервано()){
      job.step = 'Довожу прерванную установку';
      const д = spawn('sudo', ['-n', 'dpkg', '--configure', '-a'], {
        stdio:['ignore', 'pipe', 'pipe'], detached:true,
        env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' } });
      job.pid = д.pid;
      let жалоба = '';
      д.stderr.on('data', x => { жалоба += x; job.слышно = Date.now(); });
      д.stdout.on('data', () => { job.слышно = Date.now(); });
      д.on('close', код => {
        if (код === 0 && !прервано()) return начни(action, name, args);
        job.done = true; job.ok = false; job.percent = 100;
        job.error = 'Прошлая установка была прервана, и довести её не вышло: '
          + (String(жалоба).trim().split('\n').pop() || ('код ' + код))
          + '. В терминале: sudo dpkg --configure -a';
      });
      return job;
    }
    return начни(action, name, args);
  };

  /* ---------- программы Windows ----------

     Wine стоит в образе сразу. Машинам, поставленным раньше, его доставляет
     обновление системы: пакет GlowerOS обновится, а следом придёт и Wine.
     В зависимости пакета его не записать — 32-битная половина (а она нужна
     почти каждому setup.exe) требует разрешить системе архитектуру i386, и
     сам apt этого не делает. Невыполнимая зависимость остановила бы
     обновление GlowerOS целиком. */
  const естьWine = async () => {
    let вывод = '';
    try {
      вывод = (await run('dpkg-query', ['-W', '-f=${Package} ${Status}\n', 'wine64', 'wine32:i386'])).stdout;
    } catch(e){ вывод = String(e.stdout || ''); }
    const стоит = имя => new RegExp('^' + имя + ' install ok installed', 'm').test(вывод);
    const wine64 = стоит('wine64'), wine32 = стоит('wine32');
    return { wine64, wine32, полный:wine64 && wine32 };
  };

  const ПАКЕТЫ_WINE = ['wine', 'wine64', 'wine32:i386'];
  const СРЕДА_APT = () => ({ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' });

  /* Поставить Wine. Если работа уже идёт (обновление системы), она сама
     передаёт сюда управление, и тогда job не создаётся заново. */
  const доставьWine = async мягко => {
    if (!job || job.done)
      job = { name:'wine', action:'install', percent:2, step:'', done:false, ok:false,
              error:null, log:'', слышно:Date.now(), pid:null };
    job.name = 'wine'; job.action = 'install'; job.percent = 3;
    job.step = 'Добавляю поддержку программ Windows';
    job.мягко = !!мягко;
    try {
      const { stdout } = await run('dpkg', ['--print-foreign-architectures']).catch(() => ({ stdout:'' }));
      if (!/\bi386\b/.test(stdout))
        await run('sudo', ['-n', 'dpkg', '--add-architecture', 'i386'], { timeout:60000, env:СРЕДА_APT() });
      job.step = 'Обновляю списки пакетов'; job.слышно = Date.now();
      await run('sudo', ['-n', 'apt-get', 'update'], { timeout:600000, env:СРЕДА_APT() });
    } catch(e){
      const почему = String(e.stderr || e.message || '').trim().split('\n').pop();
      job.done = true; job.percent = 100;
      if (мягко){ job.ok = true; job.step = 'Готово'; job.заметка = 'Wine поставить не вышло: ' + почему; }
      else { job.ok = false; job.error = 'Не вышло подготовить систему к Wine: ' + почему; }
      return;
    }
    начни('install', 'wine', ['install', '-y', '--no-install-recommends', ...ПАКЕТЫ_WINE]);
  };

  /* Wine сам, без просьбы: через несколько минут после входа, если его на
     машине ещё нет. В образ он не влезает (с ним образ больше 2 ГБ, а
     больше GitHub не принимает), поэтому «встроен» он так: система ставит
     его себе сама, первым же сеансом с сетью. Нет сети — работа тихо не
     удастся и повторится при следующем входе. С носителя не ставим: там
     всё живёт в памяти, и сотни мегабайт Wine заняли бы её целиком.
     GLOWER_NO_AUTO_WINE — для стенда: там агент с правом установки, а
     тянуть Wine в машину сборки незачем. */
  if (allowPackages && !process.env.GLOWER_NO_AUTO_WINE){
    const сам = setTimeout(async () => {
      try {
        if (живая() || (job && !job.done)) return;
        if ((await естьWine()).полный) return;
        доставьWine(true);
      } catch(e){}
    }, 4 * 60 * 1000);
    if (сам.unref) сам.unref();
  }

  const начни = (action, name, args) => {
    /* apt умеет отдавать свой собственный ход работы числами — просим его об
       этом. Раньше проценты выводились по узнаваемым строкам, и на длинной
       закачке полоса просто стояла на месте. */
    /* Ход работы apt отдаёт в отдельный канал, а не в общий вывод. Раньше он
       шёл туда же, куда и обычные сообщения, и служебные строки протокола
       попадали человеку прямо в окно ошибки: «pmerror:happ:60.0000:...».
       Читать такое невозможно, а главное — незачем. */
    const p = spawn('sudo', ['-n', 'apt-get',
      '-o', 'Dpkg::Use-Pty=0', '-o', 'Dpkg::Options::=--force-confold',
      '-o', 'APT::Status-Fd=3', ...args], {
      stdio:['ignore', 'pipe', 'pipe', 'pipe'],
      detached:true,     // своя группа процессов: иначе работу нечем остановить
      env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' }
    });
    job.pid = p.pid;
    let tail = '';
    const шаг = line => {
      job.слышно = Date.now();

      /* строки состояния: «dlstatus:1:12.3:Скачивание…», «pmstatus:пакет:64.0:…» */
      const st = line.match(/^(dlstatus|pmstatus|status):[^:]*:([\d.]+):(.*)$/);
      if (st){
        const п = Math.round(parseFloat(st[2]));
        if (п >= 0 && п <= 100){
          /* скачивание — первая половина полосы, установка — вторая */
          job.percent = st[1] === 'dlstatus' ? Math.min(50, Math.round(п / 2))
                                             : Math.max(50, 50 + Math.round(п / 2));
        }
        if (st[3]) job.step = st[1] === 'dlstatus' ? 'Скачиваю' : 'Устанавливаю';
        return;
      }

      job.log = (job.log + line + '\n').slice(-8000);
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
    /* тот самый отдельный канал: только числа хода работы */
    let хвостСостояния = '';
    if (p.stdio[3]) p.stdio[3].on('data', d => {
      хвостСостояния += d;
      const строки = хвостСостояния.split('\n'); хвостСостояния = строки.pop();
      строки.forEach(шаг);
    });
    p.stderr.on('data', d => {
      const t = String(d).trim();
      job.слышно = Date.now();
      if (t) job.error = t.split('\n').pop();
      job.log = (job.log + t + '\n').slice(-8000);
    });
    p.on('exit', async code => {
      if (code === 0){
        /* Следом за этой работой может идти ещё одна — так за обновлением
           системы идёт доставка Wine. Одна работа за раз, поэтому очередь
           здесь, а не у того, кто просил. */
        if (job.потом){
          const дальше = job.потом; job.потом = null;
          return дальше();
        }
        job.done = true; job.percent = 100; job.step = 'Готово'; job.ok = true; job.error = null;
        return;
      }
      /* «Unable to fetch» значит, что списки устарели: в репозитории пакеты
         уже другие. Это чинится обновлением, и незачем гонять человека —
         обновляемся сами и пробуем ещё раз, но только один. */
      /* Пакет, у которого не отработал сценарий настройки, остаётся в системе
         наполовину: dpkg помечает его как ненастроенный, и следующая же
         установка чего угодно упирается в него. Человек в этом не виноват и
         починить это одним нажатием не может, а лечится оно одной командой,
         которую сам apt и советует. Делаем её сами — один раз, и говорим об
         этом в журнале работы. */
      const недонастроен = /dpkg was interrupted|--configure -a|not configured yet|returned an error code/i
        .test(job.log + ' ' + (job.error || ''));
      if (недонастроен && !job.починка){
        job.step = 'Привожу пакеты в порядок';
        job.log = (job.log + '\n— dpkg остался с ненастроенным пакетом, выполняю dpkg --configure -a\n').slice(-8000);
        let вышло = true;
        try {
          await run('sudo', ['-n', 'dpkg', '--configure', '-a'],
            { timeout:600000, env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' } });
        } catch(e){ вышло = false; }
        /* Починили — делаем то, о чём просили, ещё раз. Раньше на этом
           останавливались: пакеты приводились в порядок, а человек всё
           равно видел «Не вышло» и не понимал, что достаточно нажать ещё
           раз. Один повтор, не больше: если не помогло и он, дальше гадать
           нечего — говорим как есть. */
        if (вышло && !прервано()){
          const прежний = { ...job };
          запусти(action, name, args);
          job.починка = true;
          job.мягко = прежний.мягко;
          job.потом = прежний.потом;
          job.log = (прежний.log + '\n— пакеты в порядке, пробую снова\n').slice(-8000);
          return;
        }
        job.починка = true;
      }

      /* «Невозможно найти пакет» здесь — не опечатка человека, а пустой
         индекс: образ мы отдаём без списков пакетов, чтобы влезть в лимит
         на размер, и до первого обновления apt не знает ни одного пакета,
         которого нет на диске. Признак другой, а лечится тем же самым, так
         что и его считаем поводом обновиться и попробовать снова. */
      const устарело = /Unable to fetch|Failed to fetch|404\s+Not Found|Unable to locate package|Невозможно найти пакет|Couldn't find any package/i
        .test(job.log + ' ' + (job.error || ''));
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
        job.мягко = прежний.мягко;
        job.потом = прежний.потом;
        job.log = прежний.log;
        return;
      }
      job.done = true;
      if (!job.error) job.error = 'apt завершился с кодом ' + code;
      /* Необязательное дело, пристёгнутое к главному: главное уже сделано,
         и его провал не должен выглядеть провалом всего. Обновление системы
         прошло — значит, «Готово», а о Wine скажем отдельной строкой. */
      if (job.мягко){
        job.заметка = job.error; job.error = null; job.ok = true; job.step = 'Готово';
      }
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
  /* свой — программа стоит у человека, а не в системе: тогда flatpak
     зовём от его имени, без sudo. Через sudo он смотрел бы в дом root и
     честно отвечал бы, что такой программы нет. */
  const запустиFlatpak = (action, name, args, свой = false) => {
    job = { name, action, source:'flatpak', percent:2, step:'Начинаю', done:false, ok:false,
            error:null, log:'', слышно:Date.now(), pid:null };
    const p = spawn(свой ? 'flatpak' : 'sudo', свой ? args : ['-n', 'flatpak', ...args], {
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

  /* ======================================================================
     Удаление программы по её ярлыку

     Меню знает о программе только ярлык: имя, значок и строку запуска. Чтобы
     удалить программу, нужно узнать, кто её хозяин, — и у разных программ
     он разный:
       — пакет Ubuntu: владельца ярлыка знает dpkg;
       — Flathub: имя программы записано прямо в ярлыке;
       — Windows: ярлык сделал Wine, а удаляет программу её собственный
         деинсталлятор, который Wine знает по своему списку;
       — AppImage: программа — один файл, на него указывает ярлык;
       — просто ярлык, сделанный руками: удалить можно только его.
     Решает всегда агент, заново, по самому ярлыку: оболочке на слово не
     верим — иначе «удалить Блокнот» можно было бы превратить в «удалить
     что угодно».
     ====================================================================== */

  /* Без чего сеанс GlowerOS не живёт — сверх того, без чего не живёт сама
     Ubuntu. Удалить labwc или агента из меню значит остаться с чёрным
     экраном. */
  const СВОИ_НЕЛЬЗЯ = [/^glower/, /^labwc$/, /^foot$/, /^thunar$/, /^xwayland$/,
    /^greetd$/, /^pipewire/, /^wireplumber$/, /^network-manager/, /^policykit/, /^polkitd$/,
    /^xdg-desktop-portal/, /^flatpak$/, /^bluez$/, /^wine/, /^gir1\.2-/, /^libwebkit/,
    /* Сам Python — да, а не всё, что начинается с «python3»: LibreOffice
       тянет python3-uno, и широкий образец не давал удалить LibreOffice.
       Если удаление программы задевает сам Python, это поймает пробный
       прогон apt — там python3 появится в списке уходящего. */
    /^python3$/, /^python3-minimal$/, /^python3-gi/, /^libpython3\.\d+$/];
  const несъёмный = п => НЕЛЬЗЯ_УДАЛЯТЬ.some(re => re.test(п)) || СВОИ_НЕЛЬЗЯ.some(re => re.test(п));

  const домой = () => homedir();
  const ПАПКА_WINE = () => join(домой(), '.local/share/applications/wine');
  const СРЕДА_WINE = () => ({ ...process.env, HOME:домой(),
    WINEPREFIX:process.env.WINEPREFIX || join(домой(), '.wine'), WINEDEBUG:'-all' });

  /* Что Wine считает поставленным: ключ записи и имя, как в «Установке и
     удалении программ» Windows. Wine печатает их строками «ключ|||имя». */
  const списокWindows = async () => {
    if (!existsSync(СРЕДА_WINE().WINEPREFIX)) return [];
    let вывод = '';
    try { вывод = (await run('wine', ['uninstaller', '--list'], { timeout:60000, env:СРЕДА_WINE() })).stdout; }
    catch(e){ вывод = String(e.stdout || ''); }
    return String(вывод).split('\n').map(с => с.trim()).filter(с => с.includes('|||'))
      .map(с => { const [ключ, ...имя] = с.split('|||'); return { ключ:ключ.trim(), имя:имя.join('|||').trim() }; })
      .filter(з => з.ключ && з.имя);
  };
  /* Запись Wine для программы — по имени. Сначала точное совпадение,
     потом «одно начинается с другого»: ярлык зовётся «Steam», а запись —
     «Steam (remove only)» или наоборот. */
  const найдиЗапись = (список, имя) => {
    const н = String(имя || '').toLowerCase().trim();
    if (!н) return null;
    return список.find(з => з.имя.toLowerCase() === н)
        || список.find(з => з.имя.toLowerCase().startsWith(н) || н.startsWith(з.имя.toLowerCase()))
        || null;
  };

  const поле = (текст, k) => {
    const начало = текст.search(/^\[Desktop Entry\]\s*$/m);
    const t = начало < 0 ? текст : текст.slice(начало).split(/^\[(?!Desktop Entry)[^\]]+\]\s*$/m)[0];
    return (t.match(new RegExp('^' + k + '=(.*)$', 'm')) || [])[1] || '';
  };

  const хозяин = async id => {
    const имя = String(id || '');
    if (!/^[^/\\\0]+\.desktop$/.test(имя) || имя.startsWith('.'))
      throw new Error('неверный идентификатор программы');
    const { найди_ярлык, разбери_exec } = await import('./system.mjs');
    const файл = await найди_ярлык(имя);
    if (!файл) throw new Error('такой программы на машине уже нет');
    const текст = await readFile(файл, 'utf8');
    const назван = поле(текст, 'Name') || имя.replace(/\.desktop$/, '');
    const exec = разбери_exec(поле(текст, 'Exec'));
    const свой = файл.startsWith(домой() + '/');
    const итог = { id:имя, файл, 'имя':назван, 'можно':true };

    /* Flathub: ярлык сам называет программу. */
    const fp = поле(текст, 'X-Flatpak');
    if (fp && APPID.test(fp))
      return { ...итог, 'вид':'flatpak', 'пакет':fp,
               'у_человека':файл.includes('/.local/share/flatpak/') };

    /* Windows: ярлык из папки Wine или строка запуска, зовущая wine. */
    const зовётWine = exec.some(ч => /(^|\/)wine(64)?$/.test(ч));
    if (файл.startsWith(ПАПКА_WINE() + '/') || зовётWine){
      const запись = найдиЗапись(await списокWindows(), назван);
      return { ...итог, 'вид':'windows', 'ключ':запись ? запись.ключ : null,
               'запись':запись ? запись.имя : null };
    }

    /* AppImage: удалить — значит убрать сам файл и ярлык к нему. */
    const образ = exec.find(ч => /\.appimage$/i.test(ч) && ч.startsWith('/'));
    if (образ && свой)
      return { ...итог, 'вид':'appimage', 'образ':образ };

    /* Ярлык в доме человека: пакета за ним нет, удалить можно его самого. */
    if (свой) return { ...итог, 'вид':'ярлык' };

    /* Системный ярлык — у него есть пакет. */
    let пакет = '';
    try {
      const { stdout } = await run('dpkg', ['-S', файл], { timeout:15000, env:{ ...process.env, LC_ALL:'C' } });
      пакет = String(stdout).split('\n').find(с => /: \//.test(с) && !/^diversion/.test(с)) || '';
      пакет = пакет.split(': /')[0].split(',')[0].trim().replace(/:[a-z0-9]+$/, '');
    } catch(e){}
    if (!пакет || !NAME.test(пакет))
      return { ...итог, 'вид':'система', 'можно':false,
               'почему':'Этот ярлык — часть самой системы, а не отдельная программа' };
    if (несъёмный(пакет))
      return { ...итог, 'вид':'apt', 'пакет':пакет, 'можно':false,
               'почему':'Без «' + назван + '» система работать не будет — удалить её нельзя' };

    /* Что уйдёт вместе с ней: спрашиваем apt «понарошку», без root. Если в
       этом списке есть то, без чего система не живёт, — отказываем: так
       бывает, когда программа тянет за собой общую часть рабочего стола. */
    let уйдёт = [];
    try {
      const { stdout } = await run('apt-get', ['-s', 'remove', '--auto-remove', пакет],
        { timeout:60000, env:{ ...process.env, LC_ALL:'C' } });
      уйдёт = [...String(stdout).matchAll(/^Remv (\S+)/gm)].map(м => м[1].replace(/:[a-z0-9]+$/, ''));
    } catch(e){}
    const опасно = уйдёт.filter(несъёмный);
    if (опасно.length)
      return { ...итог, 'вид':'apt', 'пакет':пакет, 'можно':false, 'уйдёт':уйдёт,
               'почему':'Вместе с «' + назван + '» ушла бы часть системы: ' + опасно.slice(0, 3).join(', ') };
    let кб = 0;
    if (уйдёт.length){
      try {
        const { stdout } = await run('dpkg-query', ['-W', '-f=${Installed-Size}\n', ...уйдёт], { timeout:15000 });
        кб = String(stdout).split('\n').reduce((с, ч) => с + (parseInt(ч, 10) || 0), 0);
      } catch(e){ кб = parseInt(String(e.stdout || '').split('\n').reduce((с, ч) => с + (parseInt(ч, 10) || 0), 0), 10) || 0; }
    }
    return { ...итог, 'вид':'apt', 'пакет':пакет, 'уйдёт':уйдёт, 'освободит':кб * 1024 };
  };

  /* Удаление программ Windows идёт своим окном — окном её деинсталлятора.
     Следим, пока запись не пропадёт из списка Wine: многие деинсталляторы
     (NSIS — у Steam он такой) копируют себя во временную папку, запускают
     копию и сразу выходят, так что «процесс закончился» ещё не значит
     «программа удалена». */
  const удаленияWindows = new Map();

  /* Ярлыки программы Windows: сам ярлык, его соседи по папке программы в
     меню и значки на рабочем столе с тем же именем. */
  const уберИЯрлыки = async (файл, имя) => {
    const { unlink, readdir, rmdir } = await import('node:fs/promises');
    const убрано = [];
    const убери = async ф => { try { await unlink(ф); убрано.push(ф); } catch(e){} };
    const корень = join(ПАПКА_WINE(), 'Programs');
    const папка = dirname(файл);
    if (папка.startsWith(корень + '/')){
      /* Своя папка программы в меню — уходит вся. */
      const своя = join(корень, папка.slice(корень.length + 1).split('/')[0]);
      const обойди = async д => {
        for (const з of await readdir(д, { withFileTypes:true }).catch(() => [])){
          const п = join(д, з.name);
          if (з.isDirectory()){ await обойди(п); await rmdir(п).catch(() => {}); }
          else if (з.name.endsWith('.desktop')) await убери(п);
        }
      };
      await обойди(своя);
      await rmdir(своя).catch(() => {});
    } else await убери(файл);
    for (const стол of ['Desktop', 'Рабочий стол']){
      const д = join(домой(), стол);
      for (const з of await readdir(д).catch(() => [])){
        if (!з.endsWith('.desktop')) continue;
        try {
          const т = await readFile(join(д, з), 'utf8');
          if (/\bwine\b/.test(поле(т, 'Exec')) && поле(т, 'Name').toLowerCase() === String(имя).toLowerCase())
            await убери(join(д, з));
        } catch(e){}
      }
    }
    return убрано;
  };

  const удалиWindows = async о => {
    const ключ = о.ключ;
    const запись = { id:о.id, 'имя':о['имя'], 'идёт':true, 'удалено':false, 'почему':'', начало:Date.now() };
    удаленияWindows.set(о.id, запись);
    if (!ключ){
      /* Wine не знает, как её удалять: её не ставили установщиком (или он
         не записался). Убираем то, что видит человек, — ярлыки; файлы
         остаются в папке Windows, и об этом говорим прямо. */
      await уберИЯрлыки(о.файл, о['имя']);
      Object.assign(запись, { 'идёт':false, 'удалено':true, 'толькоЯрлык':true });
      return запись;
    }
    const { средаЭкрана } = await import('./system.mjs');
    const env = { ...(await средаЭкрана()), ...СРЕДА_WINE() };
    const д = spawn('wine', ['uninstaller', '--remove', ключ], { env, detached:true, stdio:'ignore' });
    д.on('error', e => Object.assign(запись, { 'идёт':false, 'почему':'не удалось запустить удаление: ' + e.message }));
    д.unref();
    /* Смотрим в список раз в три секунды, до двадцати минут: столько
       человек может сидеть над окном деинсталлятора. */
    (async () => {
      await new Promise(r => д.on('exit', r));
      const предел = Date.now() + 20 * 60 * 1000;
      while (Date.now() < предел && запись['идёт']){
        const есть = (await списокWindows()).some(з => з.ключ === ключ);
        if (!есть){
          await уберИЯрлыки(о.файл, о['имя']);
          Object.assign(запись, { 'идёт':false, 'удалено':true });
          return;
        }
        await new Promise(r => setTimeout(r, 3000));
      }
      if (запись['идёт'])
        Object.assign(запись, { 'идёт':false, 'почему':'программа осталась в списке — удаление отменили или оно не удалось' });
    })().catch(e => Object.assign(запись, { 'идёт':false, 'почему':String(e.message || e) }));
    return запись;
  };

  return {
    /* умеет ли машина ставить программы и что для этого есть */
    async 'pkg.state'(){
      /* Что на машине есть, за время работы не меняется, а оболочка
         спрашивает об этом часто. Раньше каждый такой вопрос дёргал sudo, и
         в журнале безопасности копились записи «COMMAND=/usr/bin/true» — по
         одной в секунду. Спрашиваем один раз и помним ответ. */
      const теперь = Date.now();
      if (!память || теперь - память.когда > 60000){
        let apt_ = false, sudo = false;
        try { await run('which', ['apt-get']); apt_ = true; } catch(e){}
        try { await run('sudo', ['-n', 'true']); sudo = true; } catch(e){}
        память = { когда:теперь, apt_, sudo };
      }
      const { apt_, sudo } = память;
      /* Списки пакетов в образе вычищены, чтобы он не пух. Пока их не
         обновили, поиск честно ничего не найдёт — и оболочка должна об
         этом знать, а не показывать пустоту как «ничего не найдено». */
      let lists = false;
      try {
        const { readdirSync } = await import('node:fs');
        lists = readdirSync('/var/lib/apt/lists').some(f => /_Packages(\.|$)/.test(f));
      } catch(e){}
      let flat = false, flathub = false, flathubData = false;
      try { await run('which', ['flatpak']); flat = true; } catch(e){}
      if (flat){
        try { flathub = /flathub/.test(await flatpak(['remotes'], 15000)); } catch(e){}
        /* Списки Flathub качаются отдельно от подключения. Без них поиск
           честно ничего не находит — и это надо показывать как «источник ещё
           не готов», а не как «ничего не найдено». */
        try {
          const { readdirSync } = await import('node:fs');
          flathubData = readdirSync('/var/lib/flatpak/appstream/flathub').length > 0;
        } catch(e){}
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
        flatpak:flat, flathub, flathubData,
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

      /* Что из найденного уже стоит. Без этого поиск предлагал «Установить»
         то, что установлено, — человек нажимал, работа заканчивалась мгновенно
         с «Skipping: уже установлено», и выглядело это как сбой. Спрашиваем обе
         системы: обе отвечают из своих списков на диске, сеть не нужна. */
      const стоят = new Set();
      try {
        const { stdout } = await run('dpkg-query', ['-W', '-f=${Package}\t${Status}\n'],
          { maxBuffer:16 << 20 });
        stdout.split('\n').forEach(l => {
          const [имя, состояние] = l.split('\t');
          /* Состояние dpkg — три слова, и решает последнее. Проверять его
             вхождением нельзя: «purge ok not-installed» тоже содержит
             «installed», и удалённые пакеты выглядели бы установленными. */
          if (имя && (состояние || '').trim().split(/\s+/).pop() === 'installed')
            стоят.add('apt:' + имя);
        });
      } catch(e){ /* dpkg может не ответить — тогда просто не отметим */ }
      try {
        const out = await flatpak(['list', '--app', '--columns=application'], 20000);
        out.split('\n').map(x => x.trim()).filter(Boolean)
          .forEach(id => стоят.add('flatpak:' + id));
      } catch(e){ /* flatpak может отсутствовать — не беда */ }

      const отметь = x => Object.assign(x, { installed:стоят.has(x.source + ':' + x.name) });
      flathub.forEach(отметь);
      ubuntu.forEach(отметь);

      /* сначала то, что названо ровно как искали: человек ищет «telegram», а не «php-telegram» */
      const точно = x => (x.title || x.name).toLowerCase().includes(q.toLowerCase()) ? 0 : 1;
      const list = [...flathub, ...ubuntu].sort((a, b) => точно(a) - точно(b))
        .slice(0, limit || 40);
      return { list, flathubИскал:flathub.length > 0 };
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

    /* Подключить Flathub и скачать его списки — одно понятное действие,
       которое человек запускает кнопкой и видит ход. */
    async 'pkg.flathub'(){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');
      let есть = false;
      try { есть = await run('which', ['flatpak']).then(() => true); } catch(e){}
      if (!есть) throw new Error('на машине нет flatpak — Flathub подключать нечем');
      await этоFlathub();
      return запустиFlatpak('flathub', '', ['update', '--appstream', '-y', '--noninteractive', '--system']);
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

    /* Что за файл нам дали: имя программы, версия, размер, описание.
       Читает сам dpkg — гадать по имени файла мы не станем. */
    async 'pkg.file.info'({ путь }){
      const файл = проверьФайл(путь, ['.deb', '.flatpakref']);

      /* Страница Flathub отдаёт маленький файл-описание: в нём сказано, что
         за программа и откуда её брать. Ставит такое сам flatpak. */
      if (файл.toLowerCase().endsWith('.flatpakref')){
        const текст = await readFile(файл, 'utf8').catch(() => '');
        const поле = к => (текст.match(new RegExp('^' + к + '=(.*)$', 'm')) || [])[1] || '';
        const имя = поле('Name');
        if (!имя) throw new Error('в этом файле нет имени программы — flatpak его не поймёт');
        return { файл, вид:'flatpak', имя, версия:'', железо:'',
          описание:поле('Title') || поле('Comment') || '',
          откуда:поле('Url') || поле('RuntimeRepo') || '',
          зависимости:[], размер:statSync(файл).size, место:null };
      }

      let текст = '';
      try {
        const { stdout } = await run('dpkg-deb', ['-f', файл,
          'Package', 'Version', 'Architecture', 'Installed-Size', 'Depends', 'Description'],
          { timeout:15000, maxBuffer:1 << 20 });
        текст = String(stdout);
      } catch(e){
        throw new Error('это не пакет Debian или он повреждён: ' + (e.stderr || e.message));
      }
      const поле = к => (текст.match(new RegExp('^' + к + ':\\s*(.*)$', 'm')) || [])[1] || '';
      const размерФайла = statSync(файл).size;
      const место = parseInt(поле('Installed-Size'), 10);
      return {
        файл, имя:поле('Package'), версия:поле('Version'),
        железо:поле('Architecture'), описание:поле('Description'),
        зависимости:поле('Depends').split(',').map(x => x.trim()).filter(Boolean),
        размер:размерФайла,
        место:Number.isFinite(место) ? место * 1024 : null
      };
    },

    /* Поставить программу из файла. Зависимости apt подтянет сам — этим
       установка из файла и отличается от голого dpkg, который на нехватке
       зависимостей просто ломается. */
    async 'pkg.file.install'({ путь }){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');
      const про = await this['pkg.file.info']({ путь });

      if (про.вид === 'flatpak'){
        await этоFlathub();
        return запустиFlatpak('install', про.имя,
          ['install', '-y', '--noninteractive', '--system', '--from', про.файл]);
      }

      const своё = process.arch === 'x64' ? ['amd64', 'all'] : [process.arch, 'all'];
      if (про.железо && !своё.includes(про.железо))
        throw new Error('этот пакет собран для другого железа (' + про.железо +
          '), а машина — ' + своё[0]);

      return запусти('install', про.имя || про.файл,
        ['install', '-y', '--no-install-recommends', про.файл]);
    },

    /* ---------- обновления системы ----------

       «Что можно обновить» читает то, что система уже знает: быстро и без
       сети. Обновить сами списки — отдельное дело (pkg.update), у него свой
       ход работы, потому что оно ходит в интернет и бывает долгим. */
    async 'pkg.upgrade.check'(){
      let текст = '';
      try {
        const { stdout } = await run('sudo', ['-n', 'apt-get', '-s',
          '-o', 'APT::Get::Show-User-Simulation-Note=false', '--with-new-pkgs', 'upgrade'],
          { timeout:30000, maxBuffer:8 << 20,
            env:{ ...process.env, DEBIAN_FRONTEND:'noninteractive', LC_ALL:'C' } });
        текст = String(stdout);
      } catch(e){
        const вывод = String(e.stderr || '').trim() || String(e.stdout || '').trim();
        throw new Error('не вышло спросить про обновления: ' +
          (вывод.split('\n').filter(Boolean).pop() || ('код ' + (e.code != null ? e.code : '?'))));
      }

      /* Строки вида: Inst firefox [1.0] (2.0 Mozilla:mozilla [amd64]) */
      const list = [];
      for (const строка of String(текст).split('\n')){
        const м = строка.match(/^Inst\s+(\S+)\s+\[([^\]]*)\]\s+\(([^\s)]+)/);
        if (м) list.push({ name:м[1], было:м[2], станет:м[3] });
      }

      /* Придержанные обновления.
      
         Если новая версия пакета требует пакета, которого на машине ещё
         нет, обычное «обновить» его не поставит: apt отложит такое
         обновление и скажет об этом одной строкой в середине вывода.
         Человеку же со стороны видно только, что кнопка нажата, а версия
         прежняя. Собираем их отдельно, чтобы система могла сказать вслух.
      
         Список идёт следом за строкой «kept back» и переносится по
         строкам, пока они начинаются с пробела. */
      const удержано = [];
      const строки = String(текст).split('\n');
      for (let i = 0; i < строки.length; i++){
        if (!/kept back|удерж/i.test(строки[i])) continue;
        for (let j = i + 1; j < строки.length && /^\s+\S/.test(строки[j]); j++)
          удержано.push(...строки[j].trim().split(/\s+/));
      }

      let когда = null;
      try {
        const { statSync } = await import('node:fs');
        когда = statSync('/var/lib/apt/periodic/update-success-stamp').mtimeMs;
      } catch(e){
        try {
          const { statSync } = await import('node:fs');
          когда = statSync('/var/lib/apt/lists').mtimeMs;
        } catch(e2){}
      }

      /* Программы из Flathub — Telegram, Steam, OBS и всё, что человек
         поставил оттуда.

         До сих пор про них здесь не спрашивали вовсе: обновления знали
         только apt, и со стороны выглядело так, будто система обновляет
         лишь то, что написали мы сами. Спросить flatpak стоит одной
         строки: у него есть готовый список «что можно обновить».

         Список не входит в «всего» у apt — это отдельная очередь со своей
         кнопкой, потому что и обновляются они отдельно, своим путём. */
      const flathub = [];
      try {
        const выход = await flatpak(['remote-ls', '--updates', '--app',
          '--columns=application,name,version'], 60000);
        колонки(выход).forEach(([id, имя, версия]) => {
          if (id && APPID.test(id)) flathub.push({ name:id, 'имя':имя || id, 'станет':версия || '', source:'flatpak' });
        });
      } catch(e){ /* flatpak не стоит или Flathub не подключён — значит, и обновлять там нечего */ }

      return { list, всего:list.length, когда, 'удержано':удержано,
               'flathub':flathub, можно:!!allowPackages };
    },

    /* Поставить обновления — тем же способом, что и установку программ. */
    async 'pkg.upgrade.run'({ source } = {}){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');
      if (source === 'flatpak'){
        await этоFlathub();
        return запустиFlatpak('update', '', ['update', '-y', '--noninteractive', '--system']);
      }
      /* --with-new-pkgs — ради ядра, и это не мелочь.

         Обычное apt-get upgrade никогда не ставит новых пакетов, а новое
         ядро Ubuntu приходит именно новым пакетом: linux-image-6.8.0-52 —
         это другое имя, чем linux-image-6.8.0-51. Метапакет, который его
         тянет, получал отказ, и apt молча «придерживал» его. Всё остальное
         обновлялось, а ядро с его исправлениями безопасности и драйверами
         оставалось тем, что было в образе, — навсегда.

         Этот ключ разрешает ставить новое, если оно нужно для обновления,
         и ничего не удаляет. Ровно так обновляет и сама Ubuntu, когда
         человек пишет apt upgrade. */
      /* Обновление системы приводит её к тому, что система обещает, — а
         обещает она и программы Windows. Wine идёт следом, в той же работе,
         и его неудача обновление не портит. Спрашиваем заранее: быстрое
         обновление успело бы закончиться, пока мы спрашиваем, и очередь
         опоздала бы. */
      const нуженWine = !(await естьWine()).полный;
      const пуск = запусти('upgrade', 'система', ['upgrade', '-y', '--with-new-pkgs', '--no-install-recommends']);
      if (нуженWine) job.потом = () => доставьWine(true);
      return пуск;
    },

    /* Программы Windows: есть ли Wine и можно ли его поставить. */
    async 'pkg.windows'(){
      const в = await естьWine();
      return { ...в, можно:!!allowPackages };
    },
    async 'pkg.windows.install'(){
      нужноРазрешение();
      if (job && !job.done) throw new Error('уже идёт другая работа');
      if ((await естьWine()).полный) return { started:false, уже:true };
      job = null;
      доставьWine(false);
      return { started:true, name:'wine', action:'install' };
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

    /* Кто хозяин программы и что уйдёт вместе с ней — для окна «Удалить?». */
    async 'pkg.owner'({ id }){
      const о = await хозяин(id);
      delete о.файл;                          // путь оболочке ни к чему
      return { ...о, 'разрешено':!!allowPackages };
    },

    /* Удалить программу по её ярлыку. Хозяина агент выясняет сам, заново. */
    async 'pkg.uninstall'({ id }){
      нужноРазрешение();
      const о = await хозяин(id);
      if (!о['можно']) throw new Error(о['почему'] || 'эту программу удалить нельзя');
      if (о['вид'] === 'windows') return { 'вид':'windows', ...(await удалиWindows(о)) };
      if (о['вид'] === 'appimage' || о['вид'] === 'ярлык'){
        const { unlink } = await import('node:fs/promises');
        if (о['вид'] === 'appimage') await unlink(о['образ']).catch(() => {});
        await unlink(о.файл);
        return { 'вид':о['вид'], 'удалено':true };
      }
      if (job && !job.done) throw new Error('уже идёт другая работа');
      if (о['вид'] === 'flatpak')
        return { 'вид':'flatpak', ...запустиFlatpak('remove', о['пакет'],
          ['uninstall', '-y', '--noninteractive', о['у_человека'] ? '--user' : '--system', о['пакет']],
          о['у_человека']) };
      return { 'вид':'apt', ...запусти('remove', о['пакет'], ['remove', '-y', '--auto-remove', о['пакет']]) };
    },

    /* Как идёт удаление программы Windows. */
    async 'pkg.uninstall.windows'({ id }){
      const з = удаленияWindows.get(String(id || ''));
      if (!з) return { 'идёт':false, 'неизвестно':true };
      const { начало, ...ответ } = з;
      return ответ;
    },

    /* Программы Windows списком — для «Параметров»: что стоит и чем удалить. */
    async 'pkg.windows.list'(){
      return { 'список':await списокWindows() };
    },
    async 'pkg.windows.remove'({ ключ }){
      нужноРазрешение();
      const к = String(ключ || '');
      const запись = (await списокWindows()).find(з => з.ключ === к);
      if (!запись) throw new Error('такой программы Windows нет');
      /* Ярлык ищем по имени — чтобы после удаления убрать и его. */
      let файл = null;
      const { readdir } = await import('node:fs/promises');
      const обойди = async д => {
        for (const з of await readdir(д, { withFileTypes:true }).catch(() => [])){
          if (файл) return;
          const п = join(д, з.name);
          if (з.isDirectory()) await обойди(п);
          else if (з.name.endsWith('.desktop')){
            const т = await readFile(п, 'utf8').catch(() => '');
            if (найдиЗапись([запись], поле(т, 'Name'))) файл = п;
          }
        }
      };
      await обойди(ПАПКА_WINE());
      const id = 'windows:' + к;
      return удалиWindows({ id, 'имя':запись.имя, ключ:к, файл:файл || join(ПАПКА_WINE(), 'нет.desktop') });
    },

    /* Удалить все программы Windows разом: папку Windows целиком и все
       ярлыки Wine. Сам Wine остаётся — следующий .exe заведёт папку заново. */
    async 'pkg.windows.reset'(){
      нужноРазрешение();
      const { rm, readdir } = await import('node:fs/promises');
      try { await run('wineserver', ['-k'], { timeout:15000, env:СРЕДА_WINE() }); } catch(e){}
      await rm(СРЕДА_WINE().WINEPREFIX, { recursive:true, force:true });
      await rm(ПАПКА_WINE(), { recursive:true, force:true });
      for (const [д, образец] of [[join(домой(), '.local/share/desktop-directories'), /^wine-/],
                                  [join(домой(), '.config/menus/applications-merged'), /^wine-/]])
        for (const з of await readdir(д).catch(() => []))
          if (образец.test(з)) await rm(join(д, з), { force:true });
      return { 'удалено':true };
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
        'заметка':job.заметка || null,
        log:job.log.slice(-3000),
        молчит: job.done ? 0 : Math.round((Date.now() - job.слышно) / 1000) };
    }
  };
}
