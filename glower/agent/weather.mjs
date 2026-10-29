/* ==========================================================================
   Погода из нескольких источников

   Прежде погода шла только от Open-Meteo. У части российских провайдеров
   он недоступен: интернет есть, обновления качаются, а виджет пишет «нет
   связи с сервисом погоды». Теперь источников три, и берётся первый, что
   ответил:
     — Open-Meteo: точный, с прогнозом на неделю;
     — met.no, Норвежский метеоинститут, — бесплатный, государственный,
       открыт для всех; город находим через OpenStreetMap;
     — wttr.in — запасной: город понимает сам, но прогноз только на два дня.
   Какой источник сработал, помним и в следующий раз спрашиваем его первым.

   Все три отвечают по-разному, а виджет ждёт одно: сейчас (температура,
   значок, слово, влажность, ветер в км/ч) и пять следующих дней. Коды погоды
   приводим к кодам WMO, как у Open-Meteo, — по ним и рисуется значок.
   ========================================================================== */

const UA = { 'User-Agent':'GlowerOS weather (github.com/F11htq/glower)' };

export const знак = c => c === 0 ? '☀️' : c <= 2 ? '🌤' : c === 3 ? '☁️' : c <= 48 ? '🌫' : c <= 57 ? '🌦'
  : c <= 67 ? '🌧' : c <= 77 ? '❄️' : c <= 82 ? '🌧' : c <= 86 ? '🌨' : '⛈';
export const слово = c => c === 0 ? 'Ясно' : c <= 2 ? 'Малооблачно' : c === 3 ? 'Пасмурно' : c <= 48 ? 'Туман'
  : c <= 57 ? 'Морось' : c <= 67 ? 'Дождь' : c <= 77 ? 'Снег' : c <= 82 ? 'Ливень' : c <= 86 ? 'Снегопад' : 'Гроза';
const ДНИ = ['Вс', 'Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб'];

/* Дата «ГГГГ-ММ-ДД» по местному времени машины */
const местнаяДата = т => {
  const д = new Date(т);
  return `${д.getFullYear()}-${String(д.getMonth() + 1).padStart(2, '0')}-${String(д.getDate()).padStart(2, '0')}`;
};
const деньНедели = дата => ДНИ[new Date(дата + 'T12:00:00').getDay()];

function итог(город, код, t, влажность, ветер, дни){
  return { ts:Date.now(), 'город':город, t:Math.round(t), 'знак':знак(код), 'слово':слово(код),
    'влажность':Math.round(влажность ?? 0), 'ветер':Math.round((ветер ?? 0) * 10) / 10,
    'дни':дни.slice(0, 5).map(д => ({ 'день':деньНедели(д.дата), 'знак':знак(д.код), max:Math.round(д.max), min:Math.round(д.min) })) };
}

/* ---------- Open-Meteo ---------- */

export function изOpenMeteo(w, город){
  const д = w.daily;
  const дни = д.time.slice(1).map((дата, i) => ({ дата, код:д.weather_code[i + 1],
    max:д.temperature_2m_max[i + 1], min:д.temperature_2m_min[i + 1] }));
  return итог(город, w.current.weather_code, w.current.temperature_2m,
    w.current.relative_humidity_2m, w.current.wind_speed_10m, дни);
}

/* ---------- met.no ---------- */

/* clearsky_day, lightrainshowers_night, heavysnow… → код WMO */
export function кодMetNo(символ){
  const с = String(символ || '').replace(/_(day|night|polartwilight)$/, '');
  if (/thunder/.test(с)) return 95;
  if (/sleet/.test(с)) return 67;
  if (/snowshowers/.test(с)) return 85;
  if (/snow/.test(с)) return 73;
  if (/rainshowers/.test(с)) return 81;
  if (/rain/.test(с)) return 63;
  if (с === 'fog') return 45;
  if (с === 'partlycloudy') return 2;
  if (с === 'fair') return 1;
  if (с === 'clearsky') return 0;
  return 3;
}

export function изMetNo(j, город, сейчас = Date.now()){
  const ряд = (j.properties && j.properties.timeseries) || [];
  if (!ряд.length) throw new Error('пустой ответ met.no');
  const символ = т => т.data.next_1_hours?.summary?.symbol_code || т.data.next_6_hours?.summary?.symbol_code;
  const первый = ряд[0];
  const в = первый.data.instant.details;
  // Ветер у met.no — в метрах в секунду, у нас — в км/ч
  const ветер = (в.wind_speed ?? 0) * 3.6;
  const сегодня = местнаяДата(сейчас);
  const по_дням = new Map();
  for (const т of ряд){
    const дата = местнаяДата(т.time);
    if (дата <= сегодня) continue;
    const t = т.data.instant.details.air_temperature;
    const час = new Date(т.time).getHours();
    const д = по_дням.get(дата) || { дата, max:-Infinity, min:Infinity, код:null, близость:99 };
    д.max = Math.max(д.max, t);
    д.min = Math.min(д.min, t);
    // Значок дня — по погоде около полудня
    const с = символ(т);
    if (с && Math.abs(час - 12) < д.близость){ д.близость = Math.abs(час - 12); д.код = кодMetNo(с); }
    по_дням.set(дата, д);
  }
  const дни = [...по_дням.values()].filter(д => д.код !== null).sort((а, б) => а.дата < б.дата ? -1 : 1);
  return итог(город, кодMetNo(символ(первый)), в.air_temperature, в.relative_humidity, ветер, дни);
}

