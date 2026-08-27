/* ==========================================================================
   Настоящая машина внутри оболочки

   Если агент запущен с ключом --system, оболочка перестаёт быть страницей
   про систему и становится оболочкой самой машины: настоящие процессы,
   настоящая громкость и яркость, настоящий список установленных программ,
   настоящее выключение. Чего машина не умеет — о том говорится прямо.
   ========================================================================== */
'use strict';

const OS = {
  caps:null,

  on(){ return Platform.mode === 'native' && !!this.caps; },
  can(what){ return !!(this.caps && this.caps.allow && this.caps.allow[what]); },
  tool(name){ return !!(this.caps && this.caps.tools && this.caps.tools[name]); },

  async load(){
    if (Platform.mode !== 'native' || !Platform.info.system) return false;
    try { this.caps = await Platform.rpc('sys.caps'); return true; }
    catch(e){ return false; }
  },

  /* ---------- звук ---------- */
  async volume(){ return Platform.rpc('sys.volume.get'); },
  async setVolume(v){ return Platform.rpc('sys.volume.set', { volume:v }); },

  /* ---------- яркость ---------- */
  async brightness(){ return Platform.rpc('sys.brightness.get'); },
  async setBrightness(v){ return Platform.rpc('sys.brightness.set', { value:v }); },

  /* ---------- сеть, процессы, программы ---------- */
  async net(){ return Platform.rpc('sys.net'); },
  async procs(){ return Platform.rpc('sys.procs'); },
  async apps(){ return Platform.rpc('sys.apps'); },
  async launch(id){ return Platform.rpc('sys.launch', { id }); },

  /* ---------- Wi-Fi ---------- */
  async wifiState(){ return Platform.rpc('sys.wifi.state'); },
  async wifiScan(rescan){ return Platform.rpc('sys.wifi.scan', { rescan:!!rescan }); },
  async wifiSaved(){ return Platform.rpc('sys.wifi.saved'); },
  async wifiConnect(ssid, password){ return Platform.rpc('sys.wifi.connect', { ssid, password }); },
  async wifiDisconnect(device){ return Platform.rpc('sys.wifi.disconnect', { device }); },
  async wifiForget(ssid){ return Platform.rpc('sys.wifi.forget', { ssid }); },
  async wifiRadio(on){ return Platform.rpc('sys.wifi.radio', { on }); },

  /* ---------- железо: спрашиваем машину, а не браузер ---------- */
  async hardware(){ return Platform.rpc('sys.hardware'); },
  async batteryInfo(){ return Platform.rpc('sys.battery'); },
  async devices(){ return Platform.rpc('sys.devices'); },

  /* ---------- питание ---------- */
  async power(action){ return Platform.rpc('sys.power', { action }); }
};
window.OS = OS;

/* ==========================================================================
   Подключение к живой машине
   ========================================================================== */
(async function boot(){
  /* ждём, пока Platform договорится с агентом */
  for (let i = 0; i < 30 && Platform.mode !== 'native'; i++)
    await new Promise(r => setTimeout(r, 200));
  if (!await OS.load()) return;

  document.body.classList.add('os-native');
  Shell.toast('Система', `Оболочка управляет машиной ${OS.caps.host}`, '🖥️', 5000);

  syncVolume();
  wirePower();
  wireApps();
  wireMachineApps();
  wireTaskManager();
  wireNativeWindows();
  wireRealApps();
  wireNotifications();
})();

/* ---------- громкость системы вместо громкости страницы ---------- */
async function syncVolume(){
  if (!OS.tool('wpctl') && !OS.tool('amixer')) return;
  try {
    const v = await OS.volume();
    if (v.volume != null){ Store.set('volume', v.volume); Shell.updateCC(); }
  } catch(e){}

  /* всё, что двигает громкость в системе, двигает её и на машине */
  const set = Store.set.bind(Store);
  Store.set = function(key, value){
    const r = set(key, value);
    if (key === 'volume' && OS.on()) OS.setVolume(value).catch(() => {});
    if (key === 'brightness' && OS.on() && OS.tool('brightnessctl')) OS.setBrightness(value).catch(() => {});
    return r;
  };
}

