/* ==========================================================================
   Системное поведение вместо браузерного:
   собственные диалоги, центр уведомлений, восстановление сеанса,
   индикатор громкости и яркости
   ========================================================================== */
'use strict';

/* ==========================================================================
   1. Диалоги системы вместо prompt / confirm / alert
   ========================================================================== */
const Dlg = {
  open(opts){
    return new Promise(resolve => {
      const ov = el('div', 'dlg-ov');
      const box = el('div', 'dlg glass');
      box.innerHTML = `
        <div class="dlg-head">
          <div class="dlg-ico">${opts.icon || 'ℹ️'}</div>
          <div><div class="dlg-t">${esc(opts.title || Brand.name)}</div>
          ${opts.text ? `<div class="dlg-x">${esc(opts.text)}</div>` : ''}</div>
        </div>`;

      /* длинный ответ машины показываем как есть: ровными строками и с прокруткой */
      if (opts.pre){
        const бл = el('pre', 'dlg-pre'); бл.textContent = opts.pre; box.appendChild(бл);
      }

      let input = null, extra = null;
      if (opts.type === 'prompt'){
        input = el('input', 'inp');
        input.value = opts.value || '';
        input.spellcheck = false;
        /* пароль сети вводится скрытым — как везде в системе */
        if (opts.password) input.type = 'password';
        box.appendChild(input);
        /* второе поле — например время напоминания */
        if (opts.extra){
          extra = el('input', 'inp');
          extra.type = opts.extra.type || 'text';
          extra.lang = window.I18N ? I18N.locale() : 'ru-RU';
          extra.value = opts.extra.value || '';
          extra.placeholder = opts.extra.placeholder || '';
          const lab = el('label', 'dlg-extra', `<span>${esc(opts.extra.label || '')}</span>`);
          lab.appendChild(extra);
          box.appendChild(lab);
        }
      }

      const foot = el('div', 'dlg-foot');
      const done = v => { ov.classList.remove('on'); setTimeout(() => ov.remove(), 220); resolve(v); };

      if (opts.type !== 'alert'){
        const cancel = el('button', 'btn', opts.cancelText || 'Отмена');
        cancel.onclick = () => done(opts.type === 'prompt' ? null : false);
        foot.appendChild(cancel);
      }
      const ok = el('button', 'btn pri' + (opts.danger ? ' danger' : ''), opts.okText || 'ОК');
      ok.onclick = () => done(opts.type === 'prompt'
        ? (extra ? { value:input.value || '', extra:extra.value || '' } : (input.value || ''))
        : true);
      foot.appendChild(ok);
      box.appendChild(foot);

      ov.appendChild(box);
      $('#desktop').appendChild(ov);
      requestAnimationFrame(() => ov.classList.add('on'));

      ov.addEventListener('mousedown', e => {
        if (e.target === ov) done(opts.type === 'prompt' ? null : false);
      });
      const keys = e => {
        e.stopPropagation();
        if (e.key === 'Escape'){ document.removeEventListener('keydown', keys, true); done(opts.type === 'prompt' ? null : false); }
        if (e.key === 'Enter' && (!input || document.activeElement === input)){
          document.removeEventListener('keydown', keys, true); ok.click();
        }
      };
      document.addEventListener('keydown', keys, true);
      setTimeout(() => { (input || ok).focus(); if (input) input.select(); }, 120);
    });
  },
  alert(title, text, icon){ return this.open({ type:'alert', title, text, icon }); },
  confirm(title, text, opts = {}){ return this.open({ type:'confirm', title, text, icon:opts.icon || '❓', ...opts }); },
  prompt(title, text, value, icon, opts = {}){
    return this.open(Object.assign({ type:'prompt', title, text, value, icon:icon || '✏️' }, opts)); },
  /* prompt с дополнительным полем: вернёт { value, extra } */
  promptExtra(title, text, value, extra, icon){
    return this.open({ type:'prompt', title, text, value, extra, icon:icon || '✏️' });
  }
};
window.Dlg = Dlg;