/* ---------- wttr.in ---------- */

/* Коды World Weather Online → WMO */
export function кодWttr(код){
  const c = Number(код);
  if (c === 113) return 0;
  if (c === 116) return 2;
  if (c === 119 || c === 122) return 3;
  if ([143, 248, 260].includes(c)) return 45;
  if ([200, 386, 389, 392, 395].includes(c)) return 95;
  if ([263, 266, 281, 284].includes(c)) return 53;
  if ([368, 371].includes(c)) return 85;
  if ([179, 227, 230, 323, 326, 329, 332, 335, 338].includes(c)) return 73;
  if ([353, 356, 359].includes(c)) return 81;
  if ([182, 185, 311, 314, 317, 320, 350, 362, 365, 374, 377].includes(c)) return 67;
  if ([176, 293, 296, 299, 302, 305, 308].includes(c)) return 63;
  return 3;
}

export function изWttr(j, город){
  const с = j.current_condition && j.current_condition[0];
  if (!с) throw new Error('пустой ответ wttr.in');
  const дни = (j.weather || []).slice(1).map(д => ({ дата:д.date,
    код:кодWttr((д.hourly && (д.hourly[4] || д.hourly[0]) || {}).weatherCode),
    max:Number(д.maxtempC), min:Number(д.mintempC) }));
  return итог(город, кодWttr(с.weatherCode), Number(с.temp_C), Number(с.humidity), Number(с.windspeedKmph), дни);
}

/* ---------- выбор источника ---------- */

const МЕСТА = new Map();   // город → { name, lat, lon }: координаты не меняются
let первыйИсточник = 'open-meteo';

async function место(где, возьми){
  if (МЕСТА.has(где)) return МЕСТА.get(где);
  let м = null;
  try {
    const g = await возьми('https://geocoding-api.open-meteo.com/v1/search?count=1&language=ru&name=' + encodeURIComponent(где));
    const р = g.results && g.results[0];
    if (!р) throw Object.assign(new Error('город не найден: ' + где), { нет:true });
    м = { name:р.name, lat:р.latitude, lon:р.longitude };
  } catch(e){
    if (e.нет) throw e;
    const n = await возьми('https://nominatim.openstreetmap.org/search?format=json&limit=1&accept-language=ru&q=' + encodeURIComponent(где), { headers:UA });
    const р = n && n[0];
    if (!р) throw Object.assign(new Error('город не найден: ' + где), { нет:true });
    м = { name:р.name || String(р.display_name || где).split(',')[0], lat:+р.lat, lon:+р.lon };
  }
  МЕСТА.set(где, м);
  return м;
}

const ИСТОЧНИКИ = {
  async 'open-meteo'(где, возьми){
    const м = await место(где, возьми);
    const w = await возьми(`https://api.open-meteo.com/v1/forecast?latitude=${м.lat}&longitude=${м.lon}`
      + `&current=temperature_2m,weather_code,relative_humidity_2m,wind_speed_10m`
      + `&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=6`);
    return изOpenMeteo(w, м.name);
  },
  async 'met.no'(где, возьми){
    const м = await место(где, возьми);
    const j = await возьми(`https://api.met.no/weatherapi/locationforecast/2.0/compact?lat=${м.lat.toFixed(4)}&lon=${м.lon.toFixed(4)}`, { headers:UA });
    return изMetNo(j, м.name);
  },
  async 'wttr.in'(где, возьми){
    const j = await возьми(`https://wttr.in/${encodeURIComponent(где)}?format=j1&lang=ru`, { headers:UA });
    return изWttr(j, где);
  },
};

/* возьми(адрес, { headers }) → JSON */
export async function погода(где, возьми){
  const порядок = [первыйИсточник, ...Object.keys(ИСТОЧНИКИ).filter(к => к !== первыйИсточник)];
  const беды = [];
  for (const имя of порядок){
    try {
      const о = await ИСТОЧНИКИ[имя](где, возьми);
      первыйИсточник = имя;
      return { ...о, 'источник':имя };
    } catch(e){
      if (e.нет) throw e;          // город не нашёлся — другой источник не поможет
      беды.push(`${имя}: ${e.message}`);
    }
  }
  console.error('погода: ' + беды.join('; '));
  throw new Error('нет связи с сервисом погоды');
}
