/* ==========================================================================
   Состояние системы: настройки, обои, виртуальная ФС, утилиты
   ========================================================================== */
'use strict';

const $  = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const el = (tag, cls, html) => { const n = document.createElement(tag); if (cls) n.className = cls; if (html != null) n.innerHTML = html; return n; };
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const pad2 = n => String(n).padStart(2, '0');
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

/* ---------- Обои ---------- */
const WALLPAPERS = [
  { id:'bloom',   name:'Bloom',    css:'radial-gradient(120% 120% at 18% 12%,#4f6bff 0%,#8b3dff 32%,#ff4d9d 58%,#ff9d4d 78%,#151a2e 100%)' },
  { id:'aurora',  name:'Aurora',   css:'radial-gradient(90% 90% at 78% 18%,#00e0c6 0%,#0f8bff 38%,#7a3cff 66%,#0a0f22 100%)' },
  { id:'dusk',    name:'Сумерки',  css:'linear-gradient(200deg,#f7b267 0%,#f4845f 22%,#c1428a 48%,#5b2a86 72%,#101433 100%)' },
  { id:'mono',    name:'Графит',   css:'radial-gradient(110% 110% at 30% 20%,#3b4256 0%,#22273a 40%,#0d1018 100%)' },
  { id:'mint',    name:'Мята',     css:'radial-gradient(100% 100% at 20% 80%,#a8ffdf 0%,#3fd0c9 30%,#1a6f9e 65%,#0b1730 100%)' },
  { id:'sunrise', name:'Рассвет',  css:'linear-gradient(160deg,#ffd6a5 0%,#ff8fab 30%,#a06cd5 62%,#22164d 100%)' },
  { id:'deep',    name:'Глубина',  css:'radial-gradient(120% 90% at 50% 110%,#1f6feb 0%,#12306b 42%,#050a18 100%)' },
  { id:'sakura',  name:'Сакура',   css:'radial-gradient(100% 100% at 75% 15%,#ffe3ee 0%,#ffb3d1 25%,#c86dd7 58%,#2b1b45 100%)' },
  { id:'forest',  name:'Лес',      css:'radial-gradient(110% 110% at 25% 15%,#b6e388 0%,#3ba55d 35%,#12603f 65%,#06180f 100%)' },
  { id:'ember',   name:'Уголь',    css:'radial-gradient(120% 120% at 80% 85%,#ff6b35 0%,#c1121f 35%,#5a0b1a 65%,#0a0509 100%)' },
  { id:'ice',     name:'Лёд',      css:'linear-gradient(150deg,#e0f7ff 0%,#96d9ff 28%,#3d84c6 60%,#12274a 100%)' },
  { id:'cosmos',  name:'Космос',   css:'radial-gradient(70% 70% at 30% 30%,#5b2a86 0%,#1b1b3a 40%,#05060f 100%),radial-gradient(50% 50% at 80% 70%,#ff4d9d33,transparent 70%)' }
];

/* ---------- Акценты ---------- */
const ACCENTS = [
  { n:'Синий',    a:'#3a86ff', b:'#8b5cf6' },
  { n:'Фиолет',   a:'#8b5cf6', b:'#ec4899' },
  { n:'Розовый',  a:'#f472b6', b:'#f97316' },
  { n:'Красный',  a:'#ef4444', b:'#f59e0b' },
  { n:'Оранж',    a:'#f97316', b:'#facc15' },
  { n:'Зелёный',  a:'#22c55e', b:'#14b8a6' },
  { n:'Бирюза',   a:'#06b6d4', b:'#3b82f6' },
  { n:'Индиго',   a:'#6366f1', b:'#22d3ee' },
  { n:'Графит',   a:'#64748b', b:'#94a3b8' },
  { n:'Золото',   a:'#d4a017', b:'#f5d76e' }
];

/* ---------- Настройки по умолчанию ---------- */
/* Время в выбранном часовом поясе: сдвигаем момент так, чтобы обычные
   getHours() и getMinutes() показывали время нужного пояса. Пустая
   настройка означает «как на машине» — тогда ничего не трогаем. */
window.zoned = function(d){
  d = d || new Date();
  const tz = typeof S === 'undefined' ? '' : S.tz;
  if (!tz) return d;
  try { return new Date(d.toLocaleString('en-US', { timeZone:tz })); }
  catch(e){ return d; }
};

