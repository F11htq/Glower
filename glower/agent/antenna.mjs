/* ==========================================================================
   Антенны Wi-Fi

   У двухантенной карты драйвер Linux по умолчанию принимает обеими. На
   части машин так хуже, чем одной: на Realtek RTL8852BE (драйвер rtw89)
   у человека на 5 ГГц обеими антеннами было 13–18 Мбит/с, одной второй —
   86, как в Windows. Windows-драйвер Realtek выбирает антенны сам, rtw89 —
   нет, и плохая ветвь приёма портит весь сигнал.

   Подбор: по очереди «обе», «первая», «вторая»; для каждой ждём
   подключения и меряем, сколько карта приняла за несколько секунд
   скачивания в четыре потока (счётчик байтов самой карты — ему всё равно,
   откуда трафик). Одна антенна побеждает, только если она заметно лучше
   обеих: иначе оставляем как было. Выбор пишется в /etc/glower/wifi-antenna,
   а при загрузке его применяет glower-wifi-antenna (зовёт udev).

   Сам подбор запускается однажды для карт rtw89 — у них это известная
   беда, — и только на безлимитной сети: он скачивает до пары сотен
   мегабайт. Для любой другой двухантенной карты — кнопкой в Параметрах.
   ========================================================================== */
import { execFile, spawn } from 'node:child_process';
import { readFile, readdir, readlink, writeFile, unlink } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { promisify } from 'node:util';
import os from 'node:os';

const run = promisify(execFile);
const КОНФ = process.env.GLOWER_ANTENNA_CONF || '/etc/glower/wifi-antenna';
const ЗАМЕР = 'https://speedtest.selectel.ru/100MB';
const ВАРИАНТЫ = [3, 1, 2];
export const НАЗВАНИЕ = { 1:'первая', 2:'вторая', 3:'обе' };

/* «Available Antennas: TX 0x3 RX 0x3», «Configured Antennas: TX 0 RX 0» */
export function разбери_антенны(текст){
  const м = (имя) => {
    const р = String(текст).match(new RegExp(имя + ' Antennas:\\s*TX\\s*(0x[0-9a-f]+|\\d+)\\s*RX\\s*(0x[0-9a-f]+|\\d+)', 'i'));
    return р ? { tx:Number(р[1]), rx:Number(р[2]) } : null;
  };
  return { доступно:м('Available'), настроено:м('Configured') };
}

/* Сохранённый выбор: строки «адрес маска» */
export function разбери_конф(текст){
  const итог = {};
  for (const с of String(текст).split('\n')){
    const м = с.trim().match(/^([0-9a-f]{2}(?::[0-9a-f]{2}){5})\s+([123])$/i);
    if (м) итог[м[1].toLowerCase()] = +м[2];
  }
  return итог;
}

export function собери_конф(выбор){
  return '# Антенны Wi-Fi: адрес карты и маска (1 — первая, 2 — вторая, 3 — обе).\n'
    + '# Подбирает GlowerOS: Параметры → Сеть → Антенны Wi-Fi\n'
    + Object.entries(выбор).map(([адрес, маска]) => адрес + ' ' + маска).join('\n') + '\n';
}

/* Одна антенна берётся, только если она заметно лучше обеих:
   на четверть и хотя бы на 5 Мбит/с — разброс замеров бывает и такой */
export function выбери(замеры){
  const обе = замеры.find(з => з.маска === 3);
  const лучшая = [...замеры].sort((а, б) => б.скорость - а.скорость)[0];
  if (!обе || !лучшая || лучшая.маска === 3) return 3;
  return лучшая.скорость > обе.скорость * 1.25 && лучшая.скорость - обе.скорость >= 5 ? лучшая.маска : 3;
}

async function карты(){
  const итог = [];
  let имена = [];
  try { имена = await readdir('/sys/class/net'); } catch(e){ return итог; }
  for (const имя of имена){
    let phy;
    try { phy = basename(await readlink(join('/sys/class/net', имя, 'phy80211'))); } catch(e){ continue; }
    const адрес = (await readFile(join('/sys/class/ieee80211', phy, 'macaddress'), 'utf8').catch(() => '')).trim().toLowerCase();
    const драйвер = await readlink(join('/sys/class/net', имя, 'device', 'driver')).then(basename).catch(() => '');
    const сведения = await run('iw', ['phy', phy, 'info'], { timeout:5000 }).then(р => р.stdout).catch(() => '');
    итог.push({ имя, phy, адрес, драйвер, ...разбери_антенны(сведения) });
  }
  return итог;
}

const двухантенная = к => к.доступно && (к.доступно.rx & 3) === 3 && (к.доступно.tx & 3) === 3;

async function сохранённые(){
  return разбери_конф(await readFile(КОНФ, 'utf8').catch(() => ''));
}

async function подключена(имя){
  const р = await run('iw', ['dev', имя, 'link'], { timeout:5000 }).then(р => р.stdout).catch(() => '');
  return /^Connected to/m.test(р);
}

async function принято(имя){
  return Number(await readFile(join('/sys/class/net', имя, 'statistics', 'rx_bytes'), 'utf8').catch(() => 0)) || 0;
}

const спи = мс => new Promise(р => setTimeout(р, мс));

