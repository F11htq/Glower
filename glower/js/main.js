/* ==========================================================================
   Запуск системы
   ========================================================================== */
'use strict';

(function boot(){
  /* Экран входа — не рабочий стол. Здесь нечего запускать: ни оконного
     хозяйства, ни своего замка. Раньше оболочка всё равно проходила всю
     загрузку и её собственный замок забирал себе клавиатуру — в поле
     пароля на экране входа нельзя было напечатать ни буквы. */
  if (/[?&]login=1/.test(location.search)){
    const убрать = с => { const н = document.querySelector(с); if (н) н.remove(); };
    убрать('#boot'); убрать('#lock'); убрать('#desktop');
    document.body.classList.remove('locked');
    return;
  }

  applySettings();
  document.documentElement.style.fontSize = (16 * KV.get('zoom', 1)) + 'px';
  WM.init();
  Shell.init();

  // учёт недавних приложений
  const openOrig = WM.open.bind(WM);
  WM.open = (id, opts) => {
    const r = openOrig(id, opts);
    if (r){
      const rec = KV.get('recent', []).filter(x => x !== id);
      rec.unshift(id); KV.set('recent', rec.slice(0, 8));
      Shell.renderStart();
    }
    return r;
  };

  const boot = $('#boot'), lock = $('#lock'), desktop = $('#desktop');
  const skip = KV.get('skipBoot', false);

  const showDesktop = () => { desktop.classList.add('on'); };

  const unlock = () => {
    if (window.Profiles && !Profiles.authorized()){ Profiles.focusPassword(); return; }
    if (!lock.classList.contains('gone')){
      Shell.lock(false);
      Snd.blip(660, .12, 'sine', .04);
      setTimeout(() => Snd.blip(990, .16, 'sine', .035), 90);
      showDesktop();
    }
  };

  /* Решение о первом запуске принимаем, только когда все скрипты выполнены.
     Раньше оно висело на таймере в 1,9 с: на быстрой машине этого хватало,
     а на медленной — например, в виртуалке без аппаратного ускорения —
     setup.js ещё не успевал выполниться, и система молча уходила на экран
     блокировки, пропустив настройку. Время загрузки машины теперь ни на
     что не влияет. */
  const scriptsReady = document.readyState === 'complete'
    ? Promise.resolve()
    : new Promise(r => addEventListener('load', r, { once:true }));

  setTimeout(async () => {
    boot.classList.add('gone');
    document.body.classList.add('blurred');
    setTimeout(() => boot.remove(), 700);

    await scriptsReady;

    /* первый запуск: сначала настройка, рабочий стол — после неё */
    /* Установочная среда: человек пришёл ставить систему, а не пользоваться
       временной. Ни входа, ни настройки, ни рабочего стола — только мастер,
       как в любой системе при первой установке. */
    if (/[?&]install=1/.test(location.search)){
      document.body.classList.add('setup-env');
      if (window.Profiles) Profiles.ok = true;
      unlock();
      return;
    }
    if (window.Setup && Setup.needed()){
      lock.classList.add('gone');
      document.body.classList.remove('locked');
      setTimeout(() => Setup.open(), 350);
      return;
    }
    /* смена языка в настройке перезагружает страницу — здесь досматриваем приветствие */
    if (window.Welcome && KV.get('welcome.pending', false)){
      KV.set('welcome.pending', false);
      lock.classList.add('gone');
      document.body.classList.remove('locked');
      setTimeout(() => Welcome.play(), 300);
    }
  }, skip ? 200 : 1900);

  window.__unlock = unlock;
  lock.addEventListener('click', unlock);
  $('#lock-enter').addEventListener('click', e => { e.stopPropagation(); unlock(); });
  addEventListener('keydown', function once(e){
    if (lock.classList.contains('gone')) return;
    if (['Enter',' ','Escape'].includes(e.key)) unlock();
  });

  // адаптация окон при изменении размера экрана
  let rt = null;
  addEventListener('resize', () => {
    clearTimeout(rt);
    rt = setTimeout(() => {
      WM.wins.forEach(w => {
        const r = w.node.getBoundingClientRect();
        if (r.left > innerWidth - 100) w.node.style.left = (innerWidth - 200) + 'px';
        if (r.top > innerHeight - 60) w.node.style.top = (innerHeight - 200) + 'px';
        if (w.app.onResize) w.app.onResize(w);
      });
    }, 150);
  });

  // страховка: разблокировка первым кликом по документу
  addEventListener('mousedown', () => { Snd.ac(); }, { once:true });
})();
