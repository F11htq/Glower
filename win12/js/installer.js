/* ==========================================================================
   Установка GlowerOS на диск

   Живая система всё забывает при перезагрузке. Здесь — мастер, который
   переносит её на настоящий диск: выбор диска, подтверждение, ход работы.
   Появляется только там, где установка вообще возможна: в живой системе,
   при запущенном агенте с ключом --allow-install.

   Всё опасное делает не оболочка, а системный сценарий; оболочка лишь
   показывает его отчёт и не даёт выбрать носитель, с которого загружены.
   ========================================================================== */
'use strict';

const Install = {
  async can(){
    if (!(window.OS && OS.on())) return { allowed:false, reason:'система не управляет машиной' };
    try { return await Platform.rpc('install.can'); }
    catch(e){ return { allowed:false, reason:String(e.message || e) }; }
  },
  disks(){ return Platform.rpc('install.disks'); },
  plan(disk){ return Platform.rpc('install.plan', { disk }); },
  start(o){ return Platform.rpc('install.start', o); },
  state(){ return Platform.rpc('install.state'); },
  size(b){
    const g = b / 1000 / 1000 / 1000;
    return g >= 1000 ? (g / 1000).toFixed(1) + ' ТБ' : Math.round(g) + ' ГБ';
  }
};
window.Install = Install;

/* Приложение появляется в системе только тогда, когда установка возможна:
   в прототипе и на уже установленной системе его в Пуске быть не должно. */
const INSTALLER_APP = {
  name:'Установка ' + (window.Brand ? Brand.name : 'системы'),
  glyph:'💽', bg:'linear-gradient(140deg,#a78bfa,#4c1d95)', w:720, h:560, single:true,

  render(win){
    const wrap = el('div', 'app col ins'); win.body.appendChild(wrap);
    const body = el('div', 'ins-body');
    const foot = el('div', 'ins-foot');
    const back = el('button', 'btn', 'Назад');
    const next = el('button', 'btn pri', 'Далее');
    const spacer = el('div', 'grow');
    foot.append(back, spacer, next);
    wrap.append(body, foot);

    const S2 = { step:0, disk:null, disks:[], pass:'', host:'GlowerOS', can:null, busy:false };

    const draw = async () => {
      body.innerHTML = '';
      back.style.visibility = S2.step > 0 && S2.step < 4 ? '' : 'hidden';
      next.disabled = false;
      next.textContent = 'Далее';

      /* --- 0. приветствие --- */
      if (S2.step === 0){
        S2.can = S2.can || await Install.can();
        body.innerHTML = `
          <img class="os-logo ins-logo" src="assets/logo.png" alt="" draggable="false">
          <h1>Установка ${esc(Brand.name)}</h1>
          <p class="ins-lead">Сейчас система работает из памяти и всё забывает при выключении.
            Установка перенесёт её на диск: настройки, файлы и учётная запись начнут сохраняться.</p>`;
        if (!S2.can.allowed || S2.can.reason){
          body.appendChild(el('div', 'ins-warn',
            'Установка недоступна: ' + esc(S2.can.reason || 'причина неизвестна')));
          next.disabled = true;
        } else {
          body.appendChild(el('div', 'ins-note',
            'Диск, который вы выберете, будет очищен полностью. Всё, что на нём есть, исчезнет.'));
        }
        win.setSub('Шаг 1 из 4');
        return;
      }

      /* --- 1. диск --- */
      if (S2.step === 1){
        body.innerHTML = '<h2>Куда установить</h2>';
        const list = el('div', 'ins-disks');
        body.appendChild(list);
        list.appendChild(el('div', 'muted tiny', 'Ищу диски…'));
        win.setSub('Шаг 2 из 4');
        next.disabled = true;
        try {
          const d = await Install.disks();
          S2.disks = d.list;
          list.innerHTML = '';
          if (!d.list.length) list.appendChild(el('div', 'ins-warn', 'Дисков не найдено'));
          d.list.forEach(x => {
            const b = el('button', 'ins-disk' + (x.live ? ' off' : '') + (S2.disk === x.dev ? ' on' : ''));
            b.innerHTML = `<span class="e">${x.removable ? '🔌' : '💽'}</span>
              <span class="l"><b>${esc(x.model || x.name)}</b>
                <small>${esc(x.dev)} · ${Install.size(x.size)} · ${esc(x.kind)}${x.live ? ' · носитель системы' : ''}</small></span>`;
            if (x.live){ b.disabled = true; }
            else b.onclick = () => { S2.disk = x.dev; draw(); };
            list.appendChild(b);
          });
          next.disabled = !S2.disk;
        } catch(e){
          list.innerHTML = '';
          list.appendChild(el('div', 'ins-warn', esc(String(e.message || e))));
        }
        return;
      }

      /* --- 2. учётная запись --- */
      if (S2.step === 2){
        body.innerHTML = '<h2>Учётная запись машины</h2>';
        const c = el('div', 'ins-form');
        const p1 = el('input', 'inp'); p1.type = 'password'; p1.placeholder = 'Пароль (необязательно)'; p1.value = S2.pass;
        const hn = el('input', 'inp'); hn.value = S2.host; hn.placeholder = 'Имя машины';
        p1.oninput = () => S2.pass = p1.value;
        hn.oninput = () => S2.host = hn.value;
        c.append(row('🔑', 'Пароль системной записи', 'Нужен для входа в консоль и для sudo. Можно оставить пустым, как в живой системе.', p1),
                 row('🏷', 'Имя машины', 'Так система будет представляться в сети', hn));
        body.appendChild(c);
        body.appendChild(el('div', 'ins-note',
          'Имя пользователя и пароль самой оболочки вы зададите при первом запуске установленной системы.'));
        win.setSub('Шаг 3 из 4');
        return;
      }

      /* --- 3. подтверждение --- */
      if (S2.step === 3){
        const d = S2.disks.find(x => x.dev === S2.disk) || {};
        body.innerHTML = `<h2>Проверьте перед началом</h2>
          <div class="ins-sum">
            <div class="r"><span>Диск</span><b>${esc(d.model || d.name || S2.disk)}</b></div>
            <div class="r"><span>Устройство</span><b>${esc(S2.disk)}</b></div>
            <div class="r"><span>Объём</span><b>${Install.size(d.size || 0)}</b></div>
            <div class="r"><span>Имя машины</span><b>${esc(S2.host)}</b></div>
            <div class="r"><span>Пароль</span><b>${S2.pass ? 'задан' : 'без пароля'}</b></div>
          </div>`;
        body.appendChild(el('div', 'ins-warn',
          'Диск ' + esc(S2.disk) + ' будет стёрт полностью, вместе со всеми разделами и данными. Отменить это будет нечем.'));
        next.textContent = 'Стереть диск и установить';
        win.setSub('Шаг 4 из 4');
        return;
      }

      /* --- 4. ход установки --- */
      if (S2.step === 4){
        body.innerHTML = `<h2>Устанавливаю</h2>
          <div class="ins-bar"><i style="width:0%"></i></div>
          <div class="ins-step muted">Начинаю…</div>
          <details class="ins-log"><summary>Подробности</summary><pre></pre></details>`;
        back.style.visibility = 'hidden';
        next.disabled = true;
        win.setSub('Установка идёт');
        const bar = $('.ins-bar i', body), stepEl = $('.ins-step', body), pre = $('.ins-log pre', body);

        try { await Install.start({ disk:S2.disk, password:S2.pass, hostname:S2.host, tz:S.tz || undefined }); }
        catch(e){ return fail(e); }

        const tick = async () => {
          let s;
          try { s = await Install.state(); } catch(e){ return fail(e); }
          bar.style.width = (s.percent || 0) + '%';
          stepEl.textContent = s.step || '';
          pre.textContent = s.log || '';
          if (s.running) return setTimeout(tick, 700);
          if (s.ok) return done();
          fail(new Error(s.error || 'установка прервалась'));
        };
        tick();
        return;
      }

      /* --- 5. готово --- */
      if (S2.step === 5){
        body.innerHTML = `
          <img class="os-logo ins-logo" src="assets/logo.png" alt="" draggable="false">
          <h1>Система установлена</h1>
          <p class="ins-lead">${esc(Brand.name)} перенесена на ${esc(S2.disk)}.
            Выньте установочный носитель и перезагрузите машину — дальше она будет грузиться с диска
            и запомнит всё, что вы настроите.</p>`;
        back.style.visibility = 'hidden';
        next.textContent = 'Перезагрузить';
        win.setSub('Готово');
        return;
      }
    };

    const fail = e => {
      body.innerHTML = '<h2>Установка не завершилась</h2>';
      body.appendChild(el('div', 'ins-warn', esc(String(e.message || e))));
      body.appendChild(el('div', 'ins-note',
        'Диск мог остаться размеченным наполовину. Можно попробовать снова — установка начинается с полной очистки диска.'));
      back.style.visibility = '';
      next.disabled = false;
      next.textContent = 'Начать заново';
      S2.step = 6;
    };

    const done = () => { S2.step = 5; draw(); };

    back.onclick = () => { S2.step = Math.max(0, S2.step - 1); draw(); };
    next.onclick = async () => {
      if (S2.step === 6){ S2.step = 1; return draw(); }
      if (S2.step === 5){ return Shell.power('restart'); }
      if (S2.step === 3){
        if (!await Dlg.confirm('Стереть диск ' + S2.disk + '?',
            'Все разделы и данные на этом диске исчезнут. Это необратимо.',
            { icon:'💽', okText:'Стереть и установить', danger:true })) return;
      }
      S2.step++;
      draw();
    };

    draw();
  }
};

