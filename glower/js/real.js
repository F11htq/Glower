/* ==========================================================================
   Настоящие данные вместо выдуманных:
   сеть, батарея, хранилище, разрешения, устройства, дисплей, погода
   ========================================================================== */
'use strict';

const Real = {
  /* ---------- сеть ---------- */
  net(){
    const c = navigator.connection || navigator.mozConnection || navigator.webkitConnection;
    return {
      online: navigator.onLine,
      type: c && c.effectiveType,          // 4g / 3g / 2g / slow-2g
      downlink: c && c.downlink,           // Мбит/с, оценка браузера
      rtt: c && c.rtt,                     // мс
      saveData: c && c.saveData,
      supported: !!c
    };
  },

  /* ---------- батарея ---------- */
  async battery(){
    if (!navigator.getBattery) return null;
    try {
      const b = await navigator.getBattery();
      return { level:Math.round(b.level * 100), charging:b.charging,
               timeLeft:b.dischargingTime, timeFull:b.chargingTime, raw:b };
    } catch(e){ return null; }
  },

  /* ---------- хранилище ---------- */
  async storage(){
    const ls = (() => { try { return JSON.stringify(localStorage).length; } catch(e){ return 0; } })();
    if (!navigator.storage || !navigator.storage.estimate) return { ls, quota:null, usage:null };
    try {
      const e = await navigator.storage.estimate();
      const persisted = navigator.storage.persisted ? await navigator.storage.persisted() : false;
      return { ls, quota:e.quota, usage:e.usage, persisted };
    } catch(e){ return { ls, quota:null, usage:null }; }
  },
  async persist(){
    if (!navigator.storage || !navigator.storage.persist) return false;
    try { return await navigator.storage.persist(); } catch(e){ return false; }
  },

  /* ---------- разрешения ---------- */
  async perm(name){
    if (!navigator.permissions) return 'unknown';
    try { return (await navigator.permissions.query({ name })).state; }
    catch(e){ return 'unknown'; }
  },
  async askCamera(){
    try { const s = await navigator.mediaDevices.getUserMedia({ video:true });
      s.getTracks().forEach(t => t.stop()); return true; } catch(e){ return false; }
  },
  async askMic(){
    try { const s = await navigator.mediaDevices.getUserMedia({ audio:true });
      s.getTracks().forEach(t => t.stop()); return true; } catch(e){ return false; }
  },
  askGeo(){
    return new Promise(res => {
      if (!navigator.geolocation) return res(null);
      navigator.geolocation.getCurrentPosition(
        p => res({ lat:p.coords.latitude, lon:p.coords.longitude, acc:p.coords.accuracy }),
        () => res(null), { timeout:8000 });
    });
  },

  /* ---------- устройства ---------- */
  async media(){
    if (!navigator.mediaDevices || !navigator.mediaDevices.enumerateDevices) return [];
    try { return await navigator.mediaDevices.enumerateDevices(); } catch(e){ return []; }
  },
  gamepads(){
    try { return [...(navigator.getGamepads ? navigator.getGamepads() : [])].filter(Boolean); }
    catch(e){ return []; }
  },
  input(){
    return {
      touch: navigator.maxTouchPoints > 0,
      points: navigator.maxTouchPoints || 0,
      fine: matchMedia('(pointer:fine)').matches,
      hover: matchMedia('(hover:hover)').matches,
      keyboard: true
    };
  },
  bluetooth(){ return { api: !!navigator.bluetooth, secure: isSecureContext }; },
  async btPick(){
    if (!navigator.bluetooth) return { error:'API недоступен' };
    try { const d = await navigator.bluetooth.requestDevice({ acceptAllDevices:true });
      return { name:d.name || 'без имени', id:d.id }; }
    catch(e){ return { error:e.name === 'NotFoundError' ? 'устройство не выбрано' : e.message }; }
  },

  /* ---------- дисплей ---------- */
  display(){
    return {
      w:screen.width, h:screen.height, aw:innerWidth, ah:innerHeight,
      dpr:devicePixelRatio || 1, depth:screen.colorDepth,
      orient:(screen.orientation && screen.orientation.type) || (innerWidth > innerHeight ? 'landscape' : 'portrait'),
      dark:matchMedia('(prefers-color-scheme: dark)').matches,
      reduce:matchMedia('(prefers-reduced-motion: reduce)').matches,
      contrast:matchMedia('(prefers-contrast: more)').matches,
      hdr:matchMedia('(dynamic-range: high)').matches
    };
  },
  refreshRate(){
    return new Promise(res => {
      let n = 0; const t0 = performance.now();
      const tick = () => { n++;
        if (performance.now() - t0 < 500) requestAnimationFrame(tick);
        else res(Math.round(n / ((performance.now() - t0) / 1000)));
      };
      requestAnimationFrame(tick);
    });
  },

  /* ---------- система ---------- */
  system(){
    const ua = navigator.userAgentData;
    return {
      cores:navigator.hardwareConcurrency || null,
      mem:navigator.deviceMemory || null,
      platform:(ua && ua.platform) || navigator.platform || 'н/д',
      mobile:(ua && ua.mobile) || /Mobi|Android/i.test(navigator.userAgent),
      engine:navigator.userAgent.includes('Firefox') ? 'Gecko'
        : (navigator.userAgent.includes('Safari') && !navigator.userAgent.includes('Chrome')) ? 'WebKit' : 'Blink',
      lang:navigator.language, langs:navigator.languages,
      tz:Intl.DateTimeFormat().resolvedOptions().timeZone,
      secure:isSecureContext, cookies:navigator.cookieEnabled
    };
  }
};
window.Real = Real;

