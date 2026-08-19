/* ==========================================================================
   Учётные записи: несколько профилей, вход по паролю,
   у каждого свои файлы, обои и настройки
   ========================================================================== */
'use strict';

const Profiles = {
  KEY:'win12.profiles',
  CUR:'win12.profile',
  ok:false,                       // пройден ли вход в текущей сессии

  list(){ try { return JSON.parse(localStorage.getItem(this.KEY)) || []; } catch(e){ return []; } },
  save(l){ try { localStorage.setItem(this.KEY, JSON.stringify(l)); } catch(e){} },
  current(){ return this.list().find(p => p.id === window.__profile) || this.list()[0]; },

  /* ---------- пароль ---------- */
  async hash(id, pw){
    const data = new TextEncoder().encode('win12:' + id + ':' + pw);
    if (crypto.subtle){
      const b = await crypto.subtle.digest('SHA-256', data);
      return [...new Uint8Array(b)].map(x => x.toString(16).padStart(2, '0')).join('');
    }
    let h = 5381;                                  // запасной вариант без crypto.subtle
    for (const c of data) h = ((h << 5) + h + c) >>> 0;
    return 'w' + h.toString(16);
  },
  async verify(id, pw){
    const p = this.list().find(x => x.id === id);
    if (!p || !p.hash) return true;
    return (await this.hash(id, pw)) === p.hash;
  },
  async setPassword(id, pw){
    const l = this.list(), p = l.find(x => x.id === id);
    if (!p) return false;
    p.hash = pw ? await this.hash(id, pw) : null;
    this.save(l);
    return true;
  },

  authorized(){ return this.ok || !(this.current() || {}).hash; },

  /* ---------- профили ---------- */
  async add(name, emoji, pw){
    const id = 'u' + Date.now().toString(36);
    const l = this.list();
    l.push({ id, name:name || 'Пользователь', emoji:emoji || (name || 'U')[0].toUpperCase(),
             hash:pw ? await this.hash(id, pw) : null, created:Date.now() });
    this.save(l);
    return id;
  },
  remove(id){
    const l = this.list();
    if (l.length < 2) return false;
    this.save(l.filter(p => p.id !== id));
    const nsp = id === 'default' ? '' : id + '.';
    Object.keys(localStorage).filter(k => k.startsWith('win12.' + nsp) && nsp)
      .forEach(k => localStorage.removeItem(k));
    return true;
  },
  switchTo(id){
    localStorage.setItem(this.CUR, id);
    location.reload();
  },

  /* ---------- экран блокировки ---------- */
  buildLock(){
    const lock = $('#lock'), bottom = $('.lock-bottom', lock);
    const p = this.current();
    bottom.innerHTML = '';
    if (this.renderChip) this.renderChip();   // имя в Пуске идёт за именем профиля

    const ava = el('div', 'lock-avatar', esc(p.emoji || p.name[0]));
    const name = el('div', 'lock-name', esc(p.name));
    bottom.append(ava, name);

    if (p.hash){
      const box = el('div', 'lock-pw');
      const inp = el('input'); inp.type = 'password'; inp.placeholder = 'Пароль'; inp.autocomplete = 'off';
      const go = el('button', '', '→');
      box.append(inp, go);
      const err = el('div', 'lock-err', '');
      const submit = async () => {
        if (await this.verify(p.id, inp.value)){
          this.ok = true; err.textContent = '';
          window.__unlock && window.__unlock();
        } else {
          err.textContent = 'Неверный пароль';
          box.classList.remove('shake'); void box.offsetWidth; box.classList.add('shake');
          inp.select();
        }
      };
      go.onclick = e => { e.stopPropagation(); submit(); };
      inp.onkeydown = e => { e.stopPropagation(); if (e.key === 'Enter') submit(); };
      inp.onclick = e => e.stopPropagation();
      bottom.append(box, err);
      this._pw = inp;
      setTimeout(() => inp.focus(), 600);
    } else {
      const b = el('button', 'lock-btn glass', 'Войти');
      b.onclick = e => { e.stopPropagation(); this.ok = true; window.__unlock && window.__unlock(); };
      bottom.append(b, el('div', 'lock-hint', 'нажмите куда угодно или Enter'));
    }

    const others = this.list().filter(x => x.id !== p.id);
    if (others.length){
      const sw = el('div', 'lock-users');
      others.forEach(o => {
        const b = el('button', 'lock-user', `<span class="ava">${esc(o.emoji || o.name[0])}</span>${esc(o.name)}${o.hash ? ' 🔒' : ''}`);
        b.onclick = e => { e.stopPropagation(); this.switchTo(o.id); };
        sw.appendChild(b);
      });
      bottom.append(el('div', 'lock-sw-t', 'Другие пользователи'), sw);
    }
  },
  focusPassword(){ if (this._pw) this._pw.focus(); },

  /* блокировка экрана без выхода из профиля */
  lock(){
    this.ok = false;
    this.buildLock();
    Shell.lock(true);
  }
};
window.Profiles = Profiles;

/* ---------- интеграция с оболочкой ---------- */
(function wire(){
  Profiles.buildLock();

  // меню пользователя в Пуске
  const chip = $('#start-user');
  /* имя рисуем отдельной функцией: после мастера первого запуска и смены
     профиля кнопку нужно перерисовать, иначе в Пуске остаётся старое имя */
  Profiles.renderChip = () => {
    const c = $('#start-user');
    if (!c) return;
    const me = Profiles.current() || {};
    c.innerHTML = `<span class="ava">${esc(me.emoji || (S.userName || '?')[0])}</span> ${esc(me.name || S.userName)}`;
  };
  Profiles.renderChip();
  chip.onclick = e => {
    e.stopPropagation();
    const others = Profiles.list().filter(p => p.id !== window.__profile);
    Shell.ctx(e.clientX, e.clientY - 180, [
      { i:'⚙️', t:'Параметры учётной записи', f:() => { WM.open('settings', { section:'acc' }); Shell.closePanels(); } },
      { i:'🔒', t:'Заблокировать', f:() => { Shell.closePanels(); Profiles.lock(); }, k:'Win+L' },
      ...(others.length ? ['hr'] : []),
      ...others.map(p => ({ i:p.emoji || '👤', t:'Войти как ' + p.name, f:() => Profiles.switchTo(p.id) }))
    ]);
  };

  // питание: смена пользователя
  const box = $('#power-overlay .power-actions');
  const sw = el('button', '', `<svg viewBox="0 0 24 24" class="ic"><circle cx="9" cy="9" r="3.3"/><path d="M3.5 19a5.5 5.5 0 0 1 11 0"/><path d="M16 6.2a3.2 3.2 0 0 1 0 5.9"/><path d="M17.5 14.4a5.5 5.5 0 0 1 3 4.6"/></svg><span>Сменить пользователя</span>`);
  sw.onclick = () => {
    $('#power-overlay').classList.remove('on');
    Profiles.ok = false; Profiles.buildLock(); Shell.lock(true);
  };
  box.insertBefore(sw, box.firstChild);

  // Win+L
  addEventListener('keydown', e => {
    if ((e.metaKey || (e.ctrlKey && e.altKey)) && e.key.toLowerCase() === 'l'){
      e.preventDefault(); Shell.closePanels(); Profiles.lock();
    }
  });

  // блокировка из меню питания использует профильный экран
  const powerOrig = Shell.power.bind(Shell);
  Shell.power = act => { if (act === 'lock'){ $('#power-overlay').classList.remove('on'); return Profiles.lock(); }
    return powerOrig(act); };
})();
