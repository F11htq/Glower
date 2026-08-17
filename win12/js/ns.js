/* ==========================================================================
   Пространство имён профиля — загружается раньше всего:
   от него зависит, чьи настройки и файлы прочитает система
   ========================================================================== */
'use strict';

(function ns(){
  const read = (k, d) => { try { const v = localStorage.getItem(k); return v ? JSON.parse(v) : d; } catch(e){ return d; } };

  let list = read('win12.profiles', null);
  if (!Array.isArray(list) || !list.length){
    list = [{ id:'default', name:'Dymensity', emoji:'D', hash:null, created:Date.now() }];
    try { localStorage.setItem('win12.profiles', JSON.stringify(list)); } catch(e){}
  }
  let cur = localStorage.getItem('win12.profile') || 'default';
  if (!list.some(p => p.id === cur)) cur = list[0].id;

  window.__profiles = list;
  window.__profile = cur;
  window.__ns = cur === 'default' ? '' : cur + '.';   // ключи данных профиля
})();
