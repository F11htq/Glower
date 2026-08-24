/* ==========================================================================
   Раскладки клавиатуры: индикатор в панели и переключение по Alt+Shift

   Что это делает по-настоящему: пока выбрана русская раскладка, буквы,
   набранные на латинской клавиатуре, превращаются в кириллицу внутри окон
   системы. Разложить клавиатуру самой операционной системы страница,
   разумеется, не может — и не притворяется.
   ========================================================================== */
'use strict';

const Layouts = {
  ALL:{
    ru:{ code:'РУС', name:'Русская', flag:'🇷🇺',
      map:{ q:'й',w:'ц',e:'у',r:'к',t:'е',y:'н',u:'г',i:'ш',o:'щ',p:'з','[':'х',']':'ъ',
            a:'ф',s:'ы',d:'в',f:'а',g:'п',h:'р',j:'о',k:'л',l:'д',';':'ж',"'":'э',
            z:'я',x:'ч',c:'с',v:'м',b:'и',n:'т',m:'ь',',':'б','.':'ю','`':'ё','/':'.' } },
    en:{ code:'ENG', name:'Английская', flag:'🇬🇧', map:null },
    de:{ code:'DEU', name:'Немецкая', flag:'🇩🇪',
      map:{ y:'z', z:'y', ';':'ö', "'":'ä', '[':'ü', '-':'ß' } }
  },

  enabled(){ const l = KV.get('kb.enabled', ['en', 'ru']); return l.filter(x => this.ALL[x]); },
  /* по умолчанию латиница: система не трогает набор, пока её об этом не попросят */
  current(){ const c = KV.get('kb.current', 'en'); return this.ALL[c] ? c : 'en'; },
  def(){ return this.ALL[this.current()]; },

  set(id){
    if (!this.ALL[id]) return;
    KV.set('kb.current', id);
    this.paint();
    OSD && OSD.show(this.ALL[id].flag, 100, this.ALL[id].code);
  },
  next(){
    const l = this.enabled();
    if (l.length < 2) return;
    this.set(l[(l.indexOf(this.current()) + 1) % l.length]);
  },
  toggle(id){
    let l = this.enabled();
    l = l.includes(id) ? l.filter(x => x !== id) : [...l, id];
    if (!l.length) l = ['en'];
    KV.set('kb.enabled', l);
    if (!l.includes(this.current())) this.set(l[0]); else this.paint();
  },

  /* ---------- индикатор в трее ---------- */
  paint(){
    let b = $('#kb-badge');
    if (!b){
      b = el('button', 'tray-btn kb-badge');
      b.id = 'kb-badge';
      const tray = $('#tb-tray');
      if (!tray) return;
      tray.insertBefore(b, $('#tray-clock', tray));
      b.onclick = e => {
        e.stopPropagation();
        const r = b.getBoundingClientRect();
        Shell.ctx(r.left - 60, r.bottom + 8, [
          { t:'Раскладка клавиатуры', head:true },
          ...this.enabled().map(id => ({
            i:this.ALL[id].flag,
            t:this.ALL[id].name + (id === this.current() ? ' ✓' : ''),
            k:this.ALL[id].code,
            f:() => this.set(id)
          })),
          'hr',
          { i:'⚙️', t:'Параметры языка', f:() => WM.open('settings', { section:'time' }) }
        ]);
      };
    }
    const d = this.def();
    b.textContent = d.code;
    b.dataset.tip = `${d.name} · Alt+Shift для переключения`;
  },

  /* ---------- превращение латиницы в выбранную раскладку ---------- */
  install(){
    addEventListener('keydown', e => {
      if (e.key === 'Shift' && e.altKey) { e.preventDefault(); return this.next(); }
      if (e.key === 'Alt' && e.shiftKey) { e.preventDefault(); return this.next(); }

      const map = this.def().map;
      if (!map) return;                                  // латиница — ничего не трогаем
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key.length !== 1) return;

      const t = e.target;
      const editable = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable);
      if (!editable) return;
      if (t.type && ['password','number','color','range','file'].includes(t.type)) return;

      const lower = e.key.toLowerCase();
      const rep = map[lower];
      if (!rep) return;                                  // символа нет в раскладке
      if (!/[a-z\[\];'`,.\/\-]/.test(lower)) return;     // кириллицу с клавиатуры не трогаем

      e.preventDefault();
      const ch = e.key === lower ? rep : rep.toUpperCase();
      if (t.isContentEditable){ document.execCommand('insertText', false, ch); return; }
      const s = t.selectionStart, en = t.selectionEnd;
      t.setRangeText(ch, s, en, 'end');
      t.dispatchEvent(new Event('input', { bubbles:true }));
    }, true);
  }
};
window.Layouts = Layouts;

Layouts.paint();
Layouts.install();
