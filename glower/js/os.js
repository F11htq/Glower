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

async function обновиСписокМашины(){
  try {
    const d = await OS.apps();
    списокМашины = (d.list || []).map(a => ({
      id:a.id, name:a.name, comment:a.comment,
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
  let осмотр;
  try { осмотр = await Platform.rpc('sys.sandbox', { id:(a.id || '').replace(/\.desktop$/, '') }); }
  catch(e){ осмотр = { текст:'осмотр не удался: ' + (e.message || e) }; }
  await Dlg.open({ type:'alert', icon:'🔎', title:'Что отвечает система',
    text:'Это для разбирательства, чинить руками ничего не нужно', pre:осмотр.текст || '' });
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
      out.push({ emo:a.flatpak ? '🫙' : '🐧', t:a.name, s:a.comment || 'Программа машины',
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
      const ico = el('div', 'app-ico', a.flatpak ? '🫙' : '🐧');
      ico.style.background = 'linear-gradient(140deg,#fbbf24,#b45309)';
      b.appendChild(ico);
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
          list.appendChild(row('📦', a.name, a.comment || a.id, b));
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

function значокЧужого(o){
  const свой = списокМашины.find(a => (a.id || '').replace(/\.desktop$/, '') === o.appId);
  return свой ? (свой.flatpak ? '🫙' : '🐧') : '🪟';
}

function нарисуйЧужие(){
  const box = document.getElementById('dock-running');
  if (!box) return;
  чужиеОкна.forEach(o => {
    const b = el('button', 'dock-item run');
    b.dataset.tip = (o.title || o.appId) + ' — окно машины';
    b.appendChild(el('div', 'emo', значокЧужого(o)));
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
  } catch(e){ /* оконный сервер может и не уметь этого — тогда просто молчим */ }
}

function wireNativeWindows(){
  const прежний = Shell.syncDock.bind(Shell);
  Shell.syncDock = function(){ const r = прежний(); нарисуйЧужие(); return r; };
  обновиЧужие();
  setInterval(обновиЧужие, 3000);
}