async function поставь(имя, маска){
  await run('sudo', ['-n', '/usr/bin/glower-wifi-antenna', имя, String(маска)], { timeout:20000 });
}

/* Скачивание в четыре потока; сколько приняла карта за окно замера */
async function замерь(имя, секунд = 8){
  const потоки = [];
  for (let i = 0; i < 4; i++){
    const п = spawn('curl', ['-s', '--max-time', String(секунд + 2), '-o', '/dev/null', ЗАМЕР], { stdio:'ignore' });
    п.on('error', () => {});
    потоки.push(п);
  }
  await спи(1500);                              // разгон соединений не считаем
  const было = await принято(имя), с = Date.now();
  await спи(секунд * 1000);
  const стало = await принято(имя), прошло = (Date.now() - с) / 1000;
  for (const п of потоки) try { п.kill(); } catch(e){}
  return Math.round((стало - было) * 8 / прошло / 1e4) / 100;   // Мбит/с
}

async function безлимитная(имя){
  const р = await run('nmcli', ['-t', '-g', 'GENERAL.METERED', 'device', 'show', имя], { timeout:5000 })
    .then(р => р.stdout.trim()).catch(() => '');
  return !/^yes/i.test(р);
}

export function antenna(){
  let подбор = null;          // { имя, шаг, замеры, готово, ошибка, выбрано }

  async function запиши(адрес, маска){
    const выбор = await сохранённые();
    выбор[адрес] = маска;
    const врем = join(os.tmpdir(), 'glower-antenna-' + process.pid);
    await writeFile(врем, собери_конф(выбор));
    try {
      await run('sudo', ['-n', 'install', '-D', '-m', '0644', врем, КОНФ], { timeout:10000 });
    } finally { await unlink(врем).catch(() => {}); }
  }

  async function подбери(карта){
    подбор = { имя:карта.имя, шаг:'начинаю', замеры:[], готово:false, ошибка:null, выбрано:null };
    try {
      if (!await подключена(карта.имя)) throw new Error('Wi-Fi не подключён — подбирать не на чем');
      for (const маска of ВАРИАНТЫ){
        подбор.шаг = 'антенны: ' + НАЗВАНИЕ[маска];
        await поставь(карта.имя, маска);
        let есть = false;
        for (let i = 0; i < 40 && !(есть = await подключена(карта.имя)); i++) await спи(1000);
        if (есть) await спи(5000);              // адрес и маршрут после подключения
        const скорость = есть ? await замерь(карта.имя) : 0;
        подбор.замеры.push({ маска, скорость, связь:есть });
      }
      const маска = выбери(подбор.замеры);
      подбор.шаг = 'сохраняю';
      await поставь(карта.имя, маска);
      await запиши(карта.адрес, маска);
      подбор.выбрано = маска;
    } catch(e){
      подбор.ошибка = String(e.stderr || e.message || e).trim().split('\n')[0];
      await поставь(карта.имя, 3).catch(() => {});   // не оставляем карту на полпути
    }
    подбор.готово = true;
    подбор.шаг = '';
  }

  /* Однажды за сеанс — для rtw89 без сохранённого выбора: через две
     минуты после входа и потом, пока не будет сети, раз в пять минут */
  let сделано = false;
  async function сам(){
    if (сделано || (подбор && !подбор.готово)) return;
    const выбор = await сохранённые();
    for (const к of await карты()){
      if (!/^rtw89/.test(к.драйвер) || !двухантенная(к) || !к.адрес || выбор[к.адрес]) continue;
      if (!await подключена(к.имя) || !await безлимитная(к.имя)) return;
      сделано = true;
      await подбери(к);
      return;
    }
    сделано = true;
  }
  const т1 = setTimeout(() => сам().catch(() => {}), 120000);
  const т2 = setInterval(() => { if (сделано) clearInterval(т2); else сам().catch(() => {}); }, 300000);
  if (т1.unref) т1.unref();
  if (т2.unref) т2.unref();

  return {
    async 'sys.wifi.antenna'(){
      const выбор = await сохранённые();
      return {
        карты:(await карты()).filter(двухантенная).map(к => ({
          имя:к.имя, драйвер:к.драйвер,
          настроено:к.настроено ? к.настроено.rx : 0,
          сохранено:выбор[к.адрес] || null
        })),
        подбор
      };
    },
    async 'sys.wifi.antenna.tune'({ имя } = {}){
      if (подбор && !подбор.готово) throw new Error('подбор уже идёт');
      const все = (await карты()).filter(двухантенная);
      const карта = все.find(к => к.имя === имя) || все[0];
      if (!карта) throw new Error('двухантенной карты Wi-Fi нет');
      сделано = true;
      подбери(карта).catch(() => {});
      return { ok:true };
    },
    async 'sys.wifi.antenna.set'({ имя, маска }){
      маска = +маска;
      if (![1, 2, 3].includes(маска)) throw new Error('маска антенн — 1, 2 или 3');
      const карта = (await карты()).filter(двухантенная).find(к => к.имя === имя);
      if (!карта) throw new Error('нет такой карты Wi-Fi');
      await поставь(карта.имя, маска);
      await запиши(карта.адрес, маска);
      return { ok:true };
    }
  };
}
