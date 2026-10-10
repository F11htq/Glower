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
    убрать('#boot'); убрать('#desktop');
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

  const boot = $('#boot'), desktop = $('#desktop');

  /* Рой вокруг вихря — то же движение, что в заставке Plymouth: частицы
     выходят из центра на наклонную орбиту и кружат; позади вихря они
     мельче и тусклее, впереди — крупнее и поверх него. Так одна заставка
     продолжает другую, а не сменяет её. Считаем сами, по кадрам: двадцать
     точек — копейки, а CSS такого объёма без трёхмерных фокусов не даёт.
     На слабой машине и при просьбе о меньшем движении роя нет вовсе. */
  (function рой(){
    const место = document.getElementById('boot-рой');
    if (!место || document.body.classList.contains('эконом') || document.body.classList.contains('reduced')) return;
    const N = 22, точки = [], фазы = [], скорости = [];
    for (let i = 0; i < N; i++){
      const т = document.createElement('i');
      место.appendChild(т); точки.push(т);
      фазы.push(i * (2 * Math.PI / N) + Math.sin(i * 1.7) * 0.25);
      скорости.push(0.55 + (i % 3) * 0.08);
    }
    const плавно = v => { v = Math.max(0, Math.min(1, v)); return 1 - (1 - v) ** 3; };
    const начало = performance.now();
    const кадр = now => {
      if (!место.isConnected) return;
      const t = (now - начало) / 1000;
      const ш = место.clientWidth / 2, в = место.clientHeight / 2;
      const rx = 124, ry = 40, выход = плавно((t - 0.25) / 1.4);
      точки.forEach((т, i) => {
        const a = фазы[i] + t * скорости[i];
        const sx = Math.cos(a), sy = Math.sin(a);
        const x = ш + sx * rx * выход;
        const y = в + (sy * ry - sx * ry * 0.35) * выход;
        const м = 0.55 + 0.45 * (sy + 1) / 2;
        т.style.transform = `translate(${x}px,${y}px) scale(${(0.5 + 0.9 * (sy + 1) / 2).toFixed(3)})`;
        т.style.opacity = (выход * (0.75 + 0.25 * Math.sin(t * 3.1 + i * 1.3)) * м).toFixed(3);
        т.style.zIndex = sy > 0 ? 3 : 1;
      });
      requestAnimationFrame(кадр);
    };
    requestAnimationFrame(кадр);
  })();
  /* Установочная среда: человек пришёл ставить систему. Заставку ему
     показывать незачем — он и так смотрит в экран загрузки уже полминуты,
     и каждая наша секунда добавляется к тем. */
  const установка = /[?&]install=1/.test(location.search);
  const skip = KV.get('skipBoot', false) || установка;

  const showDesktop = () => { desktop.classList.add('on'); };

  /* Показать рабочий стол. Раньше здесь снимался замок оболочки — тот самый
     второй вопрос после настоящего входа. Замка больше нет, и открывать
     стол не мешает ничто. */
  const unlock = () => {
    showDesktop();
    Snd.blip(660, .12, 'sine', .04);
    setTimeout(() => Snd.blip(990, .16, 'sine', .035), 90);
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
    /* Панель наверху — часть той же картины. Пока она не готова, держим
       заставку: показать стол без панели и добавить её через секунду хуже,
       чем подождать эту секунду за заставкой. Ждём не дольше срока. */
    /* Панели в установочной среде нет вовсе — ждать нечего. */
    if (!установка && window.Поверхности && Поверхности.ждиПанель)
      await Поверхности.ждиПанель(2500);
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
      unlock();
      return;
    }
    /* Мастер настройки, приветствие и вход — дело рабочего стола. Панель
       открывает ту же страницу второй раз, и если не остановить её здесь,
       она запустит у себя второй такой же мастер. Видно это было так: стол
       уже работал, а в полосе наверху так и стоял сплющенный мастер —
       своя, отдельная его копия, которую никто не проходил. */
    const своя = !window.Поверхности || Поверхности.стол();

    if (своя && window.Setup && Setup.needed()){
      setTimeout(() => Setup.open(), 350);
      return;
    }
    /* смена языка в настройке перезагружает страницу — здесь досматриваем приветствие */
    if (своя && window.Welcome && KV.get('welcome.pending', false)){
      KV.set('welcome.pending', false);
      setTimeout(() => Welcome.play(), 300);
      return;
    }

    /* Дальше рабочий стол открывается сам.
       Замок здесь остался от тех времён, когда оболочка была всей системой
       и сама решала, кого пускать. Теперь у системы есть настоящий вход:
       пароль спрашивает служба входа до всякого рабочего стола, и спрашивает
       его у самой системы. Второй замок сразу после первого — дверь, за
       которой стоит ещё одна дверь: человек уже назвал себя, а его снова
       просят нажать «Войти» неизвестно зачем.

       Сам замок никуда не делся — он приходит по Super+L и из меню питания,
       когда человек отходит от машины. Просто он больше не встречает на
       пороге. */
    unlock();
  }, skip ? 200 : 1900);

  window.__unlock = unlock;

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
