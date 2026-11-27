/* ==========================================================================
   Магазин: всё, что человек видит о программе, — настоящее

   Витрина магазина живёт не цифрами из головы, а тем, что знает мир:
   — Flathub (flathub.org/api/v2): описание, скриншоты, что нового в
     последнем выпуске, разработчик, лицензия, возраст, сколько раз
     программу ставили (за всё время и за месяц), подборки «популярное»,
     «в тренде», «обновлённое», «новое»;
   — ODRS (odrs.gnome.org) — общий для GNOME Software и KDE Discover сбор
     оценок и отзывов: звёзды и тексты людей. Оценки всех программ разом
     приходят одним файлом (~2 МБ) — его берём раз в сутки и держим на
     диске, а отзывы — на странице программы.

   Отзывы спрашиваются от лица «никого»: хеш из нулей. ODRS для выдачи
   его не проверяет, а слать туда что-то о машине незачем.

   Ответы помним: сведения о программе — час, подборки — полчаса.
   ========================================================================== */
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

const FLATHUB = 'https://flathub.org/api/v2';
const ODRS = 'https://odrs.gnome.org/1.0/reviews/api';
const APPID = /^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+){2,}$/;
const ПАКЕТ = /^[a-z0-9][a-z0-9+.-]{0,80}$/;
const ЯЗЫК = /^[a-z]{2}$/;
const ВИДЫ = new Set(['popular', 'trending', 'recently-updated', 'recently-added']);
const папка = () => join(process.env.XDG_CACHE_HOME || join(homedir(), '.cache'), 'glower', 'магазин');

const помню = new Map();
async function запомни(ключ, срок, добудь){
  const было = помню.get(ключ);
  if (было && Date.now() - было.когда < срок) return было.что;
  const что = await добудь();
  помню.set(ключ, { когда:Date.now(), что });
  return что;
}

async function json(адрес, опции = {}){
  const о = await fetch(адрес, { signal:AbortSignal.timeout(15000), ...опции });
  if (о.status === 404) return null;
  if (!о.ok) throw new Error('Flathub ответил ' + о.status);
  return о.json();
}

/* ---------- из разметки appstream — в абзацы ---------- */
export function текст_из_html(html){
  if (!html) return '';
  return String(html)
    .replace(/<li[^>]*>/gi, '\n• ').replace(/<\/(p|ul|ol)>/gi, '\n\n').replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'")
    .split('\n').map(с => с.replace(/\s+/g, ' ').trim()).join('\n')
    .replace(/\n{3,}/g, '\n\n').trim();
}

/* Скриншот шириной поближе к 752 — для ленты на странице программы */
export function скриншоты(список){
  return (Array.isArray(список) ? список : []).map(с => {
    const размеры = (с.sizes || []).filter(р => р && р.src).map(р => ({ src:р.src, w:+р.width || 0, h:+р.height || 0 }));
    if (!размеры.length) return null;
    размеры.sort((а, б) => Math.abs(а.w - 752) - Math.abs(б.w - 752));
    return { адрес:размеры[0].src, подпись:с.caption || '' };
  }).filter(Boolean).filter(с => с.адрес.startsWith('https://dl.flathub.org/')).slice(0, 8);
}

/* Звёзды ODRS (star1…star5) → средняя и разброс */
export function оценка(з){
  if (!з || !з.total) return null;
  const звёзды = [1, 2, 3, 4, 5].map(н => +з['star' + н] || 0);
  const всего = звёзды.reduce((а, б) => а + б, 0);
  if (!всего) return null;
  const средняя = звёзды.reduce((с, к, i) => с + к * (i + 1), 0) / всего;
  return { средняя:Math.round(средняя * 10) / 10, всего, звёзды };
}

/* Для «Лучших по оценкам»: у программы с тремя пятёрками средняя 5,0, но
   верить ей меньше, чем 4,7 из тысячи. Байесовская средняя тянет малые
   выборки к общей средней */
export function вес_оценки(о, к = 50, общая = 3.8){
  if (!о) return 0;
  return (о.средняя * о.всего + общая * к) / (о.всего + к);
}

