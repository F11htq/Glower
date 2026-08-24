/* ==========================================================================
   Слой платформы: подключение к системному агенту

   Без агента оболочка работает как раньше — файлы живут в браузере.
   С агентом та же оболочка работает с настоящим диском: дерево читается
   с машины, а любые изменения тут же уходят обратно на диск.
   ========================================================================== */
'use strict';

const Platform = {
  mode:'browser',            // browser | native
  url:null,
  info:null,
  pending:0,
  rev:0,

  /* адрес агента: тот же origin (агент раздаёт оболочку) или ?agent=... */
  candidates(){
    const q = new URLSearchParams(location.search).get('agent');
    const list = [];
    if (q) list.push(q.replace(/\/$/, ''));
    if (location.protocol.startsWith('http')) list.push(location.origin);
    return list;
  },

  async rpc(method, params, base){
    const url = (base || this.url) + '/rpc';
    const r = await fetch(url, {
      method:'POST', headers:{ 'Content-Type':'application/json' },
      body:JSON.stringify({ method, params })
    });
    const j = await r.json();
    if (!j.ok) throw new Error(j.error || 'ошибка агента');
    return j.result;
  },

  async connect(){
    for (const base of this.candidates()){
      try {
        const info = await this.rpc('ping', {}, base);
        if (info && String(info.agent) === 'glower-agent'){
          this.url = base; this.info = info; this.mode = 'native';
          return true;
        }
      } catch(e){}
    }
    return false;
  },

  /* дерево с диска заменяет виртуальную ФС */
  async mount(){
    const { root } = await this.rpc('fs.tree');
    try { this.rev = (await this.rpc('fs.rev')).rev; } catch(e){}
    FS.root = root;
    FS.save();
    this.patchFS();
    Shell.renderShell();
    Shell.renderIcons();
    WM.wins.filter(w => w.appId === 'files' && w.data.refresh).forEach(w => w.data.refresh());
  },

  /* изменения в оболочке уходят на диск */
  patchFS(){
    if (this._patched) return;
    this._patched = true;
    const P = this;
    const push = (method, params) => {
      P.pending++; P.badge();
      P.rpc(method, params)
        .catch(e => Shell.toast('Агент', 'Не удалось записать на диск: ' + e.message, '⚠️'))
        .finally(() => {
          P.pending--; P.badge();
          P.rpc('fs.rev').then(r => { P.rev = r.rev; }).catch(() => {});
        });
    };

    const write = FS.write.bind(FS);
    FS.write = function(path, name, body){
      const r = write(path, name, body);
      if (r !== false) push('fs.write', { path:[...path, name], body });
      return r;
    };
    const mkdir = FS.mkdir.bind(FS);
    FS.mkdir = function(path, name){
      const r = mkdir(path, name);
      if (r !== false) push('fs.mkdir', { path:[...path, name] });
      return r;
    };
    const rm = FS.rm.bind(FS);
    FS.rm = function(path, name, permanent){
      const r = rm(path, name, permanent);
      if (r !== false) push('fs.remove', { path:[...path, name] });
      return r;
    };
    const rename = FS.rename.bind(FS);
    FS.rename = function(path, name, nn){
      const r = rename(path, name, nn);
      if (r !== false) push('fs.rename', { from:[...path, name], to:[...path, nn] });
      return r;
    };
    const put = FS.put.bind(FS);
    FS.put = function(path, node){
      const name = put(path, node);
      if (name){
        if (node.img) push('fs.writeDataUrl', { path:[...path, name], dataUrl:node.img });
        else push('fs.write', { path:[...path, name], body:node.body || '' });
      }
      return name;
    };
  },

  /* папку правили снаружи — подтягиваем изменения сами */
  watch(){
    if (this._watching) return;
    this._watching = true;
    setInterval(async () => {
      if (this.mode !== 'native' || this.pending > 0 || document.hidden) return;
      try {
        const { rev } = await this.rpc('fs.rev');
        if (rev === this.rev) return;
        this.rev = rev;
        await this.mount();
      } catch(e){}
    }, 2500);
  },

  /* открыть файл в настоящей системе (если агент запущен с --allow-open) */
  async open(path){
    if (this.mode !== 'native') throw new Error('агент не запущен');
    return this.rpc('sys.open', { path });
  },

  /* индикатор в трее */
  badge(){
    let b = $('#agent-badge');
    if (this.mode !== 'native'){ if (b) b.remove(); return; }
    if (!b){
      b = el('button', 'tray-btn agent-badge');
      b.id = 'agent-badge';
      b.innerHTML = `<svg viewBox="0 0 24 24" class="ic"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/></svg>`;
      b.onclick = () => WM.open('settings', { section:'system' });
      const tray = $('#tb-tray');
      if (tray) tray.insertBefore(b, tray.firstChild);
    }
    b.classList.toggle('busy', this.pending > 0);
    b.dataset.tip = `Настоящий диск · ${this.info.root}` + (this.pending ? ` · запись…` : '');
  },

  describe(){
    if (this.mode !== 'native')
      return { title:'Автономный режим', text:'Файлы хранятся в браузере. Запустите агент — и оболочка будет работать с настоящим диском.', ok:false };
    return { title:'Подключено к системе', ok:true,
      text:`Агент ${this.info.version} · ${this.info.platform} · Node ${this.info.node}\nРабочая папка: ${this.info.root}` };
  }
};
window.Platform = Platform;