/* ==========================================================================
   2. Центр уведомлений: уведомления больше не исчезают бесследно
   ========================================================================== */
const Notif = {
  KEY:'notif.log',
  list(){ return KV.get(this.KEY, []); },
  add(title, text, icon){
    const l = this.list();
    l.unshift({ title, text, icon, ts:Date.now() });
    KV.set(this.KEY, l.slice(0, 60));
    this.paint();
  },
  clear(){ KV.set(this.KEY, []); this.paint(); },
  remove(i){ const l = this.list(); l.splice(i, 1); KV.set(this.KEY, l); this.paint(); },

  when(ts){
    const d = Math.round((Date.now() - ts) / 60000);
    if (d < 1) return 'только что';
    if (d < 60) return d + ' мин назад';
    if (d < 1440) return Math.round(d / 60) + ' ч назад';
    return new Date(ts).toLocaleDateString('ru-RU', { day:'numeric', month:'short' });
  },

  paint(){
    const box = $('#notif-box');
    if (!box) return;
    const l = this.list();
    box.innerHTML = '';
    const head = el('div', 'nc-head');
    head.innerHTML = `<span>Уведомления${l.length ? ' · ' + l.length : ''}</span>`;
    if (l.length){
      const c = el('button', 'mini-btn', 'Очистить все');
      c.onclick = () => this.clear();
      head.appendChild(c);
    }
    box.appendChild(head);

    if (!l.length){
      box.appendChild(el('div', 'nc-empty', '🔔<div>Новых уведомлений нет</div>'));
      return;
    }
    l.forEach((n, i) => {
      const r = el('div', 'nc-item');
      r.innerHTML = `<div class="app-ico">${n.icon || '🔔'}</div>
        <div class="tx"><b>${esc(n.title)}</b>${esc(n.text)}
        <div class="tiny muted" style="margin-top:3px">${this.when(n.ts)}</div></div>
        <button class="nc-x">×</button>`;
      $('.nc-x', r).onclick = e => { e.stopPropagation(); this.remove(i); };
      box.appendChild(r);
    });
  },

  badge(){
    const b = $('#tray-widgets');
    if (b) b.classList.toggle('has-notif', this.list().length > 0);
  }
};
window.Notif = Notif;

/* уведомления попадают в журнал */
(function hookToasts(){
  const orig = Shell.toast.bind(Shell);
  Shell.toast = function(title, text, icon, ms){
    Notif.add(title, text, icon);
    Notif.badge();
    return orig(title, text, icon, ms);
  };
})();

/* журнал живёт над виджетами в той же панели */
(function buildCenter(){
  const w = $('#widgets');
  const box = el('div'); box.id = 'notif-box'; box.className = 'nc';
  const origRender = Shell.renderWidgets.bind(Shell);
  Shell.renderWidgets = function(){
    origRender();
    w.insertBefore(box, w.firstChild.nextSibling);
    Notif.paint();
  };
  Shell.renderWidgets();
  Notif.badge();
})();

/* ==========================================================================
   3. Восстановление сеанса: окна возвращаются после перезагрузки
   ========================================================================== */
const Session = {
  KEY:'session.windows',
  save(){
    if (S.restoreSession === false) return;
    const list = WM.wins.map(w => {
      const r = w.node.getBoundingClientRect();
      return { app:w.appId, x:Math.round(r.left), y:Math.round(r.top),
               w:Math.round(r.width), h:Math.round(r.height),
               max:w.maximized, min:w.minimized, desk:w.desk };
    });
    KV.set(this.KEY, list);
  },
  restore(){
    if (S.restoreSession === false) return;
    const list = KV.get(this.KEY, []);
    if (!list.length) return;
    list.forEach((s, i) => setTimeout(() => {
      if (!APPS[s.app]) return;
      const win = WM.open(s.app, { x:s.x, y:s.y, w:s.w, h:s.h });
      if (!win) return;
      win.desk = s.desk || 0;
      if (s.max) WM.toggleMax(win);
      if (s.min) WM.minimize(win);
    }, 250 + i * 160));
  }
};
window.Session = Session;
addEventListener('beforeunload', () => Session.save());
setInterval(() => Session.save(), 20000);