const DEFAULTS = {
  theme:'dark', autoTheme:false,
  accent:0, accentCustom:null,
  wallpaper:'bloom', wallShuffle:false,
  radius:18, winRadius:16,
  speed:1, reduceMotion:false,
  dockSize:52, dockAutohide:false, dockPos:'bottom',
  wctl:'win',                 // win | mac
  showDesktopIcons:true, showDeskWidgets:true, taskbarFull:true, trayInDock:true,
  deskWidgets:[{t:'clock'},{t:'weather'}], autostart:[],
  font:"'Segoe UI Variable','Segoe UI',system-ui,sans-serif",
  clock24:true, showSeconds:false, tz:'',   /* пусто — часовой пояс машины */
  volume:62, brightness:100, wifi:true, bluetooth:true, dnd:false, nightLight:false,
  airdrop:true, hotcorners:true, sounds:false, soundNotif:true,
  userName:((window.__profiles || []).find(p => p.id === window.__profile) || {}).name || 'Пользователь', city:'Москва',
  snapAssist:true, tapClickSound:false,
  pinned:['settings','notepad','files','browser','calc','term','paint','photos','music','calendar','clock','store'],
  dockApps:['browser','files','notepad','settings','music','term','paint','calc']
};

const Store = {
  key:'glower.' + (window.__ns || '') + 'settings.v1',
  s:{},
  load(){ try{ this.s = { ...DEFAULTS, ...JSON.parse(localStorage.getItem(this.key) || '{}') }; }
          catch(e){ this.s = { ...DEFAULTS }; } return this.s; },
  save(){ try{ localStorage.setItem(this.key, JSON.stringify(this.s)); }catch(e){} },
  set(k, v){ this.s[k] = v; this.save(); applySettings(); },
  reset(){ this.s = { ...DEFAULTS }; this.save(); applySettings(); }
};
const S = Store.load();

/* Прозрачной темы в системе больше нет: у кого она была — становится тёмной */
if (S.theme === 'glass'){ S.theme = 'dark'; Store.save(); }
function KV_SAFE(k){ try { return localStorage.getItem(k); } catch(e){ return null; } }

/* ---------- Применение настроек ---------- */
function accentPair(){
  if (S.accentCustom) return { a:S.accentCustom, b:S.accentCustom };
  return ACCENTS[S.accent] || ACCENTS[0];
}
function applySettings(){
  const r = document.documentElement.style, ac = accentPair();
  document.documentElement.dataset.theme = S.theme;
  document.body.dataset.wctl = S.wctl;
  r.setProperty('--accent', ac.a);
  r.setProperty('--accent-2', ac.b);
  r.setProperty('--accent-rgb', hexToRgb(ac.a));
  r.setProperty('--radius', S.radius + 'px');
  r.setProperty('--radius-win', S.winRadius + 'px');
  r.setProperty('--speed', S.reduceMotion ? 0.01 : S.speed);
  r.setProperty('--dock-size', S.dockSize + 'px');
  r.setProperty('--font', S.font);
  document.body.classList.toggle('reduced', S.reduceMotion);
  document.body.classList.toggle('dock-hidden', S.dockAutohide && !document.body.classList.contains('dock-peek'));
  const di = $('#desktop-icons'); if (di) di.style.display = S.showDesktopIcons ? '' : 'none';
  const dw = $('#desk-widgets');  if (dw) dw.style.display = S.showDeskWidgets ? '' : 'none';
  const wp = $('#wallpaper');
  if (wp){
    const w = WALLPAPERS.find(x => x.id === S.wallpaper) || WALLPAPERS[0];
    wp.style.backgroundImage = (S.wallpaper === 'custom' && window.__customWall)
      ? `url(${window.__customWall})` : w.css;
    const dim = S.theme === 'dark' ? 0.55 : 1;      // в настоящей тёмной теме обои приглушены
    wp.style.filter = `brightness(${S.brightness / 100 * dim}) ${S.theme === 'dark' ? 'saturate(.8)' : ''} ${S.nightLight ? 'sepia(.35) saturate(1.2) hue-rotate(-14deg)' : ''}`;
  }
  document.body.style.filter = S.nightLight ? 'sepia(.14) saturate(1.06)' : '';
}
function hexToRgb(h){
  const m = h.replace('#','');
  const n = parseInt(m.length === 3 ? m.split('').map(c => c + c).join('') : m, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].join(',');
}

