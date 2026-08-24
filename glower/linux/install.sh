#!/bin/sh
# ==========================================================================
#  Установка GlowerOS как сеанса рабочего стола на уже работающий Linux.
#
#  Что делает:
#    1. кладёт оболочку и агент в /usr/share/glower/ui
#    2. ставит скрипт сеанса /usr/bin/glower-session
#    3. регистрирует сеанс в списке дисплей-менеджера
#    4. по желанию включает автозапуск агента через systemd --user
#
#  Что НЕ делает: не трогает загрузчик, не удаляет ваш рабочий стол,
#  не заменяет системные компоненты. Выйти из GlowerOS — выбрать другой
#  сеанс на экране входа.
# ==========================================================================
set -e

PREFIX="${PREFIX:-/usr}"
SHARE="$PREFIX/share/glower/ui"
SRC="$(cd "$(dirname "$0")/.." && pwd)"

say(){ printf '  %s\n' "$1"; }

[ "$(id -u)" = "0" ] || { echo "Запустите с правами root: sudo sh linux/install.sh"; exit 1; }

echo
echo "  Установка GlowerOS"
echo

# --- зависимости ---
MISSING=""
for c in node curl; do command -v "$c" >/dev/null 2>&1 || MISSING="$MISSING $c"; done
if [ -z "$GLOWER_BROWSER" ]; then
  command -v chromium >/dev/null 2>&1 || command -v chromium-browser >/dev/null 2>&1 \
    || command -v google-chrome >/dev/null 2>&1 || MISSING="$MISSING chromium"
fi
if [ -n "$MISSING" ]; then
  say "не хватает:$MISSING"
  say "поставьте их и повторите, например:"
  say "  apt install nodejs curl chromium        (Debian, Ubuntu)"
  say "  dnf install nodejs curl chromium        (Fedora)"
  say "  pacman -S nodejs curl chromium          (Arch)"
  say "если браузер лежит не в PATH, укажите его: GLOWER_BROWSER=/путь/к/chromium"
  exit 1
fi

# --- файлы оболочки ---
install -d "$SHARE"
for d in css js agent assets; do
  [ -d "$SRC/$d" ] && cp -r "$SRC/$d" "$SHARE/"
done
cp "$SRC/index.html" "$SHARE/"
say "оболочка: $SHARE"

# --- сеанс ---
install -d "$PREFIX/bin"
install -m 755 "$SRC/linux/glower-session" "$PREFIX/bin/glower-session"
install -d "$PREFIX/share/wayland-sessions" "$PREFIX/share/xsessions"
install -m 644 "$SRC/linux/glower.desktop" "$PREFIX/share/wayland-sessions/glower.desktop"
install -m 644 "$SRC/linux/glower.desktop" "$PREFIX/share/xsessions/glower.desktop"
say "сеанс: доступен на экране входа как «GlowerOS»"

# --- служба агента (по желанию) ---
install -d "$PREFIX/lib/systemd/user"
install -m 644 "$SRC/linux/glower-agent.service" "$PREFIX/lib/systemd/user/glower-agent.service"
say "служба: systemctl --user enable --now glower-agent"

echo
say "Готово. Выйдите из сеанса и выберите GlowerOS на экране входа."
say "Или просто запустите:  glower-session"
echo