/* ==========================================================================
   4. Индикатор громкости и яркости, как в настоящих системах
   ========================================================================== */
const OSD = {
  el:null, timer:null,
  show(icon, value, label){
    if (!this.el){
      this.el = el('div', 'osd glass', '<div class="osd-ico"></div><div class="osd-bar"><i></i></div><div class="osd-v"></div>');
      $('#desktop').appendChild(this.el);
    }
    $('.osd-ico', this.el).textContent = icon;
    $('.osd-bar i', this.el).style.width = value + '%';
    $('.osd-v', this.el).textContent = label || (value + '%');
    this.el.classList.add('on');
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.el.classList.remove('on'), 1400);
  }
};
window.OSD = OSD;

(function hookOSD(){
  let lastVol = S.volume, lastBri = S.brightness;
  const origSet = Store.set.bind(Store);
  Store.set = (k, v) => {
    const r = origSet(k, v);
    if (k === 'volume' && v !== lastVol){ lastVol = v; OSD.show(v === 0 ? '🔇' : v < 40 ? '🔉' : '🔊', v); }
    if (k === 'brightness' && v !== lastBri){ lastBri = v; OSD.show('☀️', v); }
    return r;
  };
  // громкость с клавиатуры, как медиа-клавиши
  addEventListener('keydown', e => {
    if (!e.ctrlKey || !e.altKey) return;
    if (e.key === 'ArrowUp'){ e.preventDefault(); Store.set('volume', clamp(S.volume + 5, 0, 100)); }
    if (e.key === 'ArrowDown'){ e.preventDefault(); Store.set('volume', clamp(S.volume - 5, 0, 100)); }
  });
})();

/* ==========================================================================
   5. Замена браузерных диалогов во всей системе
   ========================================================================== */