/* ---------- показываем мастер только там, где он имеет смысл ---------- */
(async function offer(){
  for (let i = 0; i < 40 && !(window.OS && OS.on()); i++)
    await new Promise(r => setTimeout(r, 250));
  if (!(window.OS && OS.on())) return;
  const can = await Install.can();
  if (!can.allowed || can.reason) return;

  APPS.installer = INSTALLER_APP;
  if (window.Shell && Shell.renderShell) Shell.renderShell();

  /* Значок на рабочем столе живой системы — как у всех живых систем:
     установка не должна прятаться в глубине меню. */
  addDesktopIcon();

  /* Пункт меню загрузки «Установить GlowerOS» просит открыть мастер сразу */
  if (/[?&]install=1/.test(location.search)){
    setTimeout(() => { try { WM.open('installer'); } catch(e){} }, 1200);
    return;
  }

  Shell.toast('Установка', 'Систему можно перенести на диск — откройте «Установка ' + Brand.name + '»', '💽', 9000);
})();


/* ---------- значок «Установить» на рабочем столе ---------- */
function addDesktopIcon(){
  const d = document.getElementById('desktop-icons') || document.querySelector('.desk-icons');
  if (!d || d.querySelector('.ins-desk')) return;
  const i = el('div', 'desk-icon ins-desk');
  i.innerHTML = `<div class="ico" style="background:linear-gradient(140deg,#a78bfa,#4c1d95)">💽</div>
    <div class="nm">Установить ${esc(Brand.name)}</div>`;
  i.ondblclick = () => WM.open('installer');
  i.onclick = e => { if (e.detail === 1) i.classList.add('sel'); };
  d.appendChild(i);
}
