/* ==========================================================================
   Часы и часовой пояс

   Рядом с Windows аппаратные часы компьютера идут по местному времени (так
   их держит Windows). Если при этом в системе не задан часовой пояс (Etc/UTC),
   на экране время верное, а для всего мира компьютер живёт на несколько
   часов в будущем или прошлом: сайты считают сертификаты недействительными,
   Flathub отказывает, Яндекс пишет «часы спешат на 7 часов».

   Так и было у человека: установщик брал пояс из настроек живой системы, а
   там он пустой, и службы сверки времени (systemd-timesyncd) в образе не
   было. Агент лечит это сам при запуске:
     — пояса нет — определяет его: по городу погоды (геокодер Open-Meteo
       отдаёт пояс города), а нет — по адресу в сети (ip-api, ipapi);
     — задаёт пояс; часы идут по местному — перечитывает их заново, уже с
       верным поясом (hwclock --hctosys);
     — включает сверку времени по сети, если служба есть.
   ========================================================================== */
import { execFile } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);
const ПОЯСА = '/usr/share/zoneinfo';

export const годный_пояс = п => typeof п === 'string' && /^[A-Za-z_]+(\/[A-Za-z0-9_+-]+){1,2}$/.test(п) && existsSync(join(ПОЯСА, п));

/* «timedatectl show» — строки ключ=значение */
export function разбери_timedatectl(текст){
  const итог = {};
  for (const с of String(текст).split('\n')){
    const м = с.match(/^(\w+)=(.*)$/);
    if (м) итог[м[1]] = м[2];
  }
  return { пояс:итог.Timezone || '', местные:итог.LocalRTC === 'yes', ntp:итог.NTP === 'yes',
    сверено:итог.NTPSynchronized === 'yes', служба:итог.CanNTP === 'yes' };
}

const без_пояса = п => !п || /^(Etc\/)?(UTC|GMT|Universal|Zulu)$/.test(п);

/* Пояс по городу погоды, а нет — по адресу в сети */
export async function определи_пояс(город, достань = fetch){
  const попробуй = async (адрес, вынь) => {
    try {
      const о = await достань(адрес, { signal:AbortSignal.timeout(10000) });
      if (!о.ok) return null;
      const п = await вынь(о);
      return годный_пояс(п) ? п : null;
    } catch(e){ return null; }
  };
  if (город){
    const п = await попробуй('https://geocoding-api.open-meteo.com/v1/search?count=1&language=ru&name=' + encodeURIComponent(город),
      async о => ((await о.json()).results || [])[0]?.timezone);
    if (п) return п;
  }
  return await попробуй('http://ip-api.com/line/?fields=timezone', async о => (await о.text()).trim())
      || await попробуй('https://ipapi.co/timezone', async о => (await о.text()).trim());
}

export async function город_погоды(){
  const путь = join(process.env.XDG_CONFIG_HOME || join(os.homedir(), '.config'), 'glower', 'settings.json');
  try { return String(JSON.parse(await readFile(путь, 'utf8')).city || '').trim(); } catch(e){ return ''; }
}

export function clock(){
  async function состояние(){
    try { return разбери_timedatectl((await run('timedatectl', ['show'], { timeout:10000 })).stdout); }
    catch(e){ return null; }
  }

  async function почини(){
    const с = await состояние();
    if (!с) return { ok:false, почему:'timedatectl не ответил' };
    const сделано = [];
    if (без_пояса(с.пояс)){
      const пояс = await определи_пояс(await город_погоды());
      if (пояс && пояс !== с.пояс){
        await run('sudo', ['-n', 'timedatectl', 'set-timezone', пояс], { timeout:20000 });
        сделано.push('пояс ' + пояс);
        // Часы компьютера идут по местному: системное время было прочитано
        // из них как будто по Гринвичу — перечитываем, уже с верным поясом
        if (с.местные){
          await run('sudo', ['-n', 'hwclock', '--hctosys'], { timeout:20000 }).catch(() => {});
          сделано.push('время перечитано из часов компьютера');
        }
      }
    }
    if (с.служба && !с.ntp){
      await run('sudo', ['-n', 'timedatectl', 'set-ntp', 'true'], { timeout:20000 }).catch(() => {});
      сделано.push('сверка времени по сети');
    }
    return { ok:true, сделано, было:с };
  }

  // Сразу после входа сеть бывает ещё не поднялась — через полминуты
  const т = setTimeout(() => { почини().catch(() => {}); }, 30000);
  if (т.unref) т.unref();

  return {
    async 'sys.clock'(){ return await состояние() || {}; },
    async 'sys.clock.fix'(){ return await почини(); },
  };
}
