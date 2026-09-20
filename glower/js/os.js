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
  /* «сразу» — перезагрузка без аккуратного отключения. Нужна только там,
     где корень системы лежит на носителе, который вот-вот исчезнет. */
  async power(action, сразу){ return Platform.rpc('sys.power', { action, 'сразу':!!сразу }); }
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
  /* Щадящий режим включаем сами — один раз, на слабой машине.
  
     Размытие и длинные переходы на двухъядерной машине с парой гигабайт
     памяти стоят дороже всего, что есть на экране, и именно из-за них
     кажется, что программы открываются медленно. Спрашивать об этом
     человека бессмысленно: он не обязан знать, что такое backdrop-filter.
     Решаем один раз и больше не трогаем — переключатель в «Питании»
     остаётся за ним. */
  if (!KV.get('экономРешено', false)){
    KV.set('экономРешено', true);
    Platform.rpc('sys.hardware').then(ж => {
      const ядер = ж && ж.cpu && ж.cpu.cores;
      const гб = ж && ж.mem && ж.mem.total / 1073741824;
      if ((ядер && ядер <= 2) || (гб && гб < 4)){
        KV.set('ecoMode', true);
        applySettings();
      }
    }).catch(() => {});
  }

  /* Новую версию система замечает сама.
  
     Обновления теперь приходят обычным пакетом, и в «Параметрах» есть
     кнопка. Но кнопку нажимает тот, кто знает, что её надо нажать, —
     а человек узнаёт о новой версии, только если ему сказать. Спрашиваем
     раз в шесть часов, тихо: если для самой системы ничего нет, он об
     этом даже не узнает.
  
     Время последней проверки держим у себя, а не в памяти машины: иначе
     после перезагрузки система спрашивала бы заново при каждом входе. */
  const ПРОВЕРКА = 6 * 3600 * 1000;
  setTimeout(async () => {
    if (Date.now() - KV.get('обновления.спрошено', 0) < ПРОВЕРКА) return;
    KV.set('обновления.спрошено', Date.now());
    try {
      await Platform.rpc('pkg.update', {});
      /* Списки читаются не мгновенно — подождём, пока работа кончится. */
      for (let i = 0; i < 40; i++){
        const j = await Platform.rpc('pkg.job').catch(() => null);
        if (!j || !j.running) break;
        await new Promise(r => setTimeout(r, 3000));
      }
      const д = await Platform.rpc('pkg.upgrade.check');
      const наш = (д.list || []).find(x => x.name === 'glower');
      if (наш && Shell.toast)
        Shell.toast('Есть новая версия', 'GlowerOS ' + наш['станет']
          + ' · «Параметры» → «Обновления»', '✨', 15000,
          () => WM.open('settings', { section:'update' }));
    } catch(e){ /* нет сети или репозиторий молчит — не повод шуметь */ }
  }, 60000);

  /* Это новость ровно один раз: в первый запуск на новой машине. Дальше
     человек и так знает, чем он пользуется. */
  if (!KV.get('сказаноПроМашину', false)){
    KV.set('сказаноПроМашину', true);
    Shell.toast('Система', `Оболочка управляет машиной ${OS.caps.host}`, '🖥️', 5000);
  }

  syncVolume();
  wirePower();
  wireApps();
  wireMachineApps();
  wireNativeWindows();
  wireRealApps();
  wireTaskManager();
  wireNotifications();
  wireDrives();
  wireTray();
  /* Пароль на входе — системный: спрашиваем у машины, задан ли он */
  if (window.Profiles && Profiles.узнайПроПароль) Profiles.узнайПроПароль();
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
                  sleep:['suspend', 'Спящий режим', 'Машина уснёт.'],
                  logout:['logout', 'Выйти из системы',
                          'Программы закроются, и вы вернётесь к экрану входа. Машина при этом останется включённой.'] };
    const m = map[act];
    if (!m) return orig(act);
    const ov = $('#power-overlay'); if (ov) ov.classList.remove('on');
    /* Выход из системы машину не трогает — не надо пугать человека тем,
       чего не произойдёт. Остальным трём предупреждение по делу. */
    const хвост = act === 'logout' ? '' : ' Это действие затронет всю машину, а не только оболочку.';
    if (!await Dlg.confirm(m[1], m[2] + хвост,
        { icon:act === 'logout' ? '🚪' : '⏻', okText:m[1], danger:true })) return;

    /* Экран гаснет сразу, как в настоящей системе. Но если машина откажется
       выключаться, занавес надо убрать и сказать почему — иначе человек
       остаётся перед чёрным экраном работающего компьютера. */
    const fade = el('div', 'shutdown-fade');
    fade.innerHTML = act === 'restart'
      ? '<div style="text-align:center"><div class="boot-ring"><svg viewBox="0 0 50 50"><circle cx="25" cy="25" r="20"/></svg></div><div style="margin-top:14px;opacity:.7">Перезагрузка…</div></div>'
      : act === 'sleep' ? '<div style="opacity:.6">Засыпаю…</div>'
      : act === 'logout' ? '<div style="opacity:.6">Выхожу из системы…</div>'
      : '<div style="opacity:.6">Завершение работы…</div>';
    document.body.appendChild(fade);

    try { await OS.power(m[0]); }
    catch(e){
      fade.remove();
      Dlg.alert(m[1] + ' не удалась', String(e.message || e), '⚠️');
      return;
    }
    /* Машина уходит не мгновенно: подождём, и если через десять секунд мы
       всё ещё здесь — значит, не ушла. Для выхода из системы говорим о
       сеансе, а не о машине: машина и не должна была никуда уходить. */
    if (act !== 'sleep') setTimeout(() => {
      if (!document.body.contains(fade)) return;
      fade.remove();
      Dlg.alert(m[1], act === 'logout'
        ? 'Команда принята, но сеанс всё ещё идёт. Похоже, systemd не довёл выход до конца.'
        : 'Команда принята, но машина всё ещё работает. Похоже, systemd не довёл действие до конца.',
        '⚠️');
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
window.значкиПамять = значкиПамять;   /* проверкам нужно уметь её забыть */

function дайЗначок(имя, ярлык){
  if (!имя && !ярлык) return Promise.resolve(null);
  const ключ = (имя || '') + '|' + (ярлык || '');

  /* Найденный значок помним навсегда: картинка не меняется. А «значка нет»
     помним недолго — программу могли только что поставить. */
  const было = значкиПамять.get(ключ);
  if (было && (было.есть || Date.now() - было.когда < 30000)) return было.ответ;

  /* Имя ярлыка передаём вместе с именем значка. Строка Icon= бывает и
     неточной, и вовсе отсутствующей, а по имени ярлыка значок находится
     почти всегда — так поступают и настоящие рабочие столы. */
  const обещание = Platform.rpc('sys.icon', { имя, ярлык })
    .then(d => {
      const src = (d && d.есть) ? d.данные : null;
      значкиПамять.set(ключ, { ответ:Promise.resolve(src), есть:!!src, когда:Date.now() });
      return src;
    })
    .catch(() => null);
  значкиПамять.set(ключ, { ответ:обещание, есть:false, когда:Date.now() });
  return обещание;
}

/* Подставить настоящее изображение в уже нарисованный кружок значка.
   Пока изображение едет, виден запасной рисунок — пустого места не будет. */
function поставьЗначок(узел, имя, ярлык){
  дайЗначок(имя, ярлык).then(src => {
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
  поставьЗначок(d, a.значок, a.id);
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
  /* Панель — отдельная страница и в наши переменные заглянуть не может, а
     меню программ у неё своё. Пока мы об этом молчали, в нём не было ни
     одной настоящей программы машины: ни предустановленной, ни только что
     поставленной — только наши нарисованные. Говорим вслух. */
  OS.машинные = списокМашины;
  try { document.dispatchEvent(new CustomEvent('glower:программы')); } catch(e){}
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

/* Вынести программу машины на рабочий стол и обратно. Держим здесь, потому
   что стол о программах машины ничего не знает: он видит только имя, значок
   и способ запустить — этого достаточно. */
OS.наСтоле = id => (S.deskNative || []).some(x => x.id === id);
OS.наСтол = a => {
  const было = OS.наСтоле(a.id);
  S.deskNative = было ? (S.deskNative || []).filter(x => x.id !== a.id)
    : [...(S.deskNative || []), { id:a.id, name:a.name, значок:a.значок || a.icon || '' }];
  Store.save();
  Shell.renderIcons();
  Shell.toast('Рабочий стол', было ? 'Убрано со стола' : 'Вынесено на стол', '🖥');
};
OS.запустиПоЯрлыку = id => {
  const a = списокМашины.find(x => x.id === id);
  if (a) return запустиПрограмму(a);
  /* Программы уже нет — сказать об этом честнее, чем молчать. */
  Shell.toast('Программы машины', 'Такой программы на машине больше нет', '⚠️');
};

async function запустиПрограмму(a){
  const скажи = () => {
    /* «Запускаю: Firefox» здесь больше не пишем.
    
       Человек нажал на значок и сам знает, что запускает. Значок в доке
       при этом подпрыгивает — этого и довольно. Уведомление же приходило с
       опозданием в полторы секунды (столько агент смотрит, жива ли
       программа), ложилось поверх работы и требовало внимания к тому, что
       и так очевидно. Человек назвал это лишним, и он прав.
    
       Ошибки остаются: о них говорят окном, а не полоской в углу. */
    /* Про чужое окно рассказываем один раз за всю жизнь системы, а не
       каждый сеанс: это знание, а не новость. */
    if (!KV.get('окноПодсказка', false)){
      KV.set('окноПодсказка', true);
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
      b.oncontextmenu = e => {
        e.preventDefault();
        Shell.ctx(e.clientX, e.clientY, [
          { i:'🐧', t:'Открыть', f:() => { Shell.closePanels(); запустиПрограмму(a); } },
          { i:'🖥', t:OS.наСтоле(a.id) ? 'Убрать с рабочего стола' : 'Вынести на рабочий стол',
            f:() => OS.наСтол(a) }
        ]);
      };
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
          const наСтол = el('button', 'btn', OS.наСтоле(a.id) ? '🖥 Убрать со стола' : '🖥 На стол');
          наСтол.title = 'Вынести значок на рабочий стол';
          наСтол.onclick = () => {
            OS.наСтол({ id:a.id, name:a.name, значок:a.icon });
            наСтол.textContent = OS.наСтоле(a.id) ? '🖥 Убрать со стола' : '🖥 На стол';
          };
          const строка = row('', a.name, a.comment || a.id, b);
          строка.querySelector('.ctl').insertBefore(наСтол, b);
          /* Значок настоящей программы вместо общего рисунка */
          const кружок = строка.querySelector('.emo');
          if (кружок){ кружок.textContent = a.flatpak ? '🫙' : '🐧'; поставьЗначок(кружок, a.icon, a.id); }
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

/* ---------- настоящий диспетчер задач ----------

   Раньше он показывал наши окна, а доля процессора и память в нём брались
   случайными числами. Это ровно то притворство, от которого мы уходим.
   На машине он показывает настоящие процессы — те же, что видит любая
   другая программа этой системы, — и снимает задачу по-настоящему. */
function wireTaskManager(){
  if (!APPS.taskmgr) return;
  const прежний = APPS.taskmgr.render;

  APPS.taskmgr.render = function(win, opts){
    if (!OS.on()) return прежний.call(this, win, opts);

    const обёртка = el('div', 'app col'); win.body.appendChild(обёртка);
    const сводка = el('div', 'card'); сводка.style.margin = '10px 12px 0';
    const список = el('div', 'scroll pad');
    обёртка.append(сводка, список);
    let занят = false, часы = null;

    const мб = b => (b / 1048576).toFixed(0) + ' МБ';
    const гб = b => (b / 1073741824).toFixed(1) + ' ГБ';

    const нарисуй = async () => {
      if (занят || !win.node.isConnected) return;
      занят = true;
      let d = null, беда = '';
      try { d = await OS.procs(); } catch(e){ беда = String(e.message || e); }
      занят = false;
      if (!win.node.isConnected) return;

      if (!d){
        список.innerHTML = '';
        список.appendChild(el('div', 'empty', 'Система не дала список процессов: ' + беда));
        return;
      }

      сводка.innerHTML = `<div class="set-row"><div class="l"><b>Процессов</b>
          <small>всего на машине</small></div><div class="ctl">${d.total}</div></div>
        <div class="set-row"><div class="l"><b>Память</b><small>занято из всей</small></div>
          <div class="ctl">${гб(d.mem.total - d.mem.free)} из ${гб(d.mem.total)}</div></div>
        <div class="set-row"><div class="l"><b>Средняя нагрузка</b><small>за 1, 5 и 15 минут</small></div>
          <div class="ctl">${d.load.map(x => x.toFixed(2)).join(' · ')}</div></div>`;

      список.innerHTML = '<div class="tm-row head"><div>Процесс</div><div>ЦП</div><div>Память</div><div></div></div>';

      /* Программы человека и внутренности системы — врозь.
      
         Человек шёл сюда закрыть зависшую программу, а видел список, в
         котором его программа тонет среди WebKitWebProcess, systemd, dbus
         и наших собственных частей. Найти в нём нужное — задача, которой он
         не просил.
      
         Отличаем по строке запуска, а не по короткому имени: WebKit, node и
         python выглядят одинаково у всех, и только полная строка говорит,
         чьи они. */
      const СИСТЕМНОЕ = [
        /glower-/, /server\.mjs/, /labwc/, /WebKit/i, /Xwayland/, /greetd/,
        /systemd/, /dbus/, /polkit/, /udisks/, /upower/, /NetworkManager/,
        /wpa_supplicant/, /ModemManager/, /pipewire/, /wireplumber/, /avahi/,
        /bluetoothd/, /cups/, /rsyslog/, /cron/, /agetty/, /plymouth/,
        /accounts-daemon/, /gvfs/, /at-spi/, /xdg-desktop-portal/
      ];
      const системный = п => {
        /* Потоки ядра идут в квадратных скобках и строки запуска не имеют
           вовсе — их человеку показывать точно незачем. */
        if (!п.cmd) return true;
        const где = п.cmd + ' ' + п.name;
        return СИСТЕМНОЕ.some(в => в.test(где));
      };
      const свои = d.list.filter(п => !системный(п));
      const наши = d.list.filter(системный);

      const заголовок = (текст, число) => {
        const н = el('div', 'tm-row head');
        н.style.opacity = '.75';
        н.innerHTML = `<div>${текст} <small class="muted">${число}</small></div><div></div><div></div><div></div>`;
        return н;
      };

      const строка = п => {
        const r = el('div', 'tm-row');
        r.innerHTML = `<div>${esc(п.name)} <small class="muted">${п.pid}</small>
            <div class="bar"><i style="width:${Math.min(100, п.cpu)}%"></i></div></div>
          <div>${п.cpu.toFixed(1)}%</div><div>${мб(п.mem)}</div>
          <div><button class="btn">Снять</button></div>`;
        r.querySelector('.btn').onclick = async () => {
          if (!await Dlg.confirm('Снять задачу?',
              'Процесс «' + п.name + '» получит просьбу закончить работу. ' +
              'Несохранённое в нём может пропасть.', { okText:'Снять', danger:true })) return;
          try {
            await Platform.rpc('sys.stop', { pid:п.pid });
            Shell.toast('Диспетчер задач', 'Задача снята', '🛑');
          } catch(e){ Dlg.alert('Не вышло снять задачу', String(e.message || e), '⚠️'); }
          setTimeout(нарисуй, 700);
        };
        return r;
      };

      if (свои.length) список.appendChild(заголовок('Программы', свои.length));
      свои.forEach(п => список.appendChild(строка(п)));
      if (!свои.length)
        список.appendChild(el('div', 'empty', 'Ни одной программы человека не запущено'));

      /* Системное показываем по просьбе: оно нужно редко, а места занимает
         больше всего. Выбор запоминаем — кто открыл раз, обычно откроет и
         впредь. */
      const открыто = KV.get('диспетчер.система', false);
      const кнопка = el('button', 'btn');
      кнопка.style.margin = '10px 0 4px';
      кнопка.textContent = (открыто ? '▾ ' : '▸ ') + 'Система и оболочка · ' + наши.length;
      кнопка.onclick = () => { KV.set('диспетчер.система', !открыто); нарисуй(); };
      список.appendChild(кнопка);
      if (открыто) наши.forEach(п => список.appendChild(строка(п)));

      win.setSub(`${d.total} процессов · ${гб(d.mem.total - d.mem.free)} памяти занято`);
    };

    нарисуй();
    часы = setInterval(нарисуй, 2000);
    const прежнееЗакрытие = win.onClose;
    win.onClose = () => { clearInterval(часы); if (прежнееЗакрытие) прежнееЗакрытие(); };
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
  поставьЗначок(d, (свой && свой.значок) || o.appId, свой && свой.id);
  return d;
}

function нарисуйЧужие(){
  const box = document.getElementById('dock-running');
  if (!box) return;
  чужиеОкна.forEach(o => {
    const с = o.состояние || {};
    const b = el('button', 'dock-item run'
      + (с.активно ? ' active' : '') + (с.свёрнуто ? ' свёрнуто' : ''));
    b.dataset.tip = (o.title || o.appId) + ' — окно машины'
      + (с.вовесь ? ' (во весь экран)' : с.развёрнуто ? ' (развёрнуто)'
         : с.свёрнуто ? ' (свёрнуто)' : '');
    b.appendChild(значокЧужого(o));
    /* Как в любой панели задач: нажатие на текущее окно его сворачивает, на
       свёрнутое — достаёт обратно, на чужое — переключает. Раньше здесь
       всегда был «перейти к окну», и свернуть чужую программу из панели
       было нельзя вовсе. */
    b.onclick = () => {
      const действие = с.свёрнуто ? 'restore' : с.активно ? 'minimize' : 'focus';
      Platform.rpc('sys.window', { action:действие, appId:o.appId, title:o.title })
        .then(() => setTimeout(обновиЧужие, 250))
        .catch(e => Dlg.alert('Не удалось переключить окно', String(e.message || e), '⚠️'));
    };
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
    /* Есть ли на экране чужая программа — нужно не только доку. Наш стол
       прячет док, когда открыто окно, но своих окон у Firefox и терминала
       нет: они живут не в этой странице, а у оконного сервера. Пока мы об
       этом не говорили вслух, док оставался висеть поверх них. */
    document.body.classList.toggle('чужие-окна',
      чужиеОкна.some(o => !o.состояние || !o.состояние.свёрнуто));
    if (JSON.stringify(чужиеОкна) !== было) Shell.syncDock();
    вовесьЭкран(!!d.вовесьЭкран);
    занятЭкран(!!d.занятЭкран && !d.вовесьЭкран);
    /* Открылось окно программы, которой в нашем списке ещё нет (её только
       что поставили) — перечитаем список, чтобы у окна появились имя и
       значок. Перечитываем не чаще раза в полминуты. */
    if (чужиеОкна.some(o => !ярлыкОкна(o)) && Date.now() - списокЧитан > 30000){
      списокЧитан = Date.now();
      обновиСписокМашины();
    }
  } catch(e){ /* оконный сервер может и не уметь этого — тогда просто молчим */ }
}

/* Развёрнутое чужое окно: панель прижимается к краю экрана и распрямляется
   во всю ширину — как в других системах. Пока на экране только рабочий
   стол, панель остаётся плавающим островком. */
let былоЗанято = false;
function занятЭкран(да){
  if (да === былоЗанято) return;
  былоЗанято = да;
  document.body.classList.toggle('впритык', да);
  /* Вид панели меняется — значит, изменилась и полоса, которую она держит */
  setTimeout(() => {
    if (Shell.fitDock) Shell.fitDock();
    if (Shell.полосаЗабыть) Shell.полосаЗабыть();
    if (Shell.tellPanelHeight) Shell.tellPanelHeight();
  }, 420);
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
       как в любой другой системе, а не по нашему рисунку. Рядом держим имя
       ярлыка: по нему значок находится, когда строка Icon= подвела. */
    if (о.значок) APPS[id].значок = о.значок;
    if (о.ярлык) APPS[id].ярлык = о.ярлык;
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
      /* И здесь молчим: нажатие уже подтверждено подпрыгнувшим значком. */
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

/* ---------- воткнутая флешка ----------

   Человек втыкает флешку и ждёт, что она появится сама. Система замечает
   новый носитель, говорит о нём и предлагает открыть — одним нажатием. */
let носителиБыли = null;

function wireDrives(){
  if (Platform.mode !== 'native') return;
  проверьНосители();
  setInterval(проверьНосители, 5000);
}

async function проверьНосители(){
  if (document.hidden) return;
  let d;
  try { d = await Platform.rpc('sys.drives'); } catch(e){ return; }
  if (!d || !d['есть']) return;

  const сейчас = (d.list || []).map(н => н.dev);
  if (носителиБыли === null){ носителиБыли = сейчас; return; }   // при запуске не тревожим

  const новые = (d.list || []).filter(н => !носителиБыли.includes(н.dev));
  носителиБыли = сейчас;

  новые.forEach(н => {
    const имя = н['метка'] || н['модель'] || н.dev;
    const гб = н['размер'] > 1073741824
      ? (н['размер'] / 1073741824).toFixed(1) + ' ГБ'
      : Math.round(н['размер'] / 1048576) + ' МБ';
    Shell.toast('Носитель подключён', имя + ' · ' + гб + ' · нажмите, чтобы открыть', '💾', 12000,
      () => открытьНоситель(н));
  });
}

async function открытьНоситель(н){
  try {
    const о = await Platform.rpc('sys.drive', { action:'open', dev:н.dev });
    Shell.toast(н['метка'] || н.dev, 'Открываю ' + (о['где'] || ''), '📂');
  } catch(e){
    Dlg.alert('Не удалось открыть носитель', String(e.message || e), '⚠️');
  }
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
        const d = await Platform.rpc('ui.hear', { с, темы:['уведомление', 'файл-установщик', 'снимок'] });
        с = d.n || с;
        (d.список || []).forEach(м => {
          if (м.тема === 'файл-установщик') спросиПроУстановку(м.что);
          /* Снимок экрана просит оконный сервер: сочетания клавиш ловит он,
             а не мы. Страница получает клавиши, только пока оболочка в
             фокусе, — стоило открыть чужую программу, и Win+Shift+S уходил
             ей. Поэтому сочетание висит на оконном сервере, тот зовёт
             маленькую программу, а она говорит нам сюда. */
          else if (м.тема === 'снимок') сделайСнимок(м.что);
          else покажи(м.что);
        });
      } catch(e){ await new Promise(r => setTimeout(r, 3000)); }
    }
  };

  const покажи = у => {
    if (!у) return;
    const заголовок = String(у.заголовок || у.откуда || 'Уведомление').slice(0, 120);
    const текст = String(у.текст || '').replace(/<[^>]*>/g, '').slice(0, 300);
    Shell.toast(заголовок, текст || String(у.откуда || ''), '🔔', 7000);
  };

  const сделайСнимок = что => {
    const режим = что === 'экран' ? 'экран' : 'область';
    if (window.Снимки && Снимки.сними) Снимки.сними(режим);
    else Shell.toast('Снимок экрана', 'Ножницы ещё не загрузились — попробуйте ещё раз', '✂️');
  };

  слушай();
}

/* ---------- программа, скачанная файлом ----------

   Человек скачал .deb и щёлкнул по нему. Настоящая система в этот миг
   показывает, что за программа к ней просится, и спрашивает согласия —
   ставить чужой файл из интернета молча нельзя. Дальше всё как при
   установке из Магазина: тот же ход работы, те же слова. */
async function спросиПроУстановку(что){
  const путь = что && что.путь;
  if (!путь) return;

  /* Программа одним файлом ничего не ставит: её делают исполняемой и
     запускают. Спросить всё равно нужно — файл чужой. */
  if (/\.appimage$/i.test(путь)){
    const имя = путь.split('/').pop();
    const пуск = await Dlg.confirm('Запустить «' + имя + '»?',
      'Это программа одним файлом: она ничего не ставит в систему, а просто ' +
      'запускается. За то, что внутри, отвечает тот, кто её сделал.',
      { okText:'Запустить', icon:'📦' });
    if (!пуск) return;
    try {
      await Platform.rpc('sys.appimage', { путь });
      Shell.toast(имя, 'Запускаю', '🚀');
    } catch(e){ Dlg.alert('Не удалось запустить', String(e.message || e), '⚠️'); }
    return;
  }

  let про;
  try { про = await Platform.rpc('pkg.file.info', { путь }); }
  catch(e){
    return Dlg.alert('Не удалось прочитать файл',
      String(e.message || e), '⚠️');
  }

  const мб = b => b >= 1048576 ? (b / 1048576).toFixed(1) + ' МБ'
                : b >= 1024 ? Math.round(b / 1024) + ' КБ' : b + ' Б';
  const строки = [
    'Версия: ' + (про['версия'] || 'не указана'),
    'Размер файла: ' + мб(про['размер'] || 0),
    про['место'] ? 'Займёт на диске: около ' + мб(про['место']) : '',
    про['описание'] ? '' : '',
    про['описание'] || ''
  ].filter(Boolean).join('\n');

  const откуда = про['вид'] === 'flatpak'
    ? 'Программу поставит flatpak с ' + (про['откуда'] ? 'указанного в файле хранилища' : 'Flathub') + '.'
    : 'Это программа из файла, а не из Магазина: за то, что внутри, отвечает тот, кто её сделал.';
  const согласен = await Dlg.confirm('Поставить «' + (про['имя'] || 'программу') + '»?',
    строки + '\n\n' + откуда, { okText:'Поставить', icon:'📦' });
  if (!согласен) return;

  try { await Platform.rpc('pkg.file.install', { путь }); }
  catch(e){ return Dlg.alert('Не удалось начать установку', String(e.message || e), '⚠️'); }

  Shell.toast('Установка', 'Ставлю «' + (про['имя'] || 'программу') + '»', '📦');
  следиЗаУстановкой(про['имя'] || 'программа');
}

/* Ход установки показываем так же, как в Магазине: коротко и по делу. */
function следиЗаУстановкой(имя){
  /* Ход установки показывает Магазин: там для этого есть место — полоса,
     название шага и кнопка «Остановить». Раньше вместо него шла россыпь
     всплывающих сообщений: каждое накрывало предыдущее, все исчезали через
     пару секунд, и человек не мог ни понять, сколько осталось, ни
     остановить работу. Полоса при этом стояла в закрытом окне. */
  /* Именно на вкладку программ Linux: там полоса хода, название шага и
     кнопка «Остановить». Раньше открывался каталог наших приложений, и
     человек, нажавший «установить», попадал на страницу, где о его
     установке нет ни слова. */
  try {
    if (typeof WM !== 'undefined' && typeof APPS !== 'undefined' && APPS.store)
      WM.open('store', { 'вкладка':'linux' });
  } catch(e){}

  const часы = setInterval(async () => {
    let ход;
    try { ход = await Platform.rpc('pkg.job'); }
    catch(e){ clearInterval(часы); return; }
    if (!ход) return;
    if (ход.running) return;      /* о ходе рассказывает окно, а не сообщения */
    clearInterval(часы);
    if (ход.ok){
      Shell.toast(имя, 'Программа установлена', '✅', 8000);
      if (typeof обновиСписокМашины === 'function') обновиСписокМашины();
      if (typeof wireRealApps === 'function') wireRealApps();
    } else {
      Dlg.alert('Установка не удалась',
        String(ход.error || 'система не сказала, что пошло не так') +
        (ход.log ? '\n\n' + String(ход.log).slice(-600) : ''), '⚠️');
    }
  }, 1500);
}

/* проверкам нужно звать эту сборку напрямую */
window.wireRealApps = wireRealApps;
window.спросиПроУстановку = спросиПроУстановку;

/* ==========================================================================
   Системный лоток: значки чужих программ

   Программы кладут сюда значки, чтобы сворачиваться и показывать своё меню:
   мессенджеры, качалки, VPN. До сих пор класть их было некуда — приёмника в
   системе не было вовсе, и Telegram при закрытии окна уходил совсем.

   Рисуем их слева от наших кнопок: наши — про саму машину и всегда одни и те
   же, чужие приходят и уходят. Смешивать их в кучу значило бы каждый раз
   переставлять человеку то, к чему он привык.
   ========================================================================== */
function wireTray(){
  const место = document.getElementById('tb-tray');
  if (!место) return;

  let было = '';
  const короб = el('div', 'tray-их');
  место.insertBefore(короб, место.firstChild);

  const нажать = (з, действие, e) => {
    Platform.rpc('sys.tray.нажать', { 'служба':з.service, 'путь':з.path,
      'действие':действие, x:Math.round(e.clientX), y:Math.round(e.clientY) })
      .catch(err => Shell.toast(з.name || 'Значок',
        'Программа не ответила: ' + (err.message || err), '⚠️', 6000));
  };

  const рисуй = значки => {
    короб.innerHTML = '';
    значки.forEach(з => {
      const b = el('button', 'tray-btn tray-чужой');
      b.dataset.tip = з.tooltip || з.name || '';
      /* Пока значок не пришёл, показываем первую букву имени: пустой кружок
         человеку ничего не говорит, а буква уже что-то. */
      b.textContent = (з.name || '?').trim().charAt(0).toUpperCase();
      if (з.iconData){
        const и = el('img'); и.src = з.iconData; и.alt = '';
        b.textContent = ''; b.appendChild(и);
      } else if (з.iconName && typeof поставьЗначок === 'function'){
        поставьЗначок(b, з.iconName, з.iconName);
      }
      /* Левая кнопка — «покажись», правая — меню программы. Так же, как
         в любой другой системе: человеку не нужно учить наши правила. */
      b.onclick = e => нажать(з, з.itemIsMenu ? 'ContextMenu' : 'Activate', e);
      b.oncontextmenu = e => { e.preventDefault(); нажать(з, 'ContextMenu', e); };
      короб.appendChild(b);
    });
  };

  const спроси = async () => {
    if (document.hidden) return;
    try {
      const д = await Platform.rpc('sys.tray');
      const строкой = JSON.stringify(д['значки'] || []);
      if (строкой === было) return;
      было = строкой;
      рисуй(д['значки'] || []);
    } catch(e){ /* лотка может не быть — это не беда, просто пусто */ }
  };

  спроси();
  setInterval(спроси, 2000);
}
