/* ==========================================================================
   Файловые операции как в настоящем проводнике:
   разрешение конфликтов при вставке, переименование прямо на значке,
   Alt+Enter, перетаскивание файла на значок приложения
   ========================================================================== */
'use strict';

/* ==========================================================================
   Буфер обмена с разрешением конфликтов
   ========================================================================== */
const Clip2 = {
  items:[], cut:false,

  copy(path, names){ this.items = names.map(n => ({ path:path.slice(), name:n })); this.cut = false; },
  take(path, names){ this.copy(path, names); this.cut = true; },
  has(){ return this.items.length > 0; },

  async paste(dest){
    if (!this.items.length) return { done:0, skipped:0 };
    let done = 0, skipped = 0, applyAll = null;

    for (const it of this.items){
      const src = FS.node([...it.path, it.name]);
      if (!src) { skipped++; continue; }
      if (this.cut && String(it.path) === String(dest)) { skipped++; continue; }

      const exists = FS.node([...dest, it.name]);
      let action = 'copy';

      if (exists){
        action = applyAll || await this.askConflict(it.name, dest, this.items.length > 1);
        if (action && action.all){ applyAll = action.choice; action = action.choice; }
      }
      if (action === 'skip'){ skipped++; continue; }
      if (action === null){ skipped++; continue; }

      const clone = JSON.parse(JSON.stringify(src));
      if (action === 'replace'){
        FS.rm(dest, it.name, true);
        const d = FS.node(dest);
        d.children[it.name] = { ...clone, mtime:Date.now() };
        FS.save();
      } else {
        FS.put(dest, clone);            // «оставить оба» — имя получит суффикс
      }
      if (this.cut) FS.rm(it.path, it.name, true);
      done++;
    }
    if (this.cut) this.items = [];
    return { done, skipped };
  },

  /* диалог конфликта, как в Windows */
  askConflict(name, dest, many){
    return new Promise(resolve => {
      const ov = el('div', 'dlg-ov');
      const box = el('div', 'dlg glass lg');
      const old = FS.node([...dest, name]);
      const size = n => n && n.type === 'file' ? new Blob([n.body || '']).size : 0;
      box.innerHTML = `
        <div class="dlg-head">
          <div class="dlg-ico">⚠️</div>
          <div><div class="dlg-t">Файл уже существует</div>
          <div class="dlg-x">В папке «${esc(dest[dest.length - 1] || 'Этот компьютер')}» уже есть «${esc(name)}»
          ${old ? `<br>Существующий: ${size(old)} Б, изменён ${new Date(old.mtime || 0).toLocaleString('ru-RU')}` : ''}</div></div>
        </div>`;
      const opts = el('div', 'conflict');
      const choose = c => { ov.classList.remove('on');
        setTimeout(() => ov.remove(), 200);
        resolve(many && $('input', opts).checked ? { all:true, choice:c } : c); };
      [['replace', '🔁', 'Заменить файл в папке назначения'],
       ['keep',    '➕', 'Сохранить оба файла — новый получит номер'],
       ['skip',    '⏭', 'Пропустить этот файл']
      ].forEach(([c, i, t]) => {
        const b = el('button', 'conflict-b', `<span class="em">${i}</span><span>${t}</span>`);
        b.onclick = () => choose(c);
        opts.appendChild(b);
      });
      if (many){
        const lab = el('label', 'conflict-all');
        lab.innerHTML = `<input type="checkbox"><span>Применить ко всем оставшимся</span>`;
        opts.appendChild(lab);
      }
      box.appendChild(opts);
      const foot = el('div', 'dlg-foot');
      const cancel = el('button', 'btn', 'Отмена');
      cancel.onclick = () => choose(null);
      foot.appendChild(cancel);
      box.appendChild(foot);
      ov.appendChild(box);
      $('#desktop').appendChild(ov);
      requestAnimationFrame(() => ov.classList.add('on'));
      const keys = e => { if (e.key === 'Escape'){ document.removeEventListener('keydown', keys, true); choose(null); } };
      document.addEventListener('keydown', keys, true);
    });
  }
};
window.Clip2 = Clip2;

/* ==========================================================================
   Подключение к Проводнику
   ========================================================================== */
