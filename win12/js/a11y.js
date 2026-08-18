/* ==========================================================================
   Специальные возможности и уведомления — по-настоящему

   Три переключателя раньше только сохранялись в настройки и ни на что не
   влияли. Теперь каждый делает ровно то, что обещает, а чего страница
   сделать не может — там честно написано.
   ========================================================================== */
'use strict';

const A11Y = {
  /* ---------- моно-звук: настоящий сведённый в моно выход ---------- */
  audio:{ recs:[], buses:new WeakMap(), rawConnect:AudioNode.prototype.connect },

  monoOn(){ return KV.get('mono', false); },

  bus(ctx){
    let b = this.audio.buses.get(ctx);
    if (!b){
      b = ctx.createGain();
      b.channelCount = 1;                 // свести стерео в один канал
      b.channelCountMode = 'explicit';
      b.channelInterpretation = 'speakers';
      this.audio.rawConnect.call(b, ctx.destination);   // мимо перехвата, иначе рекурсия
      this.audio.buses.set(ctx, b);
    }
    return b;
  },

  installAudio(){
    if (this._audio) return;
    this._audio = true;
    const A = this;
    const orig = this.audio.rawConnect;
    AudioNode.prototype.connect = function(dest, ...rest){
      const ctx = this.context;
      if (ctx && dest === ctx.destination){
        A.audio.recs.push({ node:this, ctx });
        if (A.monoOn()) return orig.call(this, A.bus(ctx), ...rest);
      }
      return orig.call(this, dest, ...rest);
    };
  },

  /* переключение на лету: разводим уже соединённые узлы заново */
  applyMono(on){
    KV.set('mono', on);
    this.audio.recs = this.audio.recs.filter(r => {
      try {
        const to = on ? this.bus(r.ctx) : r.ctx.destination;
        const from = on ? r.ctx.destination : this.bus(r.ctx);
        try { r.node.disconnect(from); } catch(e){}
        this.audio.rawConnect.call(r.node, to);
        return true;
      } catch(e){ return false; }
    });
  },

  /* ---------- залипание клавиш ---------- */
  latched:{},
  MODS:{ Shift:'shiftKey', Control:'ctrlKey', Alt:'altKey', Meta:'metaKey' },

  stickyOn(){ return KV.get('sticky', false); },

  badge(){
    let b = $('#sticky-osd');
    const keys = Object.keys(this.latched);
    if (!keys.length){ if (b) b.remove(); return; }
    if (!b){ b = el('div', 'sticky-osd glass'); b.id = 'sticky-osd'; document.body.appendChild(b); }
    b.textContent = keys.map(k => ({ Shift:'Shift', Control:'Ctrl', Alt:'Alt', Meta:'Win' })[k]).join(' + ');
  },

  installSticky(){
    if (this._sticky) return;
    this._sticky = true;
    const A = this;

    addEventListener('keydown', e => {
      if (!A.stickyOn() || e.repeat) return;

      /* одиночное нажатие модификатора — залипает до следующей клавиши */
      if (A.MODS[e.key]){
        if (A.latched[e.key]) delete A.latched[e.key];
        else A.latched[e.key] = true;
        A.badge();
        Snd.click();
        return;
      }

      const keys = Object.keys(A.latched);
      if (!keys.length) return;

      const init = { key:e.key, code:e.code, keyCode:e.keyCode, which:e.which,
        bubbles:true, cancelable:true, composed:true,
        shiftKey:e.shiftKey, ctrlKey:e.ctrlKey, altKey:e.altKey, metaKey:e.metaKey };
      keys.forEach(k => { init[A.MODS[k]] = true; delete A.latched[k]; });
      A.badge();

      e.preventDefault();
      e.stopImmediatePropagation();

      /* Shift + буква в поле ввода: набираем прописную сами —
         синтетическое событие текст не вставит */
      const t = e.target;
      const editable = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
      if (editable && init.shiftKey && !init.ctrlKey && !init.altKey && !init.metaKey && e.key.length === 1){
        const up = e.key.toUpperCase();
        if (t.setRangeText){
          const s = t.selectionStart, en = t.selectionEnd;
          t.setRangeText(up, s, en, 'end');
          t.dispatchEvent(new Event('input', { bubbles:true }));
        } else document.execCommand('insertText', false, up);
        return;
      }
      t.dispatchEvent(new KeyboardEvent('keydown', init));
    }, true);
  },

  /* ---------- баннеры уведомлений ---------- */
  installNotif(){
    if (this._notif) return;
    this._notif = true;
    const orig = Shell.toast.bind(Shell);
    Shell.toast = function(title, text, icon, ms){
      /* выключенные уведомления прячут баннер, но запись в центре остаётся */
      if (KV.get('notif', true) === false){
        if (window.Notif && Notif.add){ Notif.add(title, text, icon); Notif.badge(); }
        return;
      }
      return orig(title, text, icon, ms);
    };
  },

  install(){ this.installAudio(); this.installSticky(); this.installNotif(); }
};
window.A11Y = A11Y;
A11Y.install();