/* ---------- настоящее выключение ---------- */
function wirePower(){
  const orig = Shell.power.bind(Shell);
  Shell.power = async function(act){
    if (!OS.on() || !OS.can('power')) return orig(act);
    const map = { shutdown:['poweroff', 'Выключение', 'Машина выключится.'],
                  restart:['reboot', 'Перезагрузка', 'Машина перезагрузится.'],
                  sleep:['suspend', 'Спящий режим', 'Машина уснёт.'] };
    const m = map[act];
    if (!m) return orig(act);
    const ov = $('#power-overlay'); if (ov) ov.classList.remove('on');
    if (!await Dlg.confirm(m[1], m[2] + ' Это действие затронет всю машину, а не только оболочку.',
        { icon:'⏻', okText:m[1], danger:true })) return;

    /* Экран гаснет сразу, как в настоящей системе. Но если машина откажется
       выключаться, занавес надо убрать и сказать почему — иначе человек
       остаётся перед чёрным экраном работающего компьютера. */
    const fade = el('div', 'shutdown-fade');
    fade.innerHTML = act === 'restart'
      ? '<div style="text-align:center"><div class="boot-ring"><svg viewBox="0 0 50 50"><circle cx="25" cy="25" r="20"/></svg></div><div style="margin-top:14px;opacity:.7">Перезагрузка…</div></div>'
      : act === 'sleep' ? '<div style="opacity:.6">Засыпаю…</div>'
      : '<div style="opacity:.6">Завершение работы…</div>';
    document.body.appendChild(fade);

    try { await OS.power(m[0]); }
    catch(e){
      fade.remove();
      Dlg.alert(m[1] + ' не удалась', String(e.message || e), '⚠️');
      return;
    }
    /* Машина уходит не мгновенно: подождём, и если через десять секунд мы
       всё ещё здесь — значит, не ушла. */
    if (act !== 'sleep') setTimeout(() => {
      if (!document.body.contains(fade)) return;
      fade.remove();
      Dlg.alert(m[1], 'Команда принята, но машина всё ещё работает. ' +
        'Похоже, systemd не довёл действие до конца.', '⚠️');
    }, 10000);
    else setTimeout(() => fade.remove(), 1500);
  };
}

/* ---------- настоящие программы машины ---------- */
/* ---------- программы машины в Пуске и поиске ----------
   Поставленную программу человек ищет там же, где остальные: в Пуске и в
   поиске. Раньше она пряталась в отдельном окне «Программы машины», и найти
   её было нельзя — только знать, где смотреть. */
let списокМашины = [];

/* ---------- настоящие значки настоящих программ ----------

   У каждой программы Linux есть свой значок, и человек узнаёт программу
   именно по нему. Рисовать вместо Telegram общий квадратик — то же
   притворство, от которого мы уходим. Имя значка записано в ярлыке,
   изображение достаёт система, а здесь оно просто показывается.

   Изображения помним: одно и то же не просим дважды. */
const значкиПамять = new Map();

function дайЗначок(имя){
  if (!имя) return Promise.resolve(null);
  if (значкиПамять.has(имя)) return значкиПамять.get(имя);
  const обещание = Platform.rpc('sys.icon', { имя })
    .then(d => (d && d.есть) ? d.данные : null)
    .catch(() => null);
  значкиПамять.set(имя, обещание);
  return обещание;
}

/* Подставить настоящее изображение в уже нарисованный кружок значка.
   Пока изображение едет, виден запасной рисунок — пустого места не будет. */
function поставьЗначок(узел, имя){
  дайЗначок(имя).then(src => {
    if (!src || !узел.isConnected) return;
    узел.textContent = '';
    узел.classList.add('свой');
    узел.style.background = 'transparent';
    const и = document.createElement('img');
    и.src = src; и.alt = ''; и.draggable = false;
    узел.appendChild(и);
  });
  return узел;
}

/* Кружок значка для программы машины. */
function значокМашины(a, класс){
  const d = el('div', класс || 'app-ico', a.flatpak ? '🫙' : '🐧');
  d.style.background = 'linear-gradient(140deg,#fbbf24,#b45309)';
  поставьЗначок(d, a.значок);
  return d;
}

async function обновиСписокМашины(){
  try {
    const d = await OS.apps();
    списокМашины = (d.list || []).map(a => ({
      id:a.id, name:a.name, comment:a.comment, значок:a.icon || '', окно:a.окно || '',
      flatpak:a.flatpak || /flatpak/.test(a.id) || /^[a-z]+\.[a-zA-Z0-9.]+\.desktop$/.test(a.id)
    }));
  } catch(e){ списокМашины = []; }
}