export function из_сведений(д, статистика = null){
  const выпуск = Array.isArray(д.releases) && д.releases[0] ? д.releases[0] : null;
  const возраст = Object.values(д.content_rating_details || {})[0];
  const м = д.metadata || {};
  return {
    ид:д.id, имя:д.name || д.id, кратко:д.summary || '',
    описание:текст_из_html(д.description),
    разработчик:д.developer_name || '',
    лицензия:д.project_license || '',
    свободная:д.is_free_license !== false,
    проверено:м['flathub::verification::verified'] === true || м['flathub::verification::verified'] === 'true',
    сайт:(д.urls && д.urls.homepage) || '',
    категории:д.categories || [],
    возраст:возраст && Number.isFinite(+возраст.minimumAge) ? +возраст.minimumAge : null,
    значок:д.icon || '',
    скриншоты:скриншоты(д.screenshots),
    выпуск:выпуск ? { версия:выпуск.version || '', когда:+выпуск.timestamp || null, что:текст_из_html(выпуск.description) } : null,
    загрузок:статистика ? +статистика.installs_total || 0 : null,
    за_месяц:статистика ? +статистика.installs_last_month || 0 : null,
  };
}

export function из_подборки(х){
  return { name:х.app_id || '', title:х.name || '', about:х.summary || '', source:'flatpak',
           за_месяц:+х.installs_last_month || 0, значок:х.icon || '', разработчик:х.developer_name || '',
           раздел:х.main_categories || '', свободная:х.is_free_license !== false };
}

export function из_отзыва(о){
  return { кто:о.user_display || 'Без имени', оценка:Math.round((+о.rating || 0) / 20),
           заголовок:о.summary || '', текст:о.description || '', когда:+о.date_created || null,
           версия:о.version || '' };
}

/* ---------- оценки всех программ: один файл на сутки ---------- */
let оценки = null;
async function все_оценки(){
  if (оценки) return оценки;
  const файл = join(папка(), 'odrs-ratings.json');
  try {
    const с = await stat(файл);
    if (Date.now() - с.mtimeMs < 86400000) return (оценки = JSON.parse(await readFile(файл, 'utf8')));
  } catch(e){}
  try {
    const о = await fetch(ODRS + '/ratings', { signal:AbortSignal.timeout(30000) });
    if (!о.ok) throw new Error(String(о.status));
    const т = await о.text();
    оценки = JSON.parse(т);
    await mkdir(папка(), { recursive:true }).catch(() => {});
    await writeFile(файл, т).catch(() => {});
  } catch(e){
    // Сеть не ответила — старый файл лучше, чем ничего
    try { оценки = JSON.parse(await readFile(файл, 'utf8')); } catch(e2){ оценки = {}; }
  }
  return оценки;
}

/* Программу ODRS знает под её id, а старые — по имени ярлыка; и регистр у
   него свой: Firefox там org.mozilla.Firefox, а на Flathub —
   org.mozilla.firefox. Записей бывает две (id и id.desktop) — берём ту, где
   оценок больше */
const УКАЗАТЕЛЬ = new WeakMap();
export function найди_оценку(все, ид){
  if (!все || !ид) return null;
  let у = УКАЗАТЕЛЬ.get(все);
  if (!у){
    у = new Map();
    for (const [к, з] of Object.entries(все)){
      const н = к.toLowerCase();
      if (!у.has(н) || (+з.total || 0) > (+у.get(н).total || 0)) у.set(н, з);
    }
    УКАЗАТЕЛЬ.set(все, у);
  }
  const н = String(ид).toLowerCase();
  const есть = [у.get(н), у.get(н + '.desktop')].filter(Boolean).sort((а, б) => (+б.total || 0) - (+а.total || 0));
  return есть.length ? оценка(есть[0]) : null;
}

