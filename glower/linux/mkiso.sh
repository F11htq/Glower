#!/bin/bash
# ==========================================================================
#  Сборка загрузочного образа GlowerOS
#
#  Делает live-ISO: ядро, systemd, киоск-компоновщик и оболочка, которая
#  стартует сама. Образ грузится с флешки или в виртуальной машине, а из
#  него систему можно установить на диск — мастером внутри самой оболочки.
#
#  Запуск (нужен root):
#     sudo bash linux/mkiso.sh [--out glower.iso] [--chromium /путь/к/chrome-linux]
#
#  Чего образ НЕ делает: не переписывает ядро и драйверы — они берутся у
#  Ubuntu. GlowerOS здесь оболочка и сеанс, а не своя система с нуля.
#  Так же устроены ChromeOS и SteamOS; граница названа честно.
# ==========================================================================
set -euo pipefail

SUITE="${SUITE:-noble}"
MIRROR="${MIRROR:-http://archive.ubuntu.com/ubuntu}"
WORK="${WORK:-/var/tmp/glower-build}"
OUT="glower.iso"
CHROMIUM=""
SRC="$(cd "$(dirname "$0")/.." && pwd)"

while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --chromium) CHROMIUM="$2"; shift 2 ;;
    --work) WORK="$2"; shift 2 ;;
    *) echo "неизвестный ключ: $1"; exit 1 ;;
  esac
done

[ "$(id -u)" = 0 ] || { echo "нужен root: sudo bash linux/mkiso.sh"; exit 1; }
for c in debootstrap mksquashfs xorriso grub-mkrescue mformat; do
  command -v "$c" >/dev/null || { echo "нет $c — поставьте:"; \
    echo "  apt install debootstrap squashfs-tools xorriso grub-pc-bin grub-efi-amd64-bin mtools"; exit 1; }
done

ROOTFS="$WORK/rootfs"
ISO="$WORK/iso"
step(){ printf '\n=== %s\n' "$1"; }

# --------------------------------------------------------------------------
step "1/6 базовая система ($SUITE)"
if [ ! -e "$ROOTFS/.debootstrapped" ]; then
  rm -rf "$ROOTFS"; mkdir -p "$ROOTFS"
  debootstrap --variant=minbase --include=systemd,systemd-sysv,dbus,ca-certificates \
    "$SUITE" "$ROOTFS" "$MIRROR"
  touch "$ROOTFS/.debootstrapped"
fi

# --------------------------------------------------------------------------
step "2/6 ядро, киоск, node"
cat > "$ROOTFS/etc/apt/sources.list" <<EOF
deb $MIRROR $SUITE main universe
deb $MIRROR $SUITE-updates main universe
EOF
mount --bind /dev "$ROOTFS/dev" 2>/dev/null || true
mount -t proc proc "$ROOTFS/proc" 2>/dev/null || true
mount -t sysfs sys "$ROOTFS/sys" 2>/dev/null || true
cleanup(){ umount -l "$ROOTFS/dev" "$ROOTFS/proc" "$ROOTFS/sys" 2>/dev/null || true; }
trap cleanup EXIT

chroot "$ROOTFS" /bin/bash -e <<'INCHROOT'
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends \
  linux-image-generic live-boot live-boot-initramfs-tools initramfs-tools \
  labwc wlrctl foot cage seatd libgl1 libegl1 libgles2 libgl1-mesa-dri mesa-vulkan-drivers \
  wl-clipboard xdg-desktop-portal xdg-desktop-portal-wlr xdg-desktop-portal-gtk \
  thunar gvfs mousepad gnome-calculator eog mpv xfce4-terminal \
  locales language-pack-ru language-pack-gnome-ru \
  python3-gi python3-gi-cairo gir1.2-gtk-3.0 gir1.2-webkit2-4.1 gir1.2-gtklayershell-0.1 \
  xserver-xorg-core xserver-xorg-legacy xserver-xorg-video-vmware xserver-xorg-video-fbdev \
  openbox wmctrl xdotool \
  xserver-xorg-video-vesa xserver-xorg-input-libinput xinit x11-xserver-utils \
  sudo \
  fonts-dejavu-core fonts-noto-color-emoji fonts-noto-core fontconfig \
  nodejs curl ca-certificates \
  parted fdisk dosfstools e2fsprogs squashfs-tools \
  flatpak bubblewrap apparmor \
  grub-pc-bin grub-efi-amd64-bin grub2-common efibootmgr \
  network-manager iproute2 alsa-utils pipewire wireplumber pipewire-pulse \
  wpasupplicant iw rfkill wireless-regdb \
  polkitd dbus dbus-daemon \
  brightnessctl xdg-utils libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 \
  libcups2t64 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 \
  libxrandr2 libgbm1 libpango-1.0-0 libcairo2 libasound2t64 \
  2>&1 | tail -2
