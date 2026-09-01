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
  repairing(){ return /[?&]repair=1/.test(location.search); },
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

    const S2 = { step:0, disk:null, disks:[], pass:'', host:'GlowerOS', can:null, busy:false,
                 crypt:false, cryptPass:'',
                 repair:Install.repairing() };

    const draw = async () => {
      body.innerHTML = '';
      back.style.visibility = S2.step > 0 && S2.step < 4 ? '' : 'hidden';
      next.disabled = false;
      next.textContent = 'Далее';

      /* --- 0. приветствие --- */
      if (S2.step === 0){
        S2.can = S2.can || await Install.can();
        const чиним = S2.repair;
        body.innerHTML = `
          <img class="os-logo ins-logo" src="assets/logo.png" alt="" draggable="false"
               onerror="this.remove()">
          <h1>${чиним ? 'Восстановление ' + esc(Brand.name) : 'Установка ' + esc(Brand.name)}</h1>
          <p class="ins-lead">${чиним
            ? 'Системные файлы будут положены заново поверх установленной системы. Личные файлы и папка пользователя останутся на месте.'
            : 'Сейчас система работает из памяти и всё забывает при выключении. Установка перенесёт её на диск: настройки, файлы и учётная запись начнут сохраняться.'}</p>`;
        if (!S2.can.allowed || S2.can.reason){
          body.appendChild(el('div', 'ins-warn',
            'Установка недоступна: ' + esc(S2.can.reason || 'причина неизвестна')));
          next.disabled = true;
        } else {
          body.appendChild(el('div', 'ins-note', чиним
            ? 'Разметка диска не меняется. Заменяются только файлы самой системы.'
            : 'Диск, который вы выберете, будет очищен полностью. Всё, что на нём есть, исчезнет.'));
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

        /* Шифрование диска. Пароль входа защищает систему от того, кто сядет
           за включённую машину. Шифрование — от того, кто унесёт её с собой:
           без него диск читается на любом другом компьютере, и никакой
           пароль входа этому не мешает. */
        const шифр = el('input'); шифр.type = 'checkbox'; шифр.checked = S2.crypt;
        const p2 = el('input', 'inp'); p2.type = 'password';
        p2.placeholder = 'Пароль диска (не короче 8 знаков)'; p2.value = S2.cryptPass;
        const строкаШифра = row('🔒', 'Зашифровать диск',
          'Система будет спрашивать пароль при включении. Без него файлы не прочитать даже вынув диск.', шифр);
        const строкаПароля = row('🔑', 'Пароль диска',
          'Отдельный от пароля записи. Забыть его нельзя: восстановить данные будет нечем.', p2);
        строкаПароля.style.display = S2.crypt ? '' : 'none';
        шифр.onchange = () => {
          S2.crypt = шифр.checked;
          строкаПароля.style.display = S2.crypt ? '' : 'none';
          проверь();
        };
        p2.oninput = () => { S2.cryptPass = p2.value; проверь(); };
        const беда = el('div', 'ins-warn'); беда.style.display = 'none';
        const проверь = () => {
          /* Пароль диска набирается до загрузки системы: там нет ни раскладок,
             ни переключения языка. Русские буквы в нём — запертый сундук без
             ключа, поэтому не даём их ввести молча. */
          const нелатиница = /[^\x20-\x7e]/.test(S2.cryptPass);
          беда.textContent = нелатиница
            ? 'Пароль диска можно набрать только латиницей: при включении машины раскладки ещё нет.'
            : '';
          беда.style.display = нелатиница ? '' : 'none';
          next.disabled = S2.crypt && (S2.cryptPass.length < 8 || нелатиница);
        };
        c.append(строкаШифра, строкаПароля, беда);

        body.appendChild(c);
        body.appendChild(el('div', 'ins-note',
          'Имя пользователя и пароль самой оболочки вы зададите при первом запуске установленной системы.'));
        проверь();
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
            <div class="r"><span>Шифрование</span><b>${S2.crypt ? 'диск будет зашифрован' : 'нет'}</b></div>
          </div>`;
        body.appendChild(el('div', S2.repair ? 'ins-note' : 'ins-warn', S2.repair
          ? 'Разметка и личные файлы не трогаются. Заменяются файлы системы: ' + esc(S2.disk) + '.'
          : 'Диск ' + esc(S2.disk) + ' будет стёрт полностью, вместе со всеми разделами и данными. Отменить это будет нечем.'));
        next.textContent = S2.repair ? 'Восстановить систему' : 'Стереть диск и установить';
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

        try { await Install.start({ disk:S2.disk, password:S2.pass, hostname:S2.host,
                                    tz:S.tz || undefined, repair:S2.repair,
                                    crypt:S2.crypt || undefined,
                                    cryptPassword:S2.crypt ? S2.cryptPass : undefined }); }
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
          <h1>${S2.repair ? 'Система восстановлена' : 'Система установлена'}</h1>
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
        const ok = S2.repair
          ? await Dlg.confirm('Восстановить систему на ' + S2.disk + '?',
              'Файлы самой системы будут заменены. Личные файлы останутся на месте.',
              { icon:'🛠', okText:'Восстановить' })
          : await Dlg.confirm('Стереть диск ' + S2.disk + '?',
              'Все разделы и данные на этом диске исчезнут. Это необратимо.',
              { icon:'💽', okText:'Стереть и установить', danger:true });
        if (!ok) return;
      }
      S2.step++;
      draw();
    };

    draw();
  }
};

/* ---------- показываем мастер только там, где он имеет смысл ---------- */
/* Если что-то в опознании сорвалось, пишем это в файл рабочей папки: на живой
   машине консоли не видно, и без записи причину отказа не узнать. При
   нормальной работе файл не появляется. */
let журнал = '';
async function отметка(текст){
  журнал += new Date().toISOString() + '  ' + текст + '\n';
  try { await Platform.rpc('fs.write', { path:'установка.log', body:журнал }); } catch(e){}
}

(async function offer(){
  /* Слабая машина отвечает медленно и не с первого раза: пока система
     раскачивается, запрос к агенту может не пройти. Одной попытки поэтому
     мало — переспрашиваем, но не дольше пяти минут. */
  const пауза = ms => new Promise(r => setTimeout(r, ms));
  let can = null;
  for (let i = 0; i < 100; i++){
    if (window.OS && OS.on()){
      can = await Install.can();
      if (can.allowed && !can.reason) break;
      if (can.reason && /живой системы/.test(can.reason)) return;   // уже установлена
    }
    await пауза(3000);
    can = null;
  }
  if (!can) return;

  APPS.installer = INSTALLER_APP;
  try { if (window.Shell && Shell.renderShell) Shell.renderShell(); }
  catch(e){ отметка('перерисовка оболочки упала: ' + e.message); }

  try { addDesktopIcon(); }
  catch(e){ отметка('значок не поставился: ' + e.message); }

  /* Установочная среда: мастер открывается сам и на весь экран */
  if (/[?&]install=1/.test(location.search)){
    /* Установка идёт на тёмном: светлое окно во весь экран бьёт по глазам,
       особенно ночью и в виртуальной машине с ярким фоном. Тему ставим только
       на это время и не запоминаем — у поставленной системы свои настройки. */
    try { S.theme = 'dark'; applySettings(); } catch(e){}
    setTimeout(() => {
      try {
        const w = WM.open('installer');
        if (w && !w.maximized) WM.toggleMax(w);
      } catch(e){ отметка('открыть мастер не вышло: ' + e.message); }
    }, 1200);
    return;
  }

  Shell.toast('Установка', 'Систему можно перенести на диск — откройте «Установка ' + Brand.name + '»', '💽', 9000);
})();


/* ---------- значок «Установить» на рабочем столе ---------- */
/* Рабочий стол перерисовывает свои значки целиком, поэтому дописывать наш
   сбоку бесполезно — он исчезал при первой же перерисовке. Вклиниваемся в
   саму отрисовку. */
function addDesktopIcon(){
  if (Shell.__insIcon) return;
  Shell.__insIcon = true;
  const orig = Shell.renderIcons.bind(Shell);
  Shell.renderIcons = function(){
    orig();
    const box = document.getElementById('desktop-icons');
    if (!box || box.querySelector('.ins-desk')) return;
    const n = el('div', 'di ins-desk',
      `<div class="glyph">💽</div><div class="lbl">Установить ${esc(Brand.name)}</div>`);
    n.onclick = e => { e.stopPropagation();
      box.querySelectorAll('.di').forEach(x => x.classList.remove('sel')); n.classList.add('sel'); };
    n.ondblclick = () => WM.open('installer');
    n.oncontextmenu = e => { e.preventDefault(); e.stopPropagation();
      Shell.ctx(e.clientX, e.clientY, [{ i:'💽', t:'Установить систему', f:() => WM.open('installer') }]); };
    box.appendChild(n);
  };
  Shell.renderIcons();
}
