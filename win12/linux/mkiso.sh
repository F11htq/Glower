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
  parted dosfstools e2fsprogs squashfs-tools \
  grub-pc-bin grub-efi-amd64-bin grub2-common efibootmgr \
  network-manager iproute2 alsa-utils pipewire wireplumber pipewire-pulse \
  brightnessctl xdg-utils libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 \
  libcups2t64 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 \
  libxrandr2 libgbm1 libpango-1.0-0 libcairo2 libasound2t64 \
  2>&1 | tail -2
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
echo "GlowerOS" > /etc/hostname
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
Environment=GLOWER_FLAGS=--system --allow-open --allow-launch --allow-power --allow-install
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

cat > "$ISO/boot/grub/grub.cfg" <<'GRUB'
set timeout=10
set default=0
menuentry "GlowerOS" {
  linux /live/vmlinuz boot=live components quiet splash
  initrd /live/initrd
}
menuentry "GlowerOS · безопасная графика" {
  # Для машин, где драйвер экрана ядра не заводится: система поднимется
  # через обычный X-сервер и без эффектов, но поднимется.
  linux /live/vmlinuz boot=live components nomodeset \
        modprobe.blacklist=bochs,vmwgfx,virtio_gpu,qxl,vboxvideo
  initrd /live/initrd
}
menuentry "GlowerOS · подробный запуск" {
  linux /live/vmlinuz boot=live components
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