# VirtualBox выдаёт себя за видеокарту VMware, но её драйвер под ним не
# работает и сам об этом пишет: «unsupported hypervisor, configuration is
# likely broken». Экран после этого чёрный, и человеку приходится вручную
# выбирать «безопасную графику». Поэтому под VirtualBox драйвер vmwgfx не
# загружаем вовсе — тогда система сразу поднимается через X-сервер. На
# настоящей машине VMware правило не срабатывает и драйвер работает как
# работал.
# Программы из Flathub запускаются в песочнице, а песочнице нужны
# пространства имён пользователя. В Ubuntu 24.04 они по умолчанию закрыты
# для программ без своего профиля AppArmor, и flatpak падает на первом же
# шаге: «ldconfig failed, exit status 256». Для этой системы ограничение
# снимаем — иначе ни одна программа из Flathub не запустится.
cat > /etc/sysctl.d/60-glower-userns.conf <<'SYSCTL'
-kernel.apparmor_restrict_unprivileged_userns=0
-kernel.unprivileged_userns_clone=1
SYSCTL

cat > /etc/modprobe.d/glower-vmwgfx.conf <<'MOD'
install vmwgfx /bin/sh -c 'grep -qi virtualbox /sys/class/dmi/id/product_name /sys/class/dmi/id/sys_vendor 2>/dev/null || exec modprobe --ignore-install vmwgfx "$@"'
MOD

