#!/usr/bin/env node
/* ==========================================================================
   Помощник проверки запуска программ (зовётся из test/linux.mjs).

   Запускается с урезанным PATH, чтобы проверить обе дороги: короткую через
   flatpak и ручную, без gio. Пишет в stdout одну строку JSON с итогами.
   ========================================================================== */
import { mkdir, writeFile, rm, readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const { apps } = await import(join(root, 'agent/system.mjs'));
const rpc = apps(true);

const где = process.env.GLOWER_TEST_DIR;
const метка = join(где, 'тронуто.txt');
const журнал = process.env.FLATPAK_LOG;
const flatДир = join(homedir(), '.local/share/flatpak/exports/share/applications');
const обычДир = join(homedir(), '.local/share/applications');
await mkdir(flatДир, { recursive:true });
await mkdir(обычДир, { recursive:true });
await rm(метка, { force:true });

await writeFile(join(flatДир, 'glower.test.flatpak.desktop'),
  '[Desktop Entry]\nType=Application\nName=Проверка Flathub\nX-Flatpak=glower.test.flatpak\n' +
  'Exec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=что-то --file-forwarding' +
  ' glower.test.flatpak @@u %U @@\n');
await writeFile(join(обычДир, 'glower-передача.desktop'),
  '[Desktop Entry]\nType=Application\nName=Проверка передачи файлов\nExec=/usr/bin/touch ' + метка + ' @@u %U @@\n');
await writeFile(join(обычДир, 'glower-падает.desktop'),
  '[Desktop Entry]\nType=Application\nName=Проверка падения\nExec=/bin/sh -c "echo нет_такой_библиотеки >&2; exit 3"\n');

const зови = async id => rpc['sys.launch']({ id }).then(r => r, e => ({ ok:false, error:e.message }));
const итог = {
  flatpak:await зови('glower.test.flatpak.desktop'),
  доводы:журнал && existsSync(журнал) ? (await readFile(журнал, 'utf8')).trim() : '',
  передача:await зови('glower-передача.desktop'),
  запущено:false,
  падение:await зови('glower-падает.desktop'),
  нет:await зови('glower-нет-такого.desktop'),
  путь:await зови('../../etc/passwd')
};
итог.запущено = existsSync(метка);
итог.список = (await rpc['sys.apps']()).list.find(a => a.id === 'glower.test.flatpak.desktop') || null;

await rm(join(flatДир, 'glower.test.flatpak.desktop'), { force:true });
await rm(join(обычДир, 'glower-передача.desktop'), { force:true });
await rm(join(обычДир, 'glower-падает.desktop'), { force:true });
console.log(JSON.stringify(итог));