/* ==========================================================================
   Виртуальная файловая система
   ========================================================================== */
const FS = {
  key:'glower.' + (window.__ns || '') + 'fs.v1',
  root:null,
  load(){
    try { this.root = JSON.parse(localStorage.getItem(this.key)); } catch(e){}
    if (!this.root) this.root = this.seed();
    return this.root;
  },
  seed(){
    return { type:'dir', name:'', children:{
      'Рабочий стол':{ type:'dir', name:'Рабочий стол', children:{
        'Заметка.txt':{ type:'file', name:'Заметка.txt', body:'Привет!\n\nЭто GlowerOS.\nWin — меню Пуск, Win+Space — поиск, ПКМ по столу — меню.' }
      }},
      'Документы':{ type:'dir', name:'Документы', children:{
        'Идеи.txt':{ type:'file', name:'Идеи.txt', body:'— добавить свои обои\n— выбрать цвет акцента в Параметрах\n— написать что-нибудь тут' },
        'Проекты':{ type:'dir', name:'Проекты', children:{
          'readme.md':{ type:'file', name:'readme.md', body:'# Проект\n\nОписание проекта.' }
        }}
      }},
      'Изображения':{ type:'dir', name:'Изображения', children:{} },
      'Музыка':{ type:'dir', name:'Музыка', children:{} },
      'Загрузки':{ type:'dir', name:'Загрузки', children:{} }
    }};
  },
  save(){ try{ localStorage.setItem(this.key, JSON.stringify(this.root)); }catch(e){} },
  node(path){
    let n = this.root;
    for (const p of path.filter(Boolean)){ if (!n || n.type !== 'dir' || !n.children[p]) return null; n = n.children[p]; }
    return n;
  },
  write(path, name, body){
    const d = this.node(path); if (!d || d.type !== 'dir') return false;
    d.children[name] = { type:'file', name, body }; this.save(); return true;
  },
  mkdir(path, name){
    const d = this.node(path); if (!d || d.type !== 'dir' || d.children[name]) return false;
    d.children[name] = { type:'dir', name, children:{} }; this.save(); return true;
  },
  rm(path, name){
    const d = this.node(path); if (!d || !d.children[name]) return false;
    delete d.children[name]; this.save(); return true;
  },
  rename(path, name, nn){
    const d = this.node(path); if (!d || !d.children[name] || d.children[nn]) return false;
    const it = d.children[name]; it.name = nn; d.children[nn] = it; delete d.children[name]; this.save(); return true;
  }
};
FS.load();

/* ---------- Мелкие хранилища ---------- */
const KV = {
  p(k){ return 'glower.' + (window.__ns || '') + k; },
  get(k, def){ try{ const v = localStorage.getItem(this.p(k)); return v == null ? def : JSON.parse(v); }catch(e){ return def; } },
  set(k, v){ try{ localStorage.setItem(this.p(k), JSON.stringify(v)); }catch(e){} }
};

/* ---------- Звук (WebAudio, без файлов) ---------- */
const Snd = {
  ctx:null,
  ac(){ if (!this.ctx) { try{ this.ctx = new (window.AudioContext || window.webkitAudioContext)(); }catch(e){} } return this.ctx; },
  blip(freq = 660, dur = 0.07, type = 'sine', gain = 0.05){
    if (!S.sounds) return;
    const c = this.ac(); if (!c) return;
    const o = c.createOscillator(), g = c.createGain();
    o.type = type; o.frequency.value = freq;
    g.gain.setValueAtTime(gain * (S.volume / 100), c.currentTime);
    g.gain.exponentialRampToValueAtTime(0.0001, c.currentTime + dur);
    o.connect(g).connect(c.destination); o.start(); o.stop(c.currentTime + dur);
  },
  open(){ this.blip(720, .09, 'sine', .04); setTimeout(() => this.blip(980, .07, 'sine', .03), 45); },
  close(){ this.blip(420, .08, 'sine', .035); },
  click(){ this.blip(1200, .03, 'square', .015); },
  note(){ if (!S.soundNotif) return; const o = S.sounds; S.sounds = true; this.blip(760, .1, 'sine', .05);
    setTimeout(() => { this.blip(1040, .12, 'sine', .04); S.sounds = o; }, 70); }
};
