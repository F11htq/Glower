#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Проверка оболочки на движке WebKit — том самом, которым её показывает
система. Раньше оболочку рисовал Chromium, и все проверки шли через него;
теперь важно знать, что она жива и на настоящем движке системы.

Запуск:  python3 test/webkit.py [адрес]
Поднимает окно WebKit (без экрана оно тоже работает), ждёт рабочий стол и
спрашивает у самой страницы, что у неё получилось.
"""
import json
import os
import sys

import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib   # noqa: E402

АДРЕС = sys.argv[1] if len(sys.argv) > 1 else os.environ.get('GLOWER_URL', 'http://localhost:8123/')

ПРОВЕРКИ = [
    ('оболочка отдалась движку WebKit', "!!document.getElementById('desktop')"),
    ('рабочий стол включился', "document.getElementById('desktop').classList.contains('on')"),
    ('панель задач на месте', "!!document.querySelector('.dock-wrap')"),
    ('приложения объявлены', "Object.keys(APPS).length > 10"),
    ('окно открывается', "(function(){ WM.open('files'); return WM.wins.length > 0; })()"),
    ('окно разворачивается', "(function(){ WM.toggleMax(WM.top()); return WM.top().maximized; })()"),
    ('стекло и размытие поддержаны',
     "CSS.supports('backdrop-filter','blur(10px)') || CSS.supports('-webkit-backdrop-filter','blur(10px)')"),
    ('ошибок в коде страницы нет', "window.__ошибки.length === 0"),
]

прошло = []
упало = []
окно = Gtk.Window()
вид = WebKit2.WebView()
вид.get_settings().set_property('enable-developer-extras', True)
окно.add(вид)
окно.set_default_size(1280, 800)
окно.show_all()


def спроси(js):
    """Спрашиваем страницу и ждём ответ, не выходя из цикла событий."""
    итог = {}
    def готово(вид_, задача):
        try:
            значение = вид_.evaluate_javascript_finish(задача)
            итог['ответ'] = значение.to_string() if значение else 'null'
        except GLib.Error as e:
            итог['беда'] = e.message
        Gtk.main_quit()
    вид.evaluate_javascript(js, -1, None, None, None, готово)
    Gtk.main()
    return итог


def подожди(мс):
    """Ждём, не останавливая страницу: цикл событий должен работать."""
    GLib.timeout_add(мс, Gtk.main_quit)
    Gtk.main()


def разбуди():
    """Заставка ждёт нажатия — нажимаем, как это делает человек."""
    спроси("(function(){ if (window.__unlock) __unlock();"
           "dispatchEvent(new KeyboardEvent('keydown', { key:'Enter', bubbles:true })); return true })()")


def шаг():
    разбуди()
    подожди(600)
    разбуди()
    вид.evaluate_javascript(
        "window.__ошибки = window.__ошибки || [];"
        "if (!window.__ловим){ window.__ловим = 1; addEventListener('error', e => __ошибки.push(String(e.message))); }"
        "1", -1, None, None, None, lambda *a: None)

    подожди(1800)
    for имя, js in ПРОВЕРКИ:
        итог = спроси(js)
        ладно = итог.get('ответ') == 'true'
        (прошло if ладно else упало).append((имя, итог))
        print(('  ✅ ' if ладно else '  ❌ ') + имя +
              ('' if ладно else ' — ' + json.dumps(итог, ensure_ascii=False)))

    print('\n  Пройдено: %d · Провалено: %d' % (len(прошло), len(упало)))
    Gtk.main_quit()
    return False


ПОДГОТОВЛЕНО = {'да': False}


def загрузилось(вид_, событие):
    if событие != WebKit2.LoadEvent.FINISHED:
        return
    if not ПОДГОТОВЛЕНО['да']:
        # Мастер первого запуска проверяется отдельно; здесь он только мешал бы.
        ПОДГОТОВЛЕНО['да'] = True
        вид_.evaluate_javascript(
            "try { localStorage.setItem('win12.setup.done','true'); } catch(e){}; location.reload(); 1",
            -1, None, None, None, lambda *a: None)
        return
    # оболочка встаёт не мгновенно: даём ей время на заставку
    GLib.timeout_add(4000, шаг)


вид.connect('load-changed', загрузилось)
вид.load_uri(АДРЕС)
GLib.timeout_add(60000, lambda: (print('  ❌ оболочка не ответила за минуту'), Gtk.main_quit())[1])
Gtk.main()
sys.exit(1 if упало else 0)