/* ---------- подключение при запуске ---------- */
(async function boot(){
  const ok = await Platform.connect();
  if (!ok) return;
  document.body.classList.add('native');
  try {
    await Platform.mount();
    Platform.badge();
    Platform.watch();
    Shell.toast('Система', 'Подключено к настоящему диску: ' + Platform.info.root, '🖴', 5000);
  } catch(e){
    Shell.toast('Агент', 'Не удалось прочитать диск: ' + e.message, '⚠️');
  }
})();

/* ---------- команда agent в терминале ---------- */
(function termCommand(){
  const render = APPS.term.render;
  APPS.term.render = function(win){
    render.call(this, win);
    const out = $('.term', win.body);
    const inp = $('.term-in input', win.body);
    if (!out || !inp) return;
    const print = t => { const l = el('div', 'term-line'); l.innerHTML = t;
      out.insertBefore(l, $('.term-in', out)); out.scrollTop = out.scrollHeight; };

    inp.addEventListener('keydown', async e => {
      if (e.key !== 'Enter') return;
      const line = inp.value.trim();
      if (!/^agent\b/.test(line)) return;
      e.stopImmediatePropagation();
      inp.value = '';
      print(`<span class="pr">${esc($('.pr', out).textContent)}</span> ${esc(line)}`);
      if (Platform.mode !== 'native'){
        print('Агент не запущен. Файлы хранятся в браузере.', 'er');
        print('Запустите:  node agent/server.mjs   и откройте http://localhost:8123');
        return;
      }
      const sub = line.slice(5).trim();
      if (sub.startsWith('open ')){
        const rel = sub.slice(5).trim().split('/').filter(Boolean);
        if (!Platform.info.canOpen){
          print('Открытие файлов в системе выключено. Запустите агент с ключом --allow-open.', 'er');
          return;
        }
        try { const r = await Platform.open(rel); print('Открыто в системе: ' + esc(r.opened)); }
        catch(err){ print('open: ' + esc(err.message), 'er'); }
        return;
      }
      const d = Platform.describe();
      print(`<b class="inf">${d.title}</b>\n${esc(d.text)}`);
      try {
        const s = await Platform.rpc('sys.info');
        print(`  Машина:  ${esc(s.host)} (${esc(s.user)})\n  Система: ${esc(s.platform)} ${esc(s.release)}\n` +
              `  Ядер:    ${s.cpus}\n  Память:  ${(s.memFree / 1073741824).toFixed(1)} из ${(s.mem / 1073741824).toFixed(1)} ГБ свободно\n` +
              `  Аптайм:  ${Math.floor(s.uptime / 3600)} ч ${Math.round(s.uptime % 3600 / 60)} мин`);
      } catch(err){ print('sys.info: ' + esc(err.message), 'er'); }
    }, true);
  };
})();
