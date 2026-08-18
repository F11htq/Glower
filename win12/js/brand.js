/* ==========================================================================
   Имя и версия системы — одно место на весь проект.
   Меняете здесь — меняется везде: загрузка, «О системе», терминал, окна.
   ========================================================================== */
'use strict';

const Brand = {
  name:'Glower OS',
  short:'Glower',
  edition:'Pro',
  version:'1.0',
  build:'1200',
  tagline:'Оболочка на HTML, CSS и JavaScript',
  hostname:'GLOWER-PC',

  full(){ return `${this.name} ${this.edition}`; },
  versionLine(){ return `Версия ${this.version} (сборка ${this.build})`; }
};
window.Brand = Brand;

/* подставляем имя в статическую разметку */
document.title = Brand.name;
addEventListener('DOMContentLoaded', () => {
  const b = document.querySelector('.boot-text');
  if (b) b.textContent = 'Запуск ' + Brand.name;
}, { once:true });