/* Запуск программы машины из любого места оболочки.

   Если программа не пошла, человек не должен читать строку от системы и
   догадываться, что набрать в терминале. Известную беду система называет
   своими словами и чинит сама, по кнопке; остальное показывает как есть,
   а подробности прячет под отдельной кнопкой — для тех, кому интересно. */
const ПОЧИНКИ = [{
  когда:/ldconfig|bwrap|namespace|пространств|apparmor|userns/i,
  что:'песочница',
  заголовок:'Программе закрыт доступ к песочнице',
  человеку:'Программы, поставленные из Flathub, работают в отдельной песочнице, ' +
    'а системе она сейчас запрещена. Это можно исправить прямо сейчас, и больше ' +
    'спрашивать не придётся.',
  кнопка:'Исправить и запустить'
}];

async function покажиОсмотр(a){
  const части = [];
  try {
    const о = await Platform.rpc('sys.sandbox', { id:(a.id || '').replace(/\.desktop$/, '') });
    части.push(о.текст || '');
  } catch(e){ части.push('осмотр не удался: ' + (e.message || e)); }
  try {
    const ж = await Platform.rpc('sys.log', { строк:60 });
    части.push('', ж.текст || '');
  } catch(e){ части.push('', 'журнал не получен: ' + (e.message || e)); }
  await Dlg.open({ type:'alert', icon:'🔎', title:'Что отвечает система',
    text:'Это для разбирательства, чинить руками ничего не нужно', pre:части.join('\n') });
}

async function запустиПрограмму(a){
  const скажи = () => {
    Shell.toast('Программы машины', 'Запускаю: ' + a.name, a.flatpak ? '🫙' : '🐧');
    /* Программа открывается своим окном поверх рабочего стола. Один раз за
       сеанс подсказываем, как вернуться, — дальше человек уже знает. */
    if (!sessionStorage.getItem('glower.окноПодсказка')){
      try { sessionStorage.setItem('glower.окноПодсказка', '1'); } catch(e){}
      setTimeout(() => Shell.toast('Как вернуться',
        'Программа открылась своим окном. Она есть в панели задач, а Super + D возвращают на рабочий стол.',
        '🪟', 9000), 1200);
    }
  };
  try { await OS.launch(a.id); скажи(); return; }
  catch(e){
    const текст = String(e.message || e);
    const беда = ПОЧИНКИ.find(п => п.когда.test(текст));

    if (!беда){
      const ещё = await Dlg.open({ type:'confirm', icon:'⚠️', title:'Не удалось запустить ' + a.name,
        text:текст, okText:'Закрыть', cancelText:'Подробности' });
      if (!ещё) await покажиОсмотр(a);
      return;
    }

    const чинить = await Dlg.open({ type:'confirm', icon:'🔧', title:беда.заголовок,
      text:беда.человеку + '\n\n' + a.name + ' — ' + текст,
      okText:беда.кнопка, cancelText:'Не сейчас' });
    if (!чинить) return;

    try {
      await Platform.rpc('sys.fix', { что:беда.что });
      await OS.launch(a.id);
      скажи();
      Shell.toast('Готово', 'Больше эта беда не повторится', '🔧');
    } catch(e2){
      const ещё = await Dlg.open({ type:'confirm', icon:'⚠️', title:'Починить не вышло',
        text:String(e2.message || e2), okText:'Закрыть', cancelText:'Подробности' });
      if (!ещё) await покажиОсмотр(a);
    }
  }
}

function wireMachineApps(){
  обновиСписокМашины();
  /* после установки список меняется — перечитываем его, когда открывают Пуск */
  const start = Shell.renderStart.bind(Shell);
  Shell.renderStart = function(){ обновиСписокМашины(); return start(); };

  /* поиск: программы машины ищутся наравне со всем остальным */
  const искать = Shell.searchAll.bind(Shell);
  Shell.searchAll = function(q){
    const out = искать(q);
    списокМашины.forEach(a => {
      if (!a.name.toLowerCase().includes(q)) return;
      out.push({ ico:{ glyph:a.flatpak ? '🫙' : '🐧',
                       bg:'linear-gradient(140deg,#fbbf24,#b45309)', значок:a.значок },
        t:a.name, s:a.comment || 'Программа машины',
        k:'Запуск', run:() => запустиПрограмму(a) });
    });
    return out;
  };

  /* «Все приложения»: список машины идёт следом за приложениями системы */
  const все = Shell.allApps.bind(Shell);
  Shell.allApps = function(on){
    const r = все(on);
    if (!on || !списокМашины.length) return r;
    const res = document.getElementById('start-results');
    if (!res) return r;
    res.appendChild(el('div', 'all-letter', 'Программы машины'));
    списокМашины.forEach(a => {
      const b = el('button', 'all-row');
      b.appendChild(значокМашины(a));
      b.appendChild(el('div', 't', esc(a.name)));
      b.onclick = () => { Shell.closePanels(); запустиПрограмму(a); };
      res.appendChild(b);
    });
    return r;
  };
}

