/* ==========================================================================
   Запуск системы
   ========================================================================== */
'use strict';

(function boot(){
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

  const showDesktop = () => {
    desktop.classList.add('on');
    setTimeout(() => {
      Shell.toast('Добро пожаловать', 'Win — Пуск · Win+Space — поиск · ПКМ по столу — меню', '👋', 7000);
    }, 900);
    setTimeout(() => {
      if (!KV.get('seenTip', false)){
        KV.set('seenTip', true);
        Shell.toast('Совет', 'Параметры → Liquid Glass: там живут все настройки материала', '🫧', 8000);
      }
    }, 5200);
  };

  const unlock = () => {
    if (!lock.classList.contains('gone')){
      Shell.lock(false);
      Snd.blip(660, .12, 'sine', .04);
      setTimeout(() => Snd.blip(990, .16, 'sine', .035), 90);
      showDesktop();
    }
  };

  setTimeout(() => {
    boot.classList.add('gone');
    document.body.classList.add('blurred');
    setTimeout(() => boot.remove(), 700);
  }, skip ? 200 : 1900);

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
