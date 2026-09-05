/* ==========================================================================
   Учётные записи: несколько профилей, вход по паролю,
   у каждого свои файлы, обои и настройки
   ========================================================================== */
'use strict';

const Profiles = {
  KEY:'glower.profiles',
  CUR:'glower.profile',

  list(){ try { return JSON.parse(localStorage.getItem(this.KEY)) || []; } catch(e){ return []; } },
  save(l){ try { localStorage.setItem(this.KEY, JSON.stringify(l)); } catch(e){} },
  current(){ return this.list().find(p => p.id === window.__profile) || this.list()[0]; },

  /* ---------- пароль ---------- */
  async hash(id, pw){
    const data = new TextEncoder().encode('glower:' + id + ':' + pw);
    if (crypto.subtle){
      const b = await crypto.subtle.digest('SHA-256', data);
      return [...new Uint8Array(b)].map(x => x.toString(16).padStart(2, '0')).join('');
    }
    let h = 5381;                                  // запасной вариант без crypto.subtle
    for (const c of data) h = ((h << 5) + h + c) >>> 0;
    return 'w' + h.toString(16);
  },
  /* Проверка пароля.

     На машине пароль проверяет сама система — той же программой, что
     спрашивает его при входе в консоль. Свой отпечаток в памяти оболочки
     для этого не годится: он живёт в браузере, и защита из него никакая. */
  async verify(id, pw){
    const p = this.list().find(x => x.id === id);

    if (window.Platform && Platform.mode === 'native'){
      try {
        const о = await Platform.rpc('sys.auth', { 'пароль':pw });
        if (о && о.ok) return true;
        this.последняяБеда = (о && о['почему']) || 'пароль не подошёл';
        if (о && о['ждать']) this.последняяБеда += ' (' + о['ждать'] + ' с)';
        return false;
      } catch(e){
        this.последняяБеда = 'система не ответила: ' + (e.message || e);
        return false;
      }
    }

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

  /* Узнаём у системы, задан ли пароль у нашего пользователя */
  async узнайПроПароль(){
    if (!window.Platform || Platform.mode !== 'native') return;
    try {
      const я = await Platform.rpc('sys.me');
      this.системныйПароль = я && я['пароль'] === 'задан';   // показываем в Параметрах
      this.системноеИмя = (я && я['имя']) || '';
    } catch(e){}
  },

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
    Object.keys(localStorage).filter(k => k.startsWith('glower.' + nsp) && nsp)
      .forEach(k => localStorage.removeItem(k));
    return true;
  },
  /* Пароль профиля — не пароль машины. Машину проверяет система (sys.auth),
     а здесь мы сверяем отпечаток того профиля, в который переходим: иначе
     «Войти как» пускало бы в чужие данные одним щелчком. Защита эта ровно
     той же силы, что и раньше — от чужого взгляда за общим столом, не от
     того, у кого есть корень. Настоящий замок машины — системный. */
  async проверьПрофиль(id, pw){
    const p = this.list().find(x => x.id === id);
    if (!p || !p.hash) return true;
    return (await this.hash(id, pw)) === p.hash;
  },

  async switchTo(id){
    const p = this.list().find(x => x.id === id);
    if (p && p.hash){
      for (;;){
        const pw = await Dlg.prompt('Войти как ' + p.name,
          'Профиль защищён паролем.', '', p.emoji || '👤',
          { password:true, okText:'Войти' });
        if (pw === null) return false;              // передумали
        if (await this.проверьПрофиль(id, pw)) break;
        await Dlg.alert('Пароль не подошёл',
          'Проверьте раскладку и регистр — профиль остался прежним.', '⚠️');
      }
    }
    localStorage.setItem(this.CUR, id);
    location.reload();
    return true;
  },

  /* Экран блокировки оболочки убран целиком. Он был картинкой внутри
     страницы: закрывал её собой и спрашивал пароль у самой оболочки.
     Защищал он ровно ничего — под ним продолжали работать чужие окна, а
     уйти из-под него можно было переключением консоли. Настоящей
     блокировкой занимается система: Shell.lock() зовёт её. */
};
window.Profiles = Profiles;

/* ---------- интеграция с оболочкой ---------- */
(function wire(){

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
      { i:'🔒', t:'Заблокировать', f:() => Shell.lock(), k:'Win+L' },
      ...(others.length ? ['hr'] : []),
      ...others.map(p => ({ i:p.emoji || '👤', t:'Войти как ' + p.name, f:() => Profiles.switchTo(p.id) }))
    ]);
  };

  // питание: смена пользователя
  const box = $('#power-overlay .power-actions');
  const sw = el('button', '', `<svg viewBox="0 0 24 24" class="ic"><circle cx="9" cy="9" r="3.3"/><path d="M3.5 19a5.5 5.5 0 0 1 11 0"/><path d="M16 6.2a3.2 3.2 0 0 1 0 5.9"/><path d="M17.5 14.4a5.5 5.5 0 0 1 3 4.6"/></svg><span>Сменить пользователя</span>`);
  /* «Сменить пользователя» — это именно смена профиля оболочки, а не замок:
     запереть экран можно кнопкой рядом. Если профиль всего один, менять
     не на что, и честнее сказать это, чем молча ничего не сделать. */
  sw.onclick = () => {
    $('#power-overlay').classList.remove('on');
    const others = Profiles.list().filter(p => p.id !== window.__profile);
    if (!others.length)
      return Shell.toast('Пользователи',
        'На этой машине пока один профиль. Добавить второй можно в Параметрах', '👥');
    const r = sw.getBoundingClientRect();
    Shell.ctx(r.left, Math.max(80, r.top - 40 - others.length * 34),
      others.map(p => ({ i:p.emoji || '👤', t:'Войти как ' + p.name,
                         f:() => Profiles.switchTo(p.id) })));
  };
  box.insertBefore(sw, box.firstChild);

  // Win+L
  addEventListener('keydown', e => {
    if ((e.metaKey || (e.ctrlKey && e.altKey)) && e.key.toLowerCase() === 'l'){
      e.preventDefault(); Shell.lock();
    }
  });

  // блокировка из меню питания использует профильный экран
  /* Блокировкой занимается сама оболочка через систему — перехватывать
     здесь больше нечего. */
})();
