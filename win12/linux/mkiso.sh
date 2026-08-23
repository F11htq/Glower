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
  cage seatd libgl1 libegl1 libgles2 mesa-vulkan-drivers \
  xserver-xorg-core xserver-xorg-video-vmware xserver-xorg-video-fbdev \
  xserver-xorg-video-vesa xserver-xorg-input-libinput xinit x11-xserver-utils \
  sudo \
  fonts-dejavu-core fonts-noto-color-emoji fonts-noto-core fontconfig \
  nodejs curl ca-certificates \
  parted fdisk dosfstools e2fsprogs squashfs-tools \
  flatpak bubblewrap apparmor \
  grub-pc-bin grub-efi-amd64-bin grub2-common efibootmgr \
  network-manager iproute2 alsa-utils pipewire wireplumber pipewire-pulse \
  wpasupplicant iw rfkill wireless-regdb \
  polkitd dbus \
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

if [ -n "$CHROMIUM" ]; then
  install -d "$ROOTFS/opt/chromium"
  cp -r "$CHROMIUM/." "$ROOTFS/opt/chromium/"
  ln -sf /opt/chromium/chrome "$ROOTFS/usr/bin/chromium"
  echo "  браузер взят из $CHROMIUM"
else
  chroot "$ROOTFS" apt-get install -y --no-install-recommends chromium 2>&1 | tail -1 || \
    echo "  ВНИМАНИЕ: chromium не установлен, укажите --chromium /путь"
fi

# --------------------------------------------------------------------------
step "4/6 автозапуск сеанса"
chroot "$ROOTFS" /bin/bash -e <<'INCHROOT'
id glower >/dev/null 2>&1 || useradd -m -s /bin/bash -G video,audio,input,render,tty glower
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
Environment=XDG_RUNTIME_DIR=/run/user/1000
Environment=XDG_SESSION_TYPE=wayland
# Кавычки обязательны: без них systemd видит только первое слово, а
# остальные ключи молча теряет — оттого в системе не работали ни выключение,
# ни установка, ни сети.
Environment="GLOWER_FLAGS=--system --allow-open --allow-launch --allow-power --allow-install --allow-net --allow-packages"
ExecStartPre=/bin/mkdir -p /run/user/1000
ExecStartPre=/bin/chown glower:glower /run/user/1000
ExecStart=/usr/bin/cage -- /usr/bin/glower-session
# Три неудачи подряд — и systemd останавливается: на экране остаётся
# объяснение, а не мигающий курсор.
Restart=on-failure
RestartSec=3
StartLimitBurst=3
StartLimitIntervalSec=120

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

# Меню не показываем. Носитель делает одно из двух: ставит систему, если её
# ещё нет, или уступает дорогу уже установленной. Выбор нужен только при
# разборе поломок — он под клавишей Esc.
set timeout=0
set timeout_style=hidden
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
menuentry "Установка GlowerOS · безопасная графика" {
  linux /live/vmlinuz boot=live components quiet glower.install=1 nomodeset \
        modprobe.blacklist=bochs,vmwgfx,virtio_gpu,qxl,vboxvideo
  initrd /live/initrd
}
menuentry "Восстановление установленной системы" {
  # Системные файлы кладутся заново, личные остаются на месте.
  linux /live/vmlinuz boot=live components quiet splash glower.install=1 glower.repair=1
  initrd /live/initrd
}
menuentry "Подробный запуск установщика" {
  linux /live/vmlinuz boot=live components glower.install=1
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