/* ---------- скриншоты картинками ---------- */
async function картинка(адрес){
  if (!/^https:\/\/dl\.flathub\.org\/[\w./@%-]+\.(png|jpe?g|webp)$/i.test(адрес)) throw new Error('такой адрес не скриншот Flathub');
  const имя = адрес.replace(/^https:\/\/dl\.flathub\.org\//, '').replace(/[^\w.@-]+/g, '_');
  const файл = join(папка(), 'снимки', имя);
  const отдай = б => ({ 'есть':true, 'данные':'data:image/' + (/png$/i.test(адрес) ? 'png' : 'jpeg') + ';base64,' + б.toString('base64') });
  try { return отдай(await readFile(файл)); } catch(e){}
  const о = await fetch(адрес, { signal:AbortSignal.timeout(20000) });
  if (!о.ok) return { 'есть':false };
  const б = Buffer.from(await о.arrayBuffer());
  if (б.length > 8 << 20) return { 'есть':false };
  await mkdir(join(папка(), 'снимки'), { recursive:true }).catch(() => {});
  await writeFile(файл, б).catch(() => {});
  return отдай(б);
}

const язык_из = я => ЯЗЫК.test(String(я || '')) ? String(я) : 'ru';

export function store(){
  return {
    /* Всё о программе Flathub — для её страницы */
    async 'store.details'({ ид, язык }){
      const id = String(ид || '');
      if (!APPID.test(id)) throw new Error('неверный id программы');
      const я = язык_из(язык);
      return запомни('сведения|' + id + '|' + я, 3600000, async () => {
        const [д, с] = await Promise.all([
          json(`${FLATHUB}/appstream/${encodeURIComponent(id)}?locale=${я}`),
          json(`${FLATHUB}/stats/${encodeURIComponent(id)}`).catch(() => null)
        ]);
        if (!д) return { нет:true };
        const итог = из_сведений(д, с);
        итог.оценка = найди_оценку(await все_оценки(), id);
        return итог;
      });
    },

    /* Отзывы людей — отдельно: ODRS бывает медленнее Flathub */
    async 'store.reviews'({ ид, язык, сколько = 6 }){
      const id = String(ид || '');
      if (!APPID.test(id) && !ПАКЕТ.test(id)) throw new Error('неверный id программы');
      const я = язык_из(язык);
      return запомни('отзывы|' + id + '|' + я, 3600000, async () => {
        const спроси = async app_id => {
          const о = await fetch(ODRS + '/fetch', { method:'POST', headers:{ 'Content-Type':'application/json' },
            signal:AbortSignal.timeout(20000),
            body:JSON.stringify({ app_id, locale:я + '_' + я.toUpperCase(), distro:'GlowerOS', version:'unknown',
              user_hash:'0'.repeat(40), limit:Math.max(1, Math.min(20, +сколько || 6)) }) });
          if (!о.ok) return [];
          const с = await о.json();
          return Array.isArray(с) ? с : [];
        };
        let список = await спроси(id);
        if (!список.length && !APPID.test(id)) список = await спроси(id + '.desktop');
        return { list:список.filter(о => (о.description || о.summary) && !о.reported).map(из_отзыва) };
      });
    },

    /* Подборки Flathub: популярное (по установкам за месяц), в тренде,
       обновлённое, новое — страницами */
    async 'store.collection'({ вид = 'popular', страница = 1, сколько = 24, язык }){
      if (!ВИДЫ.has(вид)) throw new Error('нет такой подборки');
      const с = Math.max(1, Math.min(200, +страница || 1));
      const н = Math.max(1, Math.min(60, +сколько || 24));
      const я = язык_из(язык);
      return запомни(['подборка', вид, с, н, я].join('|'), 1800000, async () => {
        const о = await json(`${FLATHUB}/collection/${вид}?page=${с}&per_page=${н}&locale=${я}`);
        const все = await все_оценки();
        const list = ((о && о.hits) || []).map(из_подборки).filter(х => APPID.test(х.name))
          .map(х => ({ ...х, оценка:найди_оценку(все, х.name) }));
        return { list, страниц:+(о && о.totalPages) || 1, страница:с };
      });
    },

    /* Оценки пачкой: для карточек, что пришли не из подборок */
    async 'store.ratings'({ ids }){
      const все = await все_оценки();
      const итог = {};
      for (const ид of (Array.isArray(ids) ? ids : []).slice(0, 300)){
        const id = String(ид);
        if (!APPID.test(id) && !ПАКЕТ.test(id)) continue;
        итог[id] = найди_оценку(все, id);
      }
      return итог;
    },

    async 'store.shot'({ адрес }){ return картинка(String(адрес || '')); }
  };
}