# initramfs пересобираем: в него должны попасть хуки live-boot
update-initramfs -u -k all 2>&1 | tail -1
apt-get clean
rm -rf /var/lib/apt/lists/*
INCHROOT

# --------------------------------------------------------------------------
step "3/6 оболочка GlowerOS"
install -d "$ROOTFS/usr/share/glower/ui"
for d in css js agent assets linux; do
  [ -d "$SRC/$d" ] && cp -r "$SRC/$d" "$ROOTFS/usr/share/glower/ui/"
done
cp "$SRC/index.html" "$ROOTFS/usr/share/glower/ui/"
install -m 755 "$SRC/linux/glower-session" "$ROOTFS/usr/bin/glower-session"
# установка на диск: сценарий лежит в системе и вызывается оболочкой через sudo
install -m 755 "$SRC/linux/glower-install" "$ROOTFS/usr/bin/glower-install"
install -m 755 "$SRC/linux/glower-fix" "$ROOTFS/usr/bin/glower-fix"
# своя программа-оболочка: рабочий стол и полоса под панель задач
install -m 755 "$SRC/linux/glower-shell" "$ROOTFS/usr/bin/glower-shell"
# открыть скачанный файл-установщик: передаёт его оболочке, а та спрашивает
install -m 755 "$SRC/linux/glower-open-package" "$ROOTFS/usr/bin/glower-open-package"
# настройки оконного сервера: оболочка внизу стопки, чужие окна — поверх неё
install -d "$ROOTFS/usr/share/glower/labwc"
install -m 644 "$SRC/linux/labwc/rc.xml" "$ROOTFS/usr/share/glower/labwc/rc.xml"

# --------------------------------------------------------------------------
# Браузер.
#
# В Ubuntu 24.04 пакеты firefox и chromium — пустышки, ведущие к snap: внутрь
# образа они не годятся. Поэтому берём Firefox из официального хранилища
# Mozilla: свободная лицензия, распространять можно, есть русский язык.
#
# Закрытые сборки Google в образ не кладём: раздавать чужой закрытый продукт
# без их согласия нельзя. Кому нужен именно Chrome — поставит его из Магазина
# программ, и скачает его тогда своя машина, а не мы.
# --------------------------------------------------------------------------
# Опрос чужих окон.
#
# Панель задач должна знать, что происходит с окнами других программ:
# развёрнуто ли окно, в работе ли оно, не занимает ли весь экран. wlrctl на
# это отвечает неверно — развёрнутое окно он называет неразвёрнутым, —
# поэтому спрашиваем оконный сервер сами маленькой своей программой.
#
# Средства сборки нужны только здесь и сразу убираются: в готовом образе
# остаётся один небольшой исполняемый файл.
# X-сервер должен запускаться от имени человека, а не только от root: на
# машинах без управления видеокартой это единственный способ показать
# рабочий стол. Без этой настройки Xorg молча отказывается стартовать, и
# человек видит чёрный экран.
cat > "$ROOTFS/etc/X11/Xwrapper.config" <<'XWRAP'
allowed_users=anybody
needs_root_rights=yes
XWRAP

step "3.4/6 опрос чужих окон"
install -d "$ROOTFS/tmp/toplevels"
cp "$SRC/linux/toplevels/glower-toplevels.c" "$ROOTFS/tmp/toplevels/"
cp "$SRC/linux/toplevels/wlr-foreign-toplevel-management-unstable-v1.xml" "$ROOTFS/tmp/toplevels/"
chroot "$ROOTFS" /bin/bash -e <<'INTOP'
export DEBIAN_FRONTEND=noninteractive
# Списки пакетов на этом шаге уже вычищены — читаем их заново
apt-get update -qq
apt-get install -y --no-install-recommends gcc libc6-dev libwayland-dev libwayland-bin 2>&1 | tail -1
cd /tmp/toplevels
wayland-scanner client-header wlr-foreign-toplevel-management-unstable-v1.xml ft.h
wayland-scanner private-code  wlr-foreign-toplevel-management-unstable-v1.xml ft.c
gcc -O2 -o /usr/bin/glower-toplevels glower-toplevels.c ft.c -lwayland-client
chmod 755 /usr/bin/glower-toplevels
apt-get purge -y gcc libc6-dev libwayland-dev libwayland-bin 2>&1 | tail -1
apt-get autoremove -y 2>&1 | tail -1
rm -rf /tmp/toplevels /var/lib/apt/lists/*
INTOP
chroot "$ROOTFS" test -x /usr/bin/glower-toplevels \
  || { echo "  ОШИБКА: опрос чужих окон не собрался"; exit 1; }
echo "  готово: /usr/bin/glower-toplevels"

step "3.5/6 браузер"
chroot "$ROOTFS" /bin/bash -e <<'INBROWSER'
export DEBIAN_FRONTEND=noninteractive
install -d -m 0755 /etc/apt/keyrings
if curl -fsSL https://packages.mozilla.org/apt/repo-signing-key.gpg -o /etc/apt/keyrings/packages.mozilla.org.asc; then
  echo 'deb [signed-by=/etc/apt/keyrings/packages.mozilla.org.asc] https://packages.mozilla.org/apt mozilla main' \
    > /etc/apt/sources.list.d/mozilla.list
  # Пустышка Ubuntu не должна побеждать настоящий пакет Mozilla
  cat > /etc/apt/preferences.d/mozilla <<'PIN'
Package: *
Pin: origin packages.mozilla.org
Pin-Priority: 1000
PIN
  apt-get update -qq
  apt-get install -y --no-install-recommends firefox 2>&1 | tail -1
  apt-get install -y --no-install-recommends firefox-l10n-ru 2>&1 | tail -1 || true
else
  echo "  хранилище Mozilla недоступно"
fi

# Firefox мог не поставиться: нет сети до Mozilla, отказ хранилища. Образ без
# браузера человеку не годится — тогда кладём GNOME Web (epiphany) из обычного
# хранилища Ubuntu. Это настоящий браузер на движке WebKit, свободный, и он
# говорит по-русски. Лучше так, чем «ссылки не работают».
if [ ! -x /usr/bin/firefox ]; then
  rm -f /etc/apt/sources.list.d/mozilla.list /etc/apt/preferences.d/mozilla
  apt-get update -qq
  apt-get install -y --no-install-recommends epiphany-browser 2>&1 | tail -1
fi

if [ -x /usr/bin/firefox ]; then echo "  браузер: Firefox"
elif [ -x /usr/bin/epiphany-browser ]; then echo "  браузер: GNOME Web"
else echo "  ВНИМАНИЕ: браузер в образ не попал"; fi
INBROWSER

# Сборку Chromium кладём, только если её дали ключом: она нужна лишь
# встроенной смотрелке страниц, а на живой машине браузер настоящий.
if [ -n "$CHROMIUM" ]; then
  install -d "$ROOTFS/opt/chromium"
  cp -r "$CHROMIUM/." "$ROOTFS/opt/chromium/"
  ln -sf /opt/chromium/chrome "$ROOTFS/usr/bin/chromium"
  echo "  дополнительно положен Chromium из $CHROMIUM"
fi

# --------------------------------------------------------------------------
# Чем открывать: ссылки, картинки, тексты, музыку и папки.
#
# Без этого нажатая в чужом окне ссылка не откроется нигде, и для человека
# это выглядит как «ссылки не работают».
install -d "$ROOTFS/usr/share/applications"

# Скачанный .deb должен ставиться двойным щелчком, как в любой системе.
cat > "$ROOTFS/usr/share/applications/glower-package.desktop" <<'PKG'
[Desktop Entry]
Type=Application
Name=Установка программы
Name[en]=Install package
Comment=Поставить программу из скачанного файла
Exec=/usr/bin/glower-open-package %f
Icon=system-software-install
Terminal=false
NoDisplay=true
MimeType=application/vnd.debian.binary-package;application/x-deb;application/vnd.flatpak.ref;application/vnd.appimage;application/x-iso9660-appimage;
PKG

# Ссылки отдаём тому браузеру, который на самом деле лежит в образе.
BROWSER_DESKTOP=""
for cand in firefox.desktop org.gnome.Epiphany.desktop epiphany-browser.desktop; do
  if [ -f "$ROOTFS/usr/share/applications/$cand" ]; then BROWSER_DESKTOP="$cand"; break; fi
done
[ -n "$BROWSER_DESKTOP" ] || BROWSER_DESKTOP=firefox.desktop
echo "  ссылки открывает: $BROWSER_DESKTOP"
cat > "$ROOTFS/usr/share/applications/mimeapps.list" <<MIME
[Default Applications]
x-scheme-handler/http=$BROWSER_DESKTOP
x-scheme-handler/https=$BROWSER_DESKTOP
x-scheme-handler/about=$BROWSER_DESKTOP
x-scheme-handler/unknown=$BROWSER_DESKTOP
text/html=$BROWSER_DESKTOP
text/plain=org.xfce.mousepad.desktop
inode/directory=thunar.desktop
image/png=org.gnome.eog.desktop
image/jpeg=org.gnome.eog.desktop
image/gif=org.gnome.eog.desktop
image/webp=org.gnome.eog.desktop
image/svg+xml=org.gnome.eog.desktop
audio/mpeg=mpv.desktop
audio/flac=mpv.desktop
audio/ogg=mpv.desktop
video/mp4=mpv.desktop
video/x-matroska=mpv.desktop
video/webm=mpv.desktop
application/pdf=$BROWSER_DESKTOP
application/vnd.debian.binary-package=glower-package.desktop
application/x-deb=glower-package.desktop
application/vnd.flatpak.ref=glower-package.desktop
application/vnd.appimage=glower-package.desktop
application/x-iso9660-appimage=glower-package.desktop
MIME
chroot "$ROOTFS" update-desktop-database /usr/share/applications >/dev/null 2>&1 || true

# --------------------------------------------------------------------------
step "4/6 автозапуск сеанса"
chroot "$ROOTFS" /bin/bash -e <<'INCHROOT'
id glower >/dev/null 2>&1 || useradd -m -s /bin/bash glower
# Группы задаём отдельно: при повторной сборке пользователь уже есть, и
# строка выше не выполняется — а группы всё равно должны быть на месте.
usermod -aG video,audio,input,render,tty,adm,systemd-journal glower
passwd -d glower
# как в любой живой системе: разбор на месте без пароля
usermod -aG sudo glower 2>/dev/null || true
echo 'glower ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/glower
chmod 440 /etc/sudoers.d/glower
# Ubuntu настраивает NetworkManager так, что он ведёт только Wi-Fi и модемы,
# а проводную сеть оставляет netplan и systemd-networkd, которых в нашей
# системе нет. В итоге кабель есть, а DHCP никто не запрашивает — машина без
# сети. Возвращаем NetworkManager всё железо и просим его самого писать
# адреса DNS: systemd-resolved в образ не входит.
mkdir -p /etc/NetworkManager/conf.d
cat > /etc/NetworkManager/conf.d/10-glower.conf <<'NMCONF'
[keyfile]
unmanaged-devices=none

[main]
dns=default
rc-manager=file
NMCONF

# Flathub: главный источник программ, которых нет в репозиториях Ubuntu —
# Telegram, Firefox, Spotify и прочие. Подключаем сразу, чтобы человеку не
# пришлось об этом знать.
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo 2>/dev/null || true
# Списки Flathub кладём сразу: иначе первый поиск в собранной системе честно
# ничего не найдёт, и человеку придётся догадываться, что источник не готов.
# Если у машины сборки нет сети — не беда, система подключит их сама.
flatpak update --appstream -y --noninteractive 2>/dev/null || true

# Язык системы. Без него настоящие программы говорят по-английски, и
# человек, ради которого всё затевалось, упирается в чужой язык на первом же
# окне. Русский ставим языком по умолчанию, английский остаётся запасным.
sed -i 's/^# *\(ru_RU.UTF-8\)/\1/; s/^# *\(en_US.UTF-8\)/\1/' /etc/locale.gen 2>/dev/null || true
printf 'ru_RU.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\n' >> /etc/locale.gen
locale-gen ru_RU.UTF-8 en_US.UTF-8 >/dev/null 2>&1 || true
cat > /etc/default/locale <<'LOC'
LANG=ru_RU.UTF-8
LANGUAGE=ru:en
LC_ALL=ru_RU.UTF-8
LOC
cat > /etc/locale.conf <<'LOC'
LANG=ru_RU.UTF-8
LANGUAGE=ru:en
LOC

echo "GlowerOS" > /etc/hostname
# файл достался от машины, где собирали образ: адреса чужие, пусть его
# заполняет NetworkManager по настоящему подключению
# Строку ниже NetworkManager перепишет своими адресами, как только появится
# настоящее подключение. Публичный адрес оставлен на случай, если сеть есть,
# а своих адресов DHCP не выдал.
printf '# этот файл заполняет NetworkManager\nnameserver 8.8.8.8\n' > /etc/resolv.conf
printf '127.0.0.1\tlocalhost\n127.0.1.1\tGlowerOS\n' > /etc/hosts
INCHROOT

cat > "$ROOTFS/etc/systemd/system/glower.service" <<'UNIT'
[Unit]
Description=Сеанс GlowerOS
# Три неудачи за две минуты — и systemd останавливается: на экране остаётся
# объяснение, а не бесконечный круг перезапусков.
StartLimitBurst=3
StartLimitIntervalSec=120
After=systemd-user-sessions.service seatd.service getty@tty1.service
Wants=seatd.service
# За первую консоль нельзя бороться вдвоём: если getty успевает раньше,
# он забирает tty1, а сеанс получает SIGHUP и умирает — снаружи это
# выглядит как «система показала обычный текстовый вход».
Conflicts=getty@tty1.service

[Service]
User=glower
PAMName=login
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
StandardInput=tty-force
StandardOutput=journal
# Сеансу не нужны особые права ядра. Одно такое право (cap_wake_alarm)
# доставалось ему по наследству, и из-за него отказывалась работать песочница
# flatpak: она отказывается запускаться, если у позвавшего есть права, но он
# не setuid. Снимаем их явно — программы от этого ничего не теряют.
AmbientCapabilities=
CapabilityBoundingSet=~CAP_WAKE_ALARM
Environment=XDG_RUNTIME_DIR=/run/user/1000
Environment=XDG_SESSION_TYPE=wayland
Environment=LANG=ru_RU.UTF-8
Environment=LANGUAGE=ru:en
Environment=LC_ALL=ru_RU.UTF-8
# Кавычки обязательны: без них systemd видит только первое слово, а
# остальные ключи молча теряет — оттого в системе не работали ни выключение,
# ни установка, ни сети.
Environment="GLOWER_FLAGS=--system --allow-open --allow-launch --allow-power --allow-install --allow-net --allow-packages"
ExecStartPre=/bin/mkdir -p /run/user/1000
ExecStartPre=/bin/chown glower:glower /run/user/1000
# Песочнице flatpak нужны пространства имён пользователя. Файл в /etc/sysctl.d
# делает то же самое при загрузке, но полагаться на один путь не будем:
# знак «+» означает, что строка выполняется с правами root.
ExecStartPre=+/bin/sh -c '/sbin/sysctl -q -w kernel.apparmor_restrict_unprivileged_userns=0 2>/dev/null || true'
# Оконный сервер выбирает сам сеанс: сначала labwc — настоящий, с окнами и
# слоями; если он на этой машине не пошёл, остаются cage и X-сервер. Раньше
# здесь стоял cage, и всё происходило внутри киоска: labwc поднимался вложенно
# и настоящим оконным сервером системы так и не становился.
ExecStart=/usr/bin/glower-session
# Три неудачи подряд — и systemd останавливается: на экране остаётся
# объяснение, а не мигающий курсор.
Restart=on-failure
RestartSec=3
[Install]
WantedBy=multi-user.target
UNIT

# Пакеты, установленные в chroot, свои службы не включают: политика chroot
# это запрещает. Поэтому всё, без чего система не живёт, включаем руками.
# Особенно dbus: без него не стартует NetworkManager, и машина остаётся
# вообще без сети — ни провода, ни Wi-Fi.
chroot "$ROOTFS" /bin/sh -c '
  for unit in dbus.socket dbus.service polkit.service NetworkManager.service \
              wpa_supplicant.service systemd-timesyncd.service; do
    systemctl enable "$unit" >/dev/null 2>&1 || true
  done' || true

chroot "$ROOTFS" systemctl enable glower.service seatd.service >/dev/null 2>&1 || true
# первая консоль принадлежит сеансу; для разбора остаются Ctrl+Alt+F2 и дальше
chroot "$ROOTFS" systemctl mask getty@tty1.service >/dev/null 2>&1 || true
chroot "$ROOTFS" systemctl set-default multi-user.target >/dev/null 2>&1 || true

# --------------------------------------------------------------------------
step "5/6 сжатие файловой системы"
cleanup
rm -rf "$ISO"; mkdir -p "$ISO/live" "$ISO/boot/grub"
KVER="$(basename "$(ls -1 "$ROOTFS"/boot/vmlinuz-* | tail -1)" | sed 's/vmlinuz-//')"
cp "$ROOTFS/boot/vmlinuz-$KVER" "$ISO/live/vmlinuz"
cp "$ROOTFS/boot/initrd.img-$KVER" "$ISO/live/initrd"
mksquashfs "$ROOTFS" "$ISO/live/filesystem.squashfs" \
  -comp zstd -Xcompression-level 12 -noappend \
  -e boot/vmlinuz-\* -e boot/initrd.img-\* -e .debootstrapped -quiet

# Своим встроенным шрифтом GRUB кириллицу не рисует: названия пунктов
# превращаются в обрывки латиницы. Кладём в образ полный шрифт Unicode и
# просим GRUB рисовать меню графически — тогда русские названия читаются.
FONT=""
for f in /usr/share/grub/unicode.pf2 /usr/share/grub/unifont.pf2; do
  [ -f "$f" ] && { FONT="$f"; break; }
done
if [ -n "$FONT" ]; then
  install -d "$ISO/boot/grub/fonts"
  cp "$FONT" "$ISO/boot/grub/fonts/unicode.pf2"
else
  echo "  ВНИМАНИЕ: шрифт GRUB не найден — русские названия в меню загрузки будут нечитаемы"
fi

cat > "$ISO/boot/grub/grub.cfg" <<'GRUB'
if loadfont /boot/grub/fonts/unicode.pf2 ; then
  set gfxmode=auto
  insmod all_video
  insmod gfxterm
  terminal_output gfxterm
fi

# Меню показываем. На новой машине человек просто ждёт восемь секунд, зато на
# старой — где обычная загрузка гаснет чёрным экраном — он видит, что выбор
# есть, и берёт «безопасную графику». Прятать меню значило оставлять людей
# со старыми ноутбуками наедине с погасшим экраном.
set timeout=8
set timeout_style=menu
set default=0

# Уже установленная система важнее носителя: иначе после установки образ,
# оставшийся в приводе, снова лез бы вперёд со своим установщиком.
insmod part_gpt
insmod ext2
insmod search_label
search --no-floppy --label GlowerOS --set=glower_root
if [ -n "$glower_root" ]; then
  if [ -f ($glower_root)/boot/grub/grub.cfg ]; then
    set root=$glower_root
    configfile /boot/grub/grub.cfg
  fi
fi

menuentry "Установка GlowerOS" {
  linux /live/vmlinuz boot=live components quiet splash glower.install=1
  initrd /live/initrd
}
menuentry "Установка GlowerOS · безопасная графика (для старых машин)" {
  # Ядро не берёт на себя управление видеокартой: изображение идёт простым
  # способом, который понимает почти любое железо. Медленнее, зато видно.
  linux /live/vmlinuz boot=live components glower.install=1 nomodeset \
        modprobe.blacklist=bochs,vmwgfx,virtio_gpu,qxl,vboxvideo
  initrd /live/initrd
}
menuentry "Установка GlowerOS · с сообщениями системы" {
  # Ничего не скрываем: если загрузка встанет, на экране будет видно, где.
  linux /live/vmlinuz boot=live components glower.install=1
  initrd /live/initrd
}
menuentry "Восстановление установленной системы" {
  # Системные файлы кладутся заново, личные остаются на месте.
  linux /live/vmlinuz boot=live components quiet splash glower.install=1 glower.repair=1
  initrd /live/initrd
}
GRUB

# --------------------------------------------------------------------------
step "6/6 образ"
# без запасного пути: молча собранный незагрузочный образ хуже явной ошибки
grub-mkrescue -o "$OUT" "$ISO" -- -volid GLOWEROS

# загрузочная запись обязана быть на месте — это проверялось на живой загрузке
if ! xorriso -indev "$OUT" -report_el_torito plain 2>/dev/null | grep -q "El Torito"; then
  echo "  ОШИБКА: в образе нет загрузочной записи — грузиться он не будет"; exit 1
fi

echo
echo "  Готово: $OUT ($(du -h "$OUT" | cut -f1))"
echo "  Проверить:  qemu-system-x86_64 -m 3072 -cdrom $OUT"
echo "  Записать:   sudo dd if=$OUT of=/dev/sdX bs=4M status=progress"
echo