/* ==========================================================================
   Погода: настоящий прогноз через Open-Meteo, если есть сеть
   ========================================================================== */
const Weather = {
  key:'weather.cache',
  busy:false,
  data(){ return KV.get(this.key, null); },
  fresh(){ const d = this.data(); return d && Date.now() - d.ts < 3600000 && d.city === S.city; },

  async load(force){
    if (this.busy || (!force && this.fresh())) return this.data();
    if (!navigator.onLine) return this.data();
    this.busy = true;
    try {
      const g = await fetch('https://geocoding-api.open-meteo.com/v1/search?count=1&language=ru&name='
        + encodeURIComponent(S.city)).then(r => r.json());
      const place = g.results && g.results[0];
      if (!place) throw new Error('город не найден');
      const w = await fetch(`https://api.open-meteo.com/v1/forecast?latitude=${place.latitude}` +
        `&longitude=${place.longitude}&current=temperature_2m,weather_code,relative_humidity_2m,wind_speed_10m` +
        `&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=6`).then(r => r.json());
      const d = {
        ts:Date.now(), city:S.city, real:true,
        name:place.name + (place.admin1 ? ', ' + place.admin1 : ''),
        t:Math.round(w.current.temperature_2m),
        code:w.current.weather_code,
        hum:w.current.relative_humidity_2m,
        wind:w.current.wind_speed_10m,
        days:w.daily.time.slice(1, 6).map((dt, i) => ({
          d:['Вс','Пн','Вт','Ср','Чт','Пт','Сб'][new Date(dt).getDay()],
          code:w.daily.weather_code[i + 1],
          max:Math.round(w.daily.temperature_2m_max[i + 1]),
          min:Math.round(w.daily.temperature_2m_min[i + 1])
        }))
      };
      KV.set(this.key, d);
      Shell.renderDeskWidgets && Shell.renderDeskWidgets();
      Shell.renderWidgets && Shell.renderWidgets();
      Shell.clock && Shell.clock();
      return d;
    } catch(e){
      return this.data();
    } finally { this.busy = false; }
  },

  /* коды WMO → значок и описание */
  icon(c){
    if (c === 0) return '☀️'; if (c <= 2) return '🌤'; if (c === 3) return '☁️';
    if (c <= 48) return '🌫'; if (c <= 57) return '🌦'; if (c <= 67) return '🌧';
    if (c <= 77) return '❄️'; if (c <= 82) return '🌧'; if (c <= 86) return '🌨';
    return '⛈';
  },
  desc(c){
    if (c === 0) return 'Ясно'; if (c <= 2) return 'Малооблачно'; if (c === 3) return 'Пасмурно';
    if (c <= 48) return 'Туман'; if (c <= 57) return 'Морось'; if (c <= 67) return 'Дождь';
    if (c <= 77) return 'Снег'; if (c <= 82) return 'Ливень'; if (c <= 86) return 'Снегопад';
    return 'Гроза';
  }
};
window.Weather = Weather;

/* подменяем демо-погоду настоящей, если она загрузилась */
(function patchWeather(){
  const demo = Shell.weather.bind(Shell);
  Shell.weather = function(){
    const d = Weather.data();
    if (d && d.real && d.city === S.city){
      return { t:d.t, ico:Weather.icon(d.code), desc:Weather.desc(d.code), real:true, hum:d.hum, wind:d.wind,
        days:d.days.map(x => ({ d:x.d, i:Weather.icon(x.code), t:x.max })) };
    }
    const w = demo(); w.real = false; return w;
  };
  Weather.load();
  addEventListener('online', () => Weather.load(true));
  setInterval(() => Weather.load(), 900000);
})();