(function patchDialogs(){

  /* --- Проводник: создание, переименование, удаление, свойства --- */
  APPS.files.use('диалоги', function(win, opts){
    const bar2 = $$('.toolbar', win.body)[1];
    if (!bar2) return;
    const path = () => win.data.path || [];
    const sel = () => $$('.fe-it.sel, .fe-tr.sel', win.body).map(n => n.dataset.name);
    const refresh = () => win.data.refresh && win.data.refresh();

    const nf = $('[data-a="nf"]', bar2), nt = $('[data-a="nt"]', bar2);
    const ren = $('[data-a="ren"]', bar2), del = $('[data-a="del"]', bar2);

    if (nf) nf.onclick = async () => {
      const n = await Dlg.prompt('Создать папку', 'Имя новой папки', 'Новая папка', '📁');
      if (n) { FS.mkdir(path(), FS.uniqueName(path(), n)); refresh(); }
    };
    if (nt) nt.onclick = async () => {
      const n = await Dlg.prompt('Создать файл', 'Имя нового файла', 'Новый.txt', '📄');
      if (n) { FS.write(path(), FS.uniqueName(path(), n), ''); refresh(); }
    };
    if (ren) ren.onclick = async () => {
      const s = sel(); if (s.length !== 1) return;
      const n = await Dlg.prompt('Переименовать', 'Новое имя для «' + s[0] + '»', s[0]);
      if (n && n !== s[0]) { FS.rename(path(), s[0], n); refresh(); }
    };
    if (del) del.onclick = async () => {
      const s = sel(); if (!s.length) return;
      const ok = await Dlg.confirm('Удалить в корзину',
        s.length === 1 ? `«${s[0]}» будет перемещён в корзину.` : `${s.length} элементов будет перемещено в корзину.`,
        { icon:'🗑️', okText:'Удалить', danger:true });
      if (!ok) return;
      s.forEach(n => FS.rm(path(), n));
      refresh(); Shell.renderIcons();
    };
  });

  /* --- Рабочий стол: контекстное меню и значки --- */
  const icons = Shell.renderIcons.bind(Shell);
  Shell.renderIcons = function(){
    icons();
    $$('.di').forEach(n => {
      const it = n._it;
      if (!it || !it.file) return;
      n.onkeydown = async e => {
        if (e.key === 'Enter') it.open();
        else if (e.key === 'F2'){
          const nn = await Dlg.prompt('Переименовать', 'Новое имя', it.n);
          if (nn && nn !== it.n){ FS.rename(['Рабочий стол'], it.n, nn); Shell.renderIcons(); }
        } else if (e.key === 'Delete'){
          if (await Dlg.confirm('Удалить в корзину', `«${it.n}» будет перемещён в корзину.`,
              { icon:'🗑️', okText:'Удалить', danger:true })){
            FS.rm(['Рабочий стол'], it.n); Shell.toast('Удалено', it.n + ' в корзине', '🗑️'); Shell.renderIcons();
          }
        }
      };
    });
  };

  const ctxOrig = Shell.ctx.bind(Shell);
  Shell.ctx = function(x, y, items){
    const wrapped = items.map(it => {
      if (it === 'hr' || !it.f) return it;
      const label = it.t;
      if (/^Создать папку$/.test(label)) return { ...it, f: async () => {
        const n = await Dlg.prompt('Создать папку', 'Имя новой папки', 'Новая папка', '📁');
        if (n){ FS.mkdir(['Рабочий стол'], FS.uniqueName(['Рабочий стол'], n)); Shell.renderIcons(); } } };
      if (/^Создать текстовый файл$/.test(label)) return { ...it, f: async () => {
        const n = await Dlg.prompt('Создать файл', 'Имя нового файла', 'Новый.txt', '📄');
        if (n){ FS.write(['Рабочий стол'], FS.uniqueName(['Рабочий стол'], n), ''); Shell.renderIcons(); } } };
      if (/^Очистить корзину$/.test(label)) return { ...it, f: async () => {
        if (await Dlg.confirm('Очистить корзину', 'Все элементы будут удалены безвозвратно.',
            { icon:'🧹', okText:'Очистить', danger:true })){
          KV.set('trash', []); Shell.renderIcons(); Shell.toast('Корзина', 'Очищена', '🗑️'); } } };
      return it;
    });
    return ctxOrig(x, y, wrapped);
  };

  /* --- Блокнот: сохранение под именем --- */
  const npRender = APPS.notepad.render;
  APPS.notepad.render = function(win, opts){
    npRender.call(this, win, opts);
    const save = $$('.toolbar .btn', win.body).find(b => b.textContent.includes('Сохранить'));
    const area = $('.np-area', win.body);
    if (!save || !area) return;
    save.onclick = async () => {
      const cur = win.titleEl.textContent.split(' — ')[0];
      const unsaved = /не сохранён/.test(win.subEl.textContent);
      let name = cur;
      if (unsaved){
        name = await Dlg.prompt('Сохранить файл', 'Файл будет сохранён в Документы', cur, '💾');
        if (!name) return;
      }
      const path = unsaved ? ['Документы'] : (win.subEl.textContent.replace(/^\//, '').split('/').slice(0, -1));
      FS.write(path.length ? path : ['Документы'], name, area.value);
      win.setTitle(name + ' — Блокнот');
      win.setSub('/' + (path.length ? path.join('/') : 'Документы') + '/' + name);
      Shell.toast('Сохранено', name, '💾');
      Shell.renderIcons();
    };
  };

  /* --- Корзина --- */
  const trashRender = APPS.trash.render;
  APPS.trash.render = function(win){
    trashRender.call(this, win);
    const empty = $$('.toolbar .btn', win.body).find(b => b.textContent.includes('Очистить'));
    if (empty) empty.onclick = async () => {
      if (await Dlg.confirm('Очистить корзину', 'Все элементы будут удалены безвозвратно.',
          { icon:'🧹', okText:'Очистить', danger:true })){
        KV.set('trash', []); win.body.replaceChildren(); trashRender.call(this, win); Shell.renderIcons();
      }
    };
  };

})();

/* ==========================================================================
   Запуск
   ========================================================================== */
setTimeout(() => Session.restore(), 900);