function wireApps(){
  APPS.native = {
    name:'Программы машины', glyph:'🐧', bg:'linear-gradient(140deg,#fbbf24,#b45309)', w:720, h:600, single:true,
    async render(win){
      const wrap = el('div', 'app col'); win.body.appendChild(wrap);
      const bar = el('div', 'toolbar');
      const find = el('input', 'inp grow'); find.placeholder = '🔎 Поиск программы';
      bar.appendChild(find);
      const list = el('div', 'scroll pad');
      wrap.append(bar, list);

      let data = { list:[], total:0, canLaunch:false };
      try { data = await OS.apps(); }
      catch(e){ list.appendChild(el('div', 'empty', 'Не удалось получить список: ' + e.message)); return; }

      const draw = () => {
        const q = find.value.trim().toLowerCase();
        const items = data.list.filter(a => !q || a.name.toLowerCase().includes(q));
        list.innerHTML = '';
        win.setSub(`${data.total} программ на машине`);
        if (!items.length) return list.appendChild(el('div', 'empty', 'Ничего не найдено'));
        items.forEach(a => {
          const b = el('button', 'btn' + (data.canLaunch ? ' pri' : ''), data.canLaunch ? 'Запустить' : 'Запуск выключен');
          b.disabled = !data.canLaunch;
          b.onclick = () => запустиПрограмму(a);
          const строка = row('', a.name, a.comment || a.id, b);
          /* Значок настоящей программы вместо общего рисунка */
          const кружок = строка.querySelector('.emo');
          if (кружок){ кружок.textContent = a.flatpak ? '🫙' : '🐧'; поставьЗначок(кружок, a.icon); }
          list.appendChild(строка);
        });
        if (!data.canLaunch)
          list.appendChild(el('div', 'set-note',
            'Список настоящий — он прочитан из .desktop-файлов машины. Запуск выключен: ' +
            'агент должен быть запущен с ключом --allow-launch.'));
      };
      find.oninput = draw;
      draw();
    }
  };
  if (window.Shell && Shell.renderShell) Shell.renderShell();
}

/* ---------- диспетчер задач показывает настоящие процессы ---------- */
function wireTaskManager(){
  const render = APPS.taskmgr.render;
  APPS.taskmgr.render = function(win, opts){
    render.call(this, win, opts);
    if (!OS.on()) return;

    const body = win.body;
    body.style.flexDirection = 'column';
    const bar = el('div', 'toolbar');
    const btn = el('button', 'btn pri', '🐧 Процессы машины');
    bar.appendChild(btn);
    body.prepend(bar);

    const box = el('div', 'scroll pad');
    box.style.display = 'none';
    let timer = null;

    const draw = async () => {
      let d;
      try { d = await OS.procs(); }
      catch(e){ box.innerHTML = ''; box.appendChild(el('div', 'empty', 'Не удалось прочитать /proc: ' + e.message)); return; }
      const mb = b => (b / 1048576).toFixed(1) + ' МБ';
      box.innerHTML = `<div class="card" style="padding:0">
        <div class="set-row"><div class="l"><b>Процессов</b><small>всего в системе</small></div><div class="ctl">${d.total}</div></div>
        <div class="set-row"><div class="l"><b>Память</b><small>занято на машине</small></div>
          <div class="ctl">${mb(d.mem.total - d.mem.free)} из ${mb(d.mem.total)}</div></div>
        <div class="set-row"><div class="l"><b>Средняя нагрузка</b><small>1, 5 и 15 минут</small></div>
          <div class="ctl">${d.load.map(x => x.toFixed(2)).join(' · ')}</div></div></div>
        <div class="fe-table" style="margin-top:12px">
          <div class="fe-tr head"><div class="c1">Процесс</div><div>PID</div><div>ЦП</div><div>Память</div></div>
          ${d.list.map(p => `<div class="fe-tr"><div class="c1">${esc(p.name)}</div>
            <div class="tiny muted">${p.pid}</div><div class="tiny">${p.cpu}%</div>
            <div class="tiny">${mb(p.mem)}</div></div>`).join('')}
        </div>`;
    };

    /* показываем что-то одно: либо окна оболочки, либо процессы машины */
    const others = () => [...body.children].filter(n => n !== bar && n !== box);

    btn.onclick = () => {
      const on = btn.classList.toggle('on');
      others().forEach(n => n.style.display = on ? 'none' : '');   // hidden не работает: у .app свой display
      if (on){
        body.appendChild(box);
        box.style.display = '';
        draw(); timer = setInterval(draw, 2000);
        btn.textContent = '↩ Вернуться к окнам';
      } else {
        box.style.display = 'none'; clearInterval(timer); timer = null;
        btn.textContent = '🐧 Процессы машины';
      }
    };
    const close = win.onClose;
    win.onClose = () => { clearInterval(timer); if (close) close(); };
  };
}

