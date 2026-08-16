/* ==========================================================================
   Оконный менеджер: перетаскивание, изменение размера, прилипание,
   свёртывание в док, виртуальные рабочие столы
   ========================================================================== */
'use strict';

const WM = {
  wins: [],
  z: 100,
  seq: 0,
  desk: 0,
  desks: 2,
  layer: null,
  ghost: null,

  init(){
    this.layer = $('#windows');
    this.ghost = $('#snap-ghost');
  },

  /* ---------- создание окна ---------- */
  open(appId, opts = {}){
    const app = APPS[appId];
    if (!app) return null;

    if (app.single){
      const ex = this.wins.find(w => w.appId === appId);
      if (ex){ if (ex.minimized) this.restore(ex); this.focus(ex); if (app.onReopen) app.onReopen(ex, opts); return ex; }
    }

    const id = 'w' + (++this.seq);
    const node = el('div', 'win focus');
    node.id = id;
    node.style.zIndex = ++this.z;

    const W = clamp(opts.w || app.w || 820, 320, innerWidth - 40);
    const H = clamp(opts.h || app.h || 560, 200, innerHeight - 120);
    const off = (this.wins.length % 7) * 26;
    const L = opts.x != null ? opts.x : clamp((innerWidth - W) / 2 - 60 + off, 12, innerWidth - W - 12);
    const T = opts.y != null ? opts.y : clamp((innerHeight - H) / 2 - 40 + off, 84, innerHeight - H - 90);
    Object.assign(node.style, { width:W + 'px', height:H + 'px', left:L + 'px', top:T + 'px' });

    node.innerHTML = `
      <div class="titlebar">
        <div class="tl-ico app-ico" style="${icoStyle(app)}">${app.glyph}</div>
        <div class="tl-title">${esc(app.name)}</div>
        <div class="tl-sub"></div>
        <div class="wctl">
          <button data-a="min" title="Свернуть"><svg viewBox="0 0 12 12"><path d="M2 6h8"/></svg></button>
          <button data-a="max" title="Развернуть"><svg viewBox="0 0 12 12"><rect x="2.5" y="2.5" width="7" height="7" rx="1.5"/></svg></button>
          <button data-a="close" title="Закрыть"><svg viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6"/></svg></button>
        </div>
      </div>
      <div class="win-body"></div>
      <div class="rsz n"></div><div class="rsz s"></div><div class="rsz w"></div><div class="rsz e"></div>
      <div class="rsz nw"></div><div class="rsz ne"></div><div class="rsz sw"></div><div class="rsz se"></div>`;

    const win = {
      id, appId, app, node,
      body: $('.win-body', node),
      titleEl: $('.tl-title', node),
      subEl: $('.tl-sub', node),
      minimized:false, maximized:false, desk:this.desk,
      pre:null, data:{},
      setTitle(t){ this.titleEl.textContent = t; },
      setSub(t){ this.subEl.textContent = t || ''; },
      close(){ WM.close(this); }
    };

    this.layer.appendChild(node);
    this.wins.push(win);

    // содержимое приложения
    try { app.render(win, opts); } catch(err){
      win.body.innerHTML = `<div class="pad">Ошибка приложения: ${esc(err.message)}</div>`;
      console.error(err);
    }

    this.bind(win);
    this.focus(win);
    Snd.open();
    Shell.syncDock();
    if (opts.max) this.toggleMax(win);
    return win;
  },

  /* ---------- привязка событий ---------- */
  bind(win){
    const node = win.node, tb = $('.titlebar', node);

    node.addEventListener('mousedown', () => this.focus(win), true);
    $$('.wctl button', node).forEach(b => b.addEventListener('click', e => {
      e.stopPropagation();
      const a = b.dataset.a;
      if (a === 'close') this.close(win);
      else if (a === 'min') this.minimize(win);
      else this.toggleMax(win);
    }));

    tb.addEventListener('dblclick', e => { if (!e.target.closest('.wctl')) this.toggleMax(win); });

    // перетаскивание
    tb.addEventListener('mousedown', e => {
      if (e.target.closest('.wctl') || e.button !== 0) return;
      const r = node.getBoundingClientRect();
      let sx = e.clientX, sy = e.clientY, ox = r.left, oy = r.top, moved = false, zone = null;

      if (win.maximized){ ox = e.clientX - win.pre.w / 2; oy = 6; }

      const move = ev => {
        const dx = ev.clientX - sx, dy = ev.clientY - sy;
        if (!moved && Math.hypot(dx, dy) < 4) return;
        if (!moved){
          moved = true; node.classList.add('dragging');
          if (win.maximized) this.unmax(win, ev);
        }
        node.style.left = (ox + dx) + 'px';
        node.style.top  = clamp(oy + dy, 0, innerHeight - 40) + 'px';
        zone = this.snapZone(ev.clientX, ev.clientY);
        this.showGhost(zone);
      };
      const up = () => {
        document.removeEventListener('mousemove', move);
        document.removeEventListener('mouseup', up);
        node.classList.remove('dragging');
        this.showGhost(null);
        if (zone && S.snapAssist) this.snap(win, zone);
        else this.keepInView(win);
      };
      document.addEventListener('mousemove', move);
      document.addEventListener('mouseup', up);
    });

    // изменение размера
    $$('.rsz', node).forEach(h => h.addEventListener('mousedown', e => {
      e.preventDefault(); e.stopPropagation();
      const dir = h.className.replace('rsz ', '').trim();
      const r = node.getBoundingClientRect();
      const sx = e.clientX, sy = e.clientY;
      if (win.maximized) return;
      const move = ev => {
        const dx = ev.clientX - sx, dy = ev.clientY - sy;
        let w = r.width, hh = r.height, l = r.left, t = r.top;
        if (dir.includes('e')) w = r.width + dx;
        if (dir.includes('s')) hh = r.height + dy;
        if (dir.includes('w')){ w = r.width - dx; l = r.left + dx; }
        if (dir.includes('n')){ hh = r.height - dy; t = r.top + dy; }
        w = clamp(w, 320, innerWidth); hh = clamp(hh, 180, innerHeight);
        Object.assign(node.style, { width:w + 'px', height:hh + 'px', left:l + 'px', top:Math.max(0, t) + 'px' });
        if (win.app.onResize) win.app.onResize(win);
      };
      const up = () => { document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up); };
      document.addEventListener('mousemove', move);
      document.addEventListener('mouseup', up);
    }));

    // блик Liquid Glass на заголовке
    tb.addEventListener('mousemove', e => {
      const r = tb.getBoundingClientRect();
      tb.style.background = `radial-gradient(320px circle at ${e.clientX - r.left}px ${e.clientY - r.top}px,
        rgba(255,255,255,.14), rgba(255,255,255,.03) 60%, transparent),
        linear-gradient(rgba(255,255,255,.06), transparent)`;
    });
    tb.addEventListener('mouseleave', () => { tb.style.background = ''; });
  },

  /* ---------- фокус / порядок ---------- */
  focus(win){
    if (!win || win.minimized) return;
    this.wins.forEach(w => w.node.classList.toggle('focus', w === win));
    win.node.style.zIndex = ++this.z;
    this.active = win;
    Shell.syncDock();
  },
  top(){ return [...this.wins].filter(w => !w.minimized && w.desk === this.desk).sort((a, b) => b.node.style.zIndex - a.node.style.zIndex)[0]; },

  close(win){
    const oc = win.onClose || win.app.onClose;
    if (oc) { try { oc(win); } catch(e){} }
    win.node.classList.add('closing');
    Snd.close();
    setTimeout(() => {
      win.node.remove();
      this.wins = this.wins.filter(w => w !== win);
      const t = this.top(); if (t) this.focus(t);
      Shell.syncDock();
    }, 240 * (S.reduceMotion ? 0.05 : S.speed));
  },

  minimize(win){
    const target = Shell.dockIconRect(win.appId);
    const r = win.node.getBoundingClientRect();
    if (target){
      const sx = target.width / r.width, sy = 0.1;
      const tx = target.left + target.width / 2 - (r.left + r.width / 2);
      const ty = target.top + target.height / 2 - (r.top + r.height / 2);
      win.node.style.transformOrigin = 'center center';
      win.node.style.transform = `translate(${tx}px,${ty}px) scale(${Math.max(sx, .08)},${sy})`;
    } else {
      win.node.style.transform = 'translateY(120px) scale(.85)';
    }
    win.node.classList.add('min');
    win.minimized = true;
    const t = this.top(); if (t) this.focus(t);
    Shell.syncDock();
  },

  restore(win){
    win.node.classList.remove('min');
    win.node.style.transform = '';
    win.minimized = false;
    win.node.classList.add('restoring');
    setTimeout(() => win.node.classList.remove('restoring'), 420);
    this.focus(win);
  },

  toggleMax(win){
    if (win.maximized){
      const p = win.pre;
      win.node.classList.add('snapping');
      Object.assign(win.node.style, { left:p.x + 'px', top:p.y + 'px', width:p.w + 'px', height:p.h + 'px' });
      win.node.classList.remove('max');
      win.maximized = false;
    } else {
      const r = win.node.getBoundingClientRect();
      win.pre = { x:r.left, y:r.top, w:r.width, h:r.height };
      win.node.classList.add('snapping', 'max');
      Object.assign(win.node.style, { left:'0px', top:'0px', width:'100%', height:'100%' });
      win.maximized = true;
    }
    setTimeout(() => { win.node.classList.remove('snapping'); if (win.app.onResize) win.app.onResize(win); }, 460);
  },

  unmax(win, ev){
    const p = win.pre || { w:900, h:600 };
    win.node.classList.remove('max');
    win.maximized = false;
    Object.assign(win.node.style, { width:p.w + 'px', height:p.h + 'px' });
  },

  keepInView(win){
    const r = win.node.getBoundingClientRect();
    if (r.top < 0) win.node.style.top = '0px';
    if (r.left > innerWidth - 80) win.node.style.left = (innerWidth - 80) + 'px';
    if (r.right < 80) win.node.style.left = (80 - r.width) + 'px';
  },

  /* ---------- прилипание ---------- */
  snapZone(x, y){
    const m = 12, T = 8;
    if (y < T) return 'max';
    if (x < T) return y < innerHeight * .3 ? 'tl' : y > innerHeight * .7 ? 'bl' : 'left';
    if (x > innerWidth - T) return y < innerHeight * .3 ? 'tr' : y > innerHeight * .7 ? 'br' : 'right';
    if (y > innerHeight - T) return 'bottom';
    return null;
  },
  zoneRect(z){
    const pad = 8, top = 82, bot = S.dockAutohide ? 14 : (S.dockSize + 42);
    const W = innerWidth - pad * 2, H = innerHeight - top - bot;
    const R = (x, y, w, h) => ({ left:x, top:y, width:w, height:h });
    switch(z){
      case 'max':    return R(0, 0, innerWidth, innerHeight);
      case 'left':   return R(pad, top, W / 2 - 4, H);
      case 'right':  return R(innerWidth / 2 + 4, top, W / 2 - 4, H);
      case 'tl':     return R(pad, top, W / 2 - 4, H / 2 - 4);
      case 'bl':     return R(pad, top + H / 2 + 4, W / 2 - 4, H / 2 - 4);
      case 'tr':     return R(innerWidth / 2 + 4, top, W / 2 - 4, H / 2 - 4);
      case 'br':     return R(innerWidth / 2 + 4, top + H / 2 + 4, W / 2 - 4, H / 2 - 4);
      case 'bottom': return R(pad, top + H / 2 + 4, W, H / 2 - 4);
      default: return null;
    }
  },
  showGhost(z){
    const r = z && this.zoneRect(z);
    if (!r){ this.ghost.classList.remove('on'); return; }
    Object.assign(this.ghost.style, { left:r.left + 'px', top:r.top + 'px', width:r.width + 'px', height:r.height + 'px' });
    this.ghost.classList.add('on');
  },
  snap(win, z){
    const r = this.zoneRect(z); if (!r) return;
    if (z === 'max'){ if (!win.maximized) { win.pre = win.node.getBoundingClientRect().toJSON ? rectObj(win.node) : win.pre; this.toggleMax(win); } return; }
    if (!win.pre) win.pre = rectObj(win.node);
    win.node.classList.add('snapping');
    Object.assign(win.node.style, { left:r.left + 'px', top:r.top + 'px', width:r.width + 'px', height:r.height + 'px' });
    setTimeout(() => { win.node.classList.remove('snapping'); if (win.app.onResize) win.app.onResize(win); }, 460);
  },

  /* ---------- рабочие столы ---------- */
  gotoDesk(i){
    if (i === this.desk) return;
    const dir = i > this.desk ? 1 : -1;
    this.desk = i;
    this.wins.forEach(w => {
      const mine = w.desk === i;
      w.node.style.transition = 'opacity .3s var(--e-io), transform .35s var(--e-out)';
      w.node.style.opacity = mine ? '1' : '0';
      w.node.style.pointerEvents = mine ? '' : 'none';
      if (!mine) w.node.style.transform = `translateX(${-dir * 80}px) scale(.96)`;
      else if (!w.minimized) w.node.style.transform = '';
      setTimeout(() => { w.node.style.transition = ''; }, 400);
    });
    Shell.toast('Рабочий стол ' + (i + 1), 'Переключение', '🖥️');
    Shell.syncDock();
  },

  cascade(){
    let i = 0;
    this.wins.filter(w => !w.minimized).forEach(w => {
      if (w.maximized) this.toggleMax(w);
      w.node.classList.add('snapping');
      Object.assign(w.node.style, { left:(60 + i * 32) + 'px', top:(96 + i * 30) + 'px', width:'760px', height:'520px' });
      setTimeout(() => w.node.classList.remove('snapping'), 460);
      i++;
    });
  },
  tile(){
    const ws = this.wins.filter(w => !w.minimized && w.desk === this.desk);
    if (!ws.length) return;
    const cols = Math.ceil(Math.sqrt(ws.length)), rows = Math.ceil(ws.length / cols);
    const pad = 10, top = 82, bot = S.dockSize + 42;
    const cw = (innerWidth - pad * (cols + 1)) / cols, ch = (innerHeight - top - bot - pad * (rows - 1)) / rows;
    ws.forEach((w, i) => {
      if (w.maximized) { w.node.classList.remove('max'); w.maximized = false; }
      const c = i % cols, r = Math.floor(i / cols);
      w.node.classList.add('snapping');
      Object.assign(w.node.style, {
        left:(pad + c * (cw + pad)) + 'px', top:(top + r * (ch + pad)) + 'px',
        width:cw + 'px', height:ch + 'px' });
      setTimeout(() => { w.node.classList.remove('snapping'); if (w.app.onResize) w.app.onResize(w); }, 460);
    });
  },
  minimizeAll(){ this.wins.filter(w => !w.minimized).forEach(w => this.minimize(w)); }
};

function rectObj(node){ const r = node.getBoundingClientRect(); return { x:r.left, y:r.top, w:r.width, h:r.height }; }
function icoStyle(app){ return `background:${app.bg || 'linear-gradient(140deg,#5b8cff,#8b5cf6)'}`; }
