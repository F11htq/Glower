/* ==========================================================================
   Сборка словарей языков: tools/переводы/<код>.txt → js/lang/<код>.js

   Перевод живёт таблицей: строка «номер перевод». Номер — место строки в
   общем списке: сначала словарь оболочки (I18N.DICT.en в js/i18n.js), за
   ним словарь мастера настройки (Setup.T.en в js/setup.js), в том же
   порядке. Ключи — русские строки оттуда же. Так переводчику не нужно
   трогать код: он правит таблицу, сборка раскладывает её по словарям.

   Запуск: node tools/собери-языки.mjs
   Новая строка в английском словаре — значит, новая строка во всех
   таблицах; сборка остановится и скажет, где её не хватает.
   ========================================================================== */
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const корень = join(dirname(fileURLToPath(import.meta.url)), '..');
const вырежи = (файл, начало, приставка, конец) => {
  const т = readFileSync(join(корень, файл), 'utf8');
  const a = т.indexOf(начало);
  const s = т.indexOf(приставка, a) + приставка.length;
  const e = т.indexOf(конец, s);
  return Function('return (' + т.slice(s, e) + '\n})')();
};
const оболочка = вырежи('js/i18n.js', 'DICT:{ en:{', 'DICT:{ en:', '\n  }},');
const мастер = вырежи('js/setup.js', '  T:{', '    en:', '\n    }');
const ключиО = Object.keys(оболочка), ключиМ = Object.keys(мастер);
const всего = ключиО.length + ключиМ.length;

const ЯЗЫКИ = {
  uk:['Українська', 'UK', 'uk-UA'], be:['Беларуская', 'BE', 'be-BY'], kk:['Қазақша', 'KK', 'kk-KZ'],
  de:['Deutsch', 'DE', 'de-DE'], fr:['Français', 'FR', 'fr-FR'], es:['Español', 'ES', 'es-ES'],
  it:['Italiano', 'IT', 'it-IT'], pt:['Português', 'PT', 'pt-BR'], pl:['Polski', 'PL', 'pl-PL'],
  tr:['Türkçe', 'TR', 'tr-TR']
};

let ошибок = 0;
for (const файл of readdirSync(join(корень, 'tools/переводы')).filter(f => f.endsWith('.txt')).sort()){
  const код = файл.replace(/\.txt$/, '');
  const мета = ЯЗЫКИ[код];
  if (!мета){ console.error(`${файл}: язык ${код} не описан в ЯЗЫКИ`); ошибок++; continue; }
  const строки = {};
  readFileSync(join(корень, 'tools/переводы', файл), 'utf8').split('\n').forEach((с, i) => {
    if (!с.trim()) return;
    const м = с.match(/^(\d+) (.+)$/);
    if (!м){ console.error(`${файл}:${i + 1}: строка не по образцу «номер перевод»`); ошибок++; return; }
    строки[+м[1]] = м[2];
  });
  const нет = [...Array(всего).keys()].filter(n => !строки[n]);
  if (нет.length){ console.error(`${файл}: нет строк ${нет.slice(0, 10).join(', ')}${нет.length > 10 ? '…' : ''}`); ошибок++; continue; }
  const о = {}, м = {};
  ключиО.forEach((к, i) => { о[к] = строки[i]; });
  ключиМ.forEach((к, i) => { м[к] = строки[ключиО.length + i]; });
  const текст = `/* ${мета[0]} — словари оболочки и мастера настройки.
   Файл собран из tools/переводы/${файл} сборкой tools/собери-языки.mjs —
   править нужно таблицу, а не этот файл. */
'use strict';
(window.ЯЗЫКИ = window.ЯЗЫКИ || []).push({
  код:${JSON.stringify(код)}, имя:${JSON.stringify(мета[0])}, метка:${JSON.stringify(мета[1])}, локаль:${JSON.stringify(мета[2])},
  оболочка:${JSON.stringify(о, null, 1).replace(/\n/g, '\n  ')},
  мастер:${JSON.stringify(м, null, 1).replace(/\n/g, '\n  ')}
});
`;
  writeFileSync(join(корень, 'js/lang', код + '.js'), текст);
  console.log(`${код}: ${всего} строк`);
}
process.exit(ошибок ? 1 : 0);