/* ---------- настоящие окна машины в панели задач ----------

   Программа машины открывается своим окном — настоящим окном оконного
   сервера, а не нарисованным. Раньше такое окно закрывало собой всё и пути
   назад не оставляло. Теперь оно встаёт в панель задач рядом с нашими:
   щелчок — перейти к нему, правая кнопка — закрыть. */
let чужиеОкна = [];
let списокЧитан = 0;

/* Чьё это окно: оконный сервер называет программу коротким именем
   (app_id), а ярлык — своим. Сводим их: по имени файла ярлыка, по
   записанному в ярлыке имени окна и по названию программы. */
function ярлыкОкна(o){
  const имя = String(o.appId || '').toLowerCase();
  if (!имя) return null;
  const без = я => String(я || '').replace(/\.desktop$/, '').toLowerCase();
  return списокМашины.find(a => без(a.id) === имя)
      || списокМашины.find(a => (a.окно || '').toLowerCase() === имя)
      || списокМашины.find(a => без(a.id).split('.').pop() === имя)
      || списокМашины.find(a => (a.name || '').toLowerCase() === имя)
      || null;
}

function значокЧужого(o){
  const свой = ярлыкОкна(o);
  const d = el('div', 'emo', свой ? (свой.flatpak ? '🫙' : '🐧') : '🪟');
  /* Если ярлык не нашёлся, пробуем имя окна как имя значка: у многих
     программ они совпадают (firefox, org.telegram.desktop). Не нашлось и
     так — останется наш рисунок, а не пустое место. */
  поставьЗначок(d, (свой && свой.значок) || o.appId);
  return d;
}

function нарисуйЧужие(){
  const box = document.getElementById('dock-running');
  if (!box) return;
  чужиеОкна.forEach(o => {
    const с = o.состояние || {};
    const b = el('button', 'dock-item run'
      + (с.активно ? ' active' : '') + (с.развёрнуто ? ' развёрнуто' : ''));
    b.dataset.tip = (o.title || o.appId) + ' — окно машины'
      + (с.вовесь ? ' (во весь экран)' : с.развёрнуто ? ' (развёрнуто)' : '');
    b.appendChild(значокЧужого(o));
    b.onclick = () => Platform.rpc('sys.window', { action:'focus', appId:o.appId, title:o.title })
      .catch(e => Dlg.alert('Не удалось перейти к окну', String(e.message || e), '⚠️'));
    b.oncontextmenu = async e => {
      e.preventDefault();
      if (await Dlg.confirm('Закрыть окно?', o.title || o.appId, { okText:'Закрыть', danger:true }))
        Platform.rpc('sys.window', { action:'close', appId:o.appId, title:o.title })
          .then(() => setTimeout(обновиЧужие, 300))
          .catch(er => Dlg.alert('Не удалось закрыть', String(er.message || er), '⚠️'));
    };
    box.appendChild(b);
  });
  const сеп = document.getElementById('dock-sep-run');
  if (сеп && чужиеОкна.length) сеп.hidden = false;
  if (Shell.fitDock) Shell.fitDock();
}