(function wireExplorer(){
  const filesRender = APPS.files.render;

  APPS.files.render = function(win, opts){
    filesRender.call(this, win, opts);

    const bar2 = $$('.toolbar', win.body)[1];
    const status = $('.statusbar', win.body);
    if (!bar2) return;
    const path = () => win.data.path || [];
    const sel = () => $$('.fe-it.sel, .fe-tr.sel', win.body).map(n => n.dataset.name);
    const refresh = () => win.data.refresh && win.data.refresh();

    const report = r => {
      if (!status) return;
      const t = `Вставлено: ${r.done}${r.skipped ? ' · пропущено: ' + r.skipped : ''}`;
      Shell.toast('Проводник', t, '📋');
    };

    const doCopy  = () => { const s = sel(); if (s.length){ Clip2.copy(path(), s);
      Shell.toast('Буфер обмена', `Скопировано: ${s.length}`, '⧉', 1600); } };
    const doCut   = () => { const s = sel(); if (s.length){ Clip2.take(path(), s);
      Shell.toast('Буфер обмена', `Вырезано: ${s.length}`, '✂', 1600); } };
    const doPaste = async () => { if (!Clip2.has()) return;
      const r = await Clip2.paste(path()); refresh(); Shell.renderIcons(); report(r); };

    const btn = a => $(`[data-a="${a}"]`, bar2);
    if (btn('copy'))  btn('copy').onclick  = doCopy;
    if (btn('cut'))   btn('cut').onclick   = doCut;
    if (btn('paste')) btn('paste').onclick = doPaste;

    /* переименование прямо на значке */
    const inlineRename = () => {
      const s = sel(); if (s.length !== 1) return;
      const node = $(`.fe-it[data-name="${CSS.escape(s[0])}"], .fe-tr[data-name="${CSS.escape(s[0])}"]`, win.body);
      if (!node) return;
      const label = $('.nm', node) || $('.c1', node);
      if (!label) return;
      const before = s[0];
      const ed = el('input', 'inline-edit');
      ed.value = before;
      label.replaceWith(ed);
      ed.focus();
      const dot = before.lastIndexOf('.');
      ed.setSelectionRange(0, dot > 0 ? dot : before.length);
      let closed = false;
      const finish = ok => {
        if (closed) return; closed = true;
        const v = ed.value.trim();
        if (ok && v && v !== before && !FS.node([...path(), v])) FS.rename(path(), before, v);
        else if (ok && v && FS.node([...path(), v]) && v !== before)
          Shell.toast('Проводник', 'Имя уже занято', '⚠️');
        refresh(); Shell.renderIcons();
      };
      ed.onblur = () => finish(true);
      ed.onkeydown = e => {
        e.stopPropagation();
        if (e.key === 'Enter') finish(true);
        if (e.key === 'Escape'){ closed = true; refresh(); }
      };
    };
    if (btn('ren')) btn('ren').onclick = inlineRename;

    /* клавиатура: перехватываем до внутреннего обработчика */
    win.node.addEventListener('keydown', async e => {
      if (/INPUT|TEXTAREA/.test(e.target.tagName)) return;
      const k = e.key.toLowerCase();
      if ((e.ctrlKey || e.metaKey) && k === 'c'){ e.stopImmediatePropagation(); doCopy(); }
      else if ((e.ctrlKey || e.metaKey) && k === 'x'){ e.stopImmediatePropagation(); doCut(); }
      else if ((e.ctrlKey || e.metaKey) && k === 'v'){ e.stopImmediatePropagation(); e.preventDefault(); doPaste(); }
      else if (e.key === 'F2'){ e.stopImmediatePropagation(); e.preventDefault(); inlineRename(); }
      else if (e.altKey && e.key === 'Enter'){
        e.preventDefault(); e.stopImmediatePropagation();
        const s = sel(); if (s.length !== 1) return;
        const node = FS.node([...path(), s[0]]);
        if (node) WM.open('props', { node, path:path() });
      }
    }, true);

    /* перетаскивание наружу — запоминаем источник для дока */
    win.body.addEventListener('dragstart', e => {
      const it = e.target.closest('[data-name]');
      if (!it) return;
      window.__dragFiles = { path:path().slice(), names:sel().length ? sel() : [it.dataset.name] };
    }, true);
    win.body.addEventListener('dragend', () => { window.__dragFiles = null; }, true);
  };
})();

/* ==========================================================================
   Перетаскивание файла на значок приложения в доке
   ========================================================================== */
(function dropOnDock(){
  const dock = $('#dock');
  dock.addEventListener('dragover', e => {
    const b = e.target.closest('.dock-item');
    if (!b || !window.__dragFiles) return;
    e.preventDefault();
    b.classList.add('drop-target');
  });
  dock.addEventListener('dragleave', e => {
    const b = e.target.closest('.dock-item');
    if (b) b.classList.remove('drop-target');
  });
  dock.addEventListener('drop', e => {
    const b = e.target.closest('.dock-item');
    if (!b) return;
    b.classList.remove('drop-target');
    const d = window.__dragFiles;
    if (!d || !b.dataset.app) return;
    e.preventDefault(); e.stopPropagation();
    d.names.forEach(n => {
      const node = FS.node([...d.path, n]);
      if (node && node.type === 'file') Assoc.open(node, d.path, b.dataset.app);
    });
    window.__dragFiles = null;
  });
})();

/* ==========================================================================
   Переименование прямо на рабочем столе
   ========================================================================== */
(function desktopRename(){
  const icons = Shell.renderIcons.bind(Shell);
  Shell.renderIcons = function(){
    icons();
    $$('.di').forEach(n => {
      const it = n._it;
      if (!it || !it.file) return;
      const rename = () => {
        const label = $('.lbl', n);
        if (!label || $('.inline-edit', n)) return;
        const before = it.n;
        const ed = el('input', 'inline-edit');
        ed.value = before;
        label.replaceWith(ed);
        ed.focus();
        const dot = before.lastIndexOf('.');
        ed.setSelectionRange(0, dot > 0 ? dot : before.length);
        let closed = false;
        const finish = ok => {
          if (closed) return; closed = true;
          const v = ed.value.trim();
          if (ok && v && v !== before && !FS.node(['Рабочий стол', v])){
            FS.rename(['Рабочий стол'], before, v);
            const P = KV.get('iconPos', {});
            if (P[before]){ P[v] = P[before]; delete P[before]; KV.set('iconPos', P); }
          }
          Shell.renderIcons();
        };
        ed.onblur = () => finish(true);
        ed.onkeydown = e => { e.stopPropagation();
          if (e.key === 'Enter') finish(true);
          if (e.key === 'Escape'){ closed = true; Shell.renderIcons(); } };
      };
      n.addEventListener('keydown', e => {
        if (e.key === 'F2'){ e.preventDefault(); e.stopImmediatePropagation(); rename(); }
        if (e.altKey && e.key === 'Enter'){ e.preventDefault();
          WM.open('props', { node:it.file, path:['Рабочий стол'] }); }
      }, true);
      n._rename = rename;
    });
  };
  Shell.renderIcons();
})();
