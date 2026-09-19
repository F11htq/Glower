#!/bin/bash
# ==========================================================================
# Живой стенд: оболочка под настоящим оконным сервером
#
# Две попытки разделить поверхности провалились вслепую. Оболочку было не на
# чем запустить: образ уезжал человеку, а обратно приходило описание экрана.
# Ошибки вроде «селектор не тот» или «панель рисует обои второй раз» так не
# находятся — они не падают, они просто выглядят неправильно.
#
# Здесь оболочка поднимается по-настоящему: labwc без экрана, агент, обе
# поверхности. Снимок делается grim, и его видно.
#
# Имена переменных латиницей: кириллические bash не принимает. Проверено
# дорого и не один раз.
#
#   bash glower/test/стенд.sh          — поднять
#   bash glower/test/стенд.sh снимок   — поднять и снять экран
# ==========================================================================
set -u

RT=${GLOWER_RT:-/tmp/glower-rt}
SRC=$(cd "$(dirname "$0")/.." && pwd)
TST=/tmp/glower-test/glower
PY=${GLOWER_PY:-python3}

# gi собран под определённый Python. Если рядом лежит другой, он перекрывает
# библиотеку, и импорт падает «не найден _gi» — со стороны похоже на то, что
# gi сломан вовсе. Ищем тот, которому библиотека подходит.
if ! "$PY" -c 'import gi' >/dev/null 2>&1; then
  for c in /usr/bin/python3.12 /usr/bin/python3.11 /usr/bin/python3; do
    [ -x "$c" ] || continue
    "$c" -c 'import gi' >/dev/null 2>&1 && { PY=$c; break; }
  done
fi
"$PY" -c 'import gi' >/dev/null 2>&1 || { echo "нет python с gi — ставьте python3-gi"; exit 1; }

command -v labwc >/dev/null || { echo "нет labwc — ставьте labwc"; exit 1; }

mkdir -p "$RT"; chmod 700 "$RT"
export XDG_RUNTIME_DIR="$RT"
export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
export LIBGL_ALWAYS_SOFTWARE=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1

# Один только файл сокета ничего не значит: он остаётся лежать и после того,
# как сервер умер. Стенд на это попался — сокет от прошлого раза был на
# месте, labwc не запускался, а оболочка падала на пустом Gdk.Screen, и со
# стороны это выглядело как её собственная поломка. Спрашиваем про сервер.
if ! pgrep -x labwc >/dev/null 2>&1; then
  rm -f "$RT/wayland-0" "$RT/wayland-0.lock"
fi
if [ ! -S "$RT/wayland-0" ]; then
  # С боевыми настройками, а не с чужими умолчаниями. Стенд нужен, чтобы
  # видеть то же, что увидит человек; сервер с другими настройками — это
  # уже другая система, и проверка на ней стоит меньше, чем кажется.
  nohup labwc -C "$SRC/linux/labwc" > "$RT/labwc.log" 2>&1 &
  for i in $(seq 1 40); do [ -S "$RT/wayland-0" ] && break; sleep 0.25; done
fi
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland
# Размер экрана берём как у машины, на которой всё это проверяется:
# на другом разрешении раскладка панели и дока может лечь иначе.
command -v wlr-randr >/dev/null && \
  wlr-randr --output HEADLESS-1 --custom-mode "${GLOWER_SIZE:-1366x768}" >/dev/null 2>&1
[ -S "$RT/wayland-0" ] || { echo "labwc не поднялся"; tail -5 "$RT/labwc.log"; exit 1; }

pkill -f 'linux/glower-shell' >/dev/null 2>&1
pkill -f 'glower-test/glower/agent' >/dev/null 2>&1
sleep 1

# Копию правим, исходное дерево не трогаем: стенду нужно проскочить
# первоначальную настройку, а это не то, что должно уехать в образ.
rm -rf /tmp/glower-test && mkdir -p /tmp/glower-test
cp -r "$SRC" "$TST"
"$PY" - "$TST/index.html" <<'PY'
import sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()
seed = "<script>try{localStorage.setItem('glower.setup.done','true');}catch(e){}</script>\n"
i = s.index('<script')
open(p, 'w', encoding='utf-8').write(s[:i] + seed + s[i:])
PY

nohup node "$TST/agent/server.mjs" --port 8124 --root "$RT/home" > "$RT/agent.log" 2>&1 &
for i in $(seq 1 40); do curl -sf http://localhost:8124/ >/dev/null && break; sleep 0.3; done

GLOWER_URL=http://localhost:8124/ nohup "$PY" "$SRC/linux/glower-shell" > "$RT/shell.log" 2>&1 &
sleep 14

if pgrep -f 'linux/glower-shell' >/dev/null; then
  echo "оболочка жива"
else
  echo "оболочка упала:"; tail -15 "$RT/shell.log"; exit 1
fi

if [ "${1:-}" = "снимок" ]; then
  out=${2:-$RT/снимок.png}
  grim "$out" && echo "снято: $out"
fi