async function обновиЧужие(){
  if (document.hidden) return;
  try {
    const d = await Platform.rpc('sys.windows');
    const было = JSON.stringify(чужиеОкна);
    чужиеОкна = (d.list || []).filter(o => !o.оболочка);
    if (JSON.stringify(чужиеОкна) !== было) Shell.syncDock();
    вовесьЭкран(!!d.вовесьЭкран);
    /* Открылось окно программы, которой в нашем списке ещё нет (её только
       что поставили) — перечитаем список, чтобы у окна появились имя и
       значок. Перечитываем не чаще раза в полминуты. */
    if (чужиеОкна.some(o => !ярлыкОкна(o)) && Date.now() - списокЧитан > 30000){
      списокЧитан = Date.now();
      обновиСписокМашины();
    }
  } catch(e){ /* оконный сервер может и не уметь этого — тогда просто молчим */ }
}

/* Чужая программа во весь экран: панель задач уходит с дороги и перестаёт
   держать за собой полосу — экран в этот миг принадлежит программе, как в
   любой другой системе. Когда программа выходит из полного экрана, панель
   возвращается сама. */
let былоВовесь = false;
function вовесьЭкран(да){
  if (да === былоВовесь) return;
  былоВовесь = да;
  document.body.classList.toggle('чужой-вовесь', да);
  if (да){ if (Shell.скажиПолосу) Shell.скажиПолосу(0); }
  else setTimeout(() => {
    if (Shell.полосаЗабыть) Shell.полосаЗабыть();
    if (Shell.tellPanelHeight) Shell.tellPanelHeight();
  }, 500);
}

function wireNativeWindows(){
  const прежний = Shell.syncDock.bind(Shell);
  Shell.syncDock = function(){ const r = прежний(); нарисуйЧужие(); return r; };
  обновиЧужие();
  setInterval(обновиЧужие, 3000);
}

/* ---------- настоящие программы вместо нарисованных ----------

   Раньше «Файлы», «Текст», «Калькулятор» и прочее были рисунками внутри
   одной страницы: они умели ровно то, что им написали, и ничего сверх.
   На живой машине их место занимают настоящие программы Linux — со своими
   окнами, своими возможностями и своей судьбой. Значок и имя остаются
   прежними, чтобы человеку не пришлось ничего переучивать.

   Там, где системы под оболочкой нет (обычный браузер), всё остаётся как
   было: нарисованное лучше, чем пустое место. */
const НАСТОЯЩИЕ = {
  term:    { вызов:'sys.terminal', подпись:'Терминал системы' },
  files:   { ярлыки:['thunar.desktop', 'org.xfce.thunar.desktop', 'org.gnome.Nautilus.desktop',
                     'nautilus.desktop', 'pcmanfm.desktop'], подпись:'Файлы системы' },
  notepad: { ярлыки:['org.xfce.mousepad.desktop', 'mousepad.desktop',
                     'org.gnome.TextEditor.desktop', 'gedit.desktop'], подпись:'Текстовый редактор' },
  calc:    { ярлыки:['org.gnome.Calculator.desktop', 'gnome-calculator.desktop',
                     'galculator.desktop'], подпись:'Калькулятор системы' },
  photos:  { ярлыки:['org.gnome.eog.desktop', 'eog.desktop', 'org.gnome.Loupe.desktop',
                     'ristretto.desktop'], подпись:'Просмотр изображений' },
  music:   { ярлыки:['mpv.desktop', 'io.mpv.Mpv.desktop'], подпись:'Проигрыватель' },
  browser: { ярлыки:['firefox.desktop', 'firefox-esr.desktop',
                     'org.gnome.Epiphany.desktop', 'epiphany-browser.desktop',
                     'chromium.desktop', 'chromium-browser.desktop',
                     'google-chrome.desktop'], подпись:'Браузер системы' }
};

/* Нарисованные приложения, которым на живой машине замены нет: они уходят,
   а не притворяются. Пусть лучше их не будет, чем будет подделка. */
const УБРАТЬ_НА_МАШИНЕ = ['paint', 'todo', 'calendar', 'clock', 'trash'];

/* У терминала своего ярлыка в наших списках нет — его ищет агент, — но
   значок у настоящего терминала машины всё-таки есть. */
const ТЕРМИНАЛЫ = ['foot.desktop', 'org.gnome.Terminal.desktop', 'kitty.desktop',
  'alacritty.desktop', 'xterm.desktop', 'debian-xterm.desktop'];

async function wireRealApps(){
  let список = [];
  try { список = (await OS.apps()).list || []; } catch(e){}
  const есть = список.map(a => a.id);
  const значокЯрлыка = я => (список.find(a => a.id === я) || {}).icon || '';

  const найден = {};
  Object.entries(НАСТОЯЩИЕ).forEach(([id, о]) => {
    if (!APPS[id]) return;
    if (о.вызов){                                          // терминал ищет агент сам
      найден[id] = Object.assign(
        { значок:значокЯрлыка(ТЕРМИНАЛЫ.find(я => есть.includes(я))) }, о);
      return;
    }
    const ярлык = (о.ярлыки || []).find(я => есть.includes(я));
    if (ярлык) найден[id] = Object.assign({ ярлык, значок:значокЯрлыка(ярлык) }, о);
  });

  Object.entries(найден).forEach(([id, о]) => {
    APPS[id].sub = о.подпись;
    APPS[id].настоящее = true;
    /* Значок — настоящий, программы: человек должен узнавать её так же,
       как в любой другой системе, а не по нашему рисунку. */
    if (о.значок) APPS[id].значок = о.значок;
  });

  УБРАТЬ_НА_МАШИНЕ.forEach(id => { delete APPS[id]; });
  /* Убранное не должно оставаться ни в панели, ни в Пуске: значок, за
     которым ничего нет, — та же подделка, только меньше. */
  ['dockApps', 'pinned'].forEach(ключ => {
    const было = S[ключ] || [];
    const стало = было.filter(id => APPS[id]);
    if (стало.length !== было.length){ S[ключ] = стало; Store.save(); }
  });

  /* Значки рабочего стола рисуются отдельно: убираем оттуда те, что вели
     к нарисованным приложениям, и открываем настоящие вместо них. */
  const прежниеЗначки = Shell.renderIcons.bind(Shell);
  Shell.renderIcons = function(){
    const r = прежниеЗначки();
    const короб = document.getElementById('desktop-icons');
    if (короб) [...короб.children].forEach(n => {
      const имя = (n.querySelector('.lbl') || {}).textContent || '';
      if (имя === 'Корзина' && !APPS.trash) n.remove();
    });
    return r;
  };

  const открыть = async id => {
    const о = найден[id];
    try {
      if (о.вызов) await Platform.rpc(о.вызов, {});
      else await OS.launch(о.ярлык);
      Shell.toast(APPS[id] ? APPS[id].name : 'Программа', 'Открываю', '🚀');
    } catch(e){
      Dlg.alert('Не удалось открыть', String(e.message || e), '⚠️');
    }
  };

  const прежнийЗапуск = Shell.launch.bind(Shell);
  Shell.launch = function(id, btn){
    if (!найден[id]) return прежнийЗапуск(id, btn);
    if (btn){ btn.classList.add('bounce'); setTimeout(() => btn.classList.remove('bounce'), 700); }
    Shell.closePanels();
    открыть(id);
  };

  const прежнееОкно = WM.open.bind(WM);
  WM.open = function(id, opts){
    if (!найден[id]) return прежнееОкно(id, opts);
    открыть(id);
    return null;
  };

  Shell.renderShell();
}

/* ---------- уведомления настоящих программ ----------

   Программы Linux сообщают о событиях через шину сеанса. Оболочка их
   принимает (это делает программа-хозяин) и передаёт сюда — а здесь они
   показываются так же, как свои: всплывающим окошком и записью в центре
   уведомлений. Для человека разницы нет, и это правильно. */
function wireNotifications(){
  let с = 0, работаем = true;

  const слушай = async () => {
    while (работаем){
      try {
        const d = await Platform.rpc('ui.hear', { с, темы:['уведомление'] });
        с = d.n || с;
        (d.список || []).forEach(м => покажи(м.что));
      } catch(e){ await new Promise(r => setTimeout(r, 3000)); }
    }
  };

  const покажи = у => {
    if (!у) return;
    const заголовок = String(у.заголовок || у.откуда || 'Уведомление').slice(0, 120);
    const текст = String(у.текст || '').replace(/<[^>]*>/g, '').slice(0, 300);
    Shell.toast(заголовок, текст || String(у.откуда || ''), '🔔', 7000);
  };

  слушай();
}

/* проверкам нужно звать эту сборку напрямую */
window.wireRealApps = wireRealApps;
