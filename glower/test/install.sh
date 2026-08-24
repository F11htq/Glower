#!/bin/bash
# ==========================================================================
#  Проверка установки на диск — на настоящем диске, а не на словах.
#
#  Берёт собранный образ, подкладывает его как живой носитель, создаёт
#  пустой диск-файл и ставит на него систему тем же сценарием, что и в
#  живой системе. Потом смотрит, что получилось: разделы, загрузчик, fstab.
#
#  Запуск (нужен root):  sudo bash test/install.sh /var/tmp/glower.iso
# ==========================================================================
set -euo pipefail
ISO="${1:-/var/tmp/glower.iso}"
SRC="$(cd "$(dirname "$0")/.." && pwd)"
IMG="${IMG:-/var/tmp/glower-disk.img}"
MNT_ISO=/mnt/glower-iso
MEDIUM=/run/live/medium

[ "$(id -u)" = 0 ] || { echo "нужен root"; exit 1; }
[ -f "$ISO" ] || { echo "нет образа: $ISO"; exit 1; }

ok=0; bad=0
check(){ if [ "$2" = 0 ]; then echo "  ✅ $1"; ok=$((ok+1)); else echo "  ❌ $1 ${3:-}"; bad=$((bad+1)); fi; }

cleanup(){
  umount -R "$MEDIUM" 2>/dev/null || true
  [ -f "$MEDIUM/live/filesystem.squashfs" ] && rm -f "$MEDIUM/live/filesystem.squashfs" || true
  umount -R /mnt/glower-check 2>/dev/null || true
  umount -R "$MNT_ISO" 2>/dev/null || true
  [ -n "${LOOP:-}" ] && losetup -d "$LOOP" 2>/dev/null || true
}
trap cleanup EXIT

echo
echo "  Проверка установки GlowerOS на диск"
echo

mkdir -p "$MNT_ISO" "$MEDIUM/live"
# ядро умеет iso9660 не везде (в контейнерах — обычно нет), поэтому если
# образ не подключается, достаём из него файл системы напрямую
if mount -o loop,ro "$ISO" "$MNT_ISO" 2>/dev/null; then
  mount --bind "$MNT_ISO" "$MEDIUM"
else
  echo "  образ не подключается — достаю файл системы напрямую"
  rm -f "$MEDIUM/live/filesystem.squashfs"
  xorriso -osirrox on -indev "$ISO" -extract /live/filesystem.squashfs \
    "$MEDIUM/live/filesystem.squashfs" >/dev/null 2>&1
fi
[ -f "$MEDIUM/live/filesystem.squashfs" ] || { echo "в образе нет live/filesystem.squashfs"; exit 1; }

rm -f "$IMG"; truncate -s 14G "$IMG"
LOOP="$(losetup --show -fP "$IMG")"
echo "  пустой диск: $LOOP ($IMG)"

echo "  --- план ---"
"$SRC/linux/glower-install" --disk "$LOOP" --dry-run

echo "  --- установка ---"
"$SRC/linux/glower-install" --disk "$LOOP" --yes --pass проверка --hostname GlowerTest --tz Europe/Moscow \
  | while read -r l; do case "$l" in ШАГ*) echo "  $l" ;; esac; done

partprobe "$LOOP" 2>/dev/null || true
sleep 1

# --- что получилось ---
sfdisk -l "$LOOP" | grep -q 'EFI System' ; check "раздел EFI создан" $?
blkid "${LOOP}p3" | grep -q 'TYPE="ext4"' ; check "система лежит на ext4" $?
dd if="$LOOP" bs=512 count=1 2>/dev/null | grep -qa GRUB ; check "загрузчик BIOS записан в начало диска" $?

mkdir -p /mnt/glower-check
mount "${LOOP}p3" /mnt/glower-check
mount "${LOOP}p2" /mnt/glower-check/boot/efi 2>/dev/null || true

[ -f /mnt/glower-check/boot/grub/grub.cfg ] ; check "меню загрузчика создано" $?
grep -q "UUID=" /mnt/glower-check/etc/fstab ; check "fstab прописан по UUID" $?
grep -q "root=UUID=" /mnt/glower-check/boot/grub/grub.cfg ; check "загрузчик ищет корень по UUID" $?
grep -q "root=/dev/" /mnt/glower-check/boot/grub/grub.cfg && r=1 || r=0
check "имён устройств машины-установщика в меню не осталось" $r
grep -q GlowerTest /mnt/glower-check/etc/hostname ; check "имя машины записано" $?
[ -x /mnt/glower-check/usr/bin/glower-session ] ; check "сеанс оболочки перенесён" $?
[ -f /mnt/glower-check/usr/share/glower/ui/index.html ] ; check "оболочка перенесена" $?
[ -L /mnt/glower-check/etc/localtime ] ; check "часовой пояс задан" $?
grep -q '^glower:[^!*]' /mnt/glower-check/etc/shadow ; check "пароль учётной записи задан" $?
ls /mnt/glower-check/boot/vmlinuz-* >/dev/null 2>&1 ; check "ядро на месте" $?
grep -q "boot=live" /mnt/glower-check/boot/grub/grub.cfg && r=1 || r=0
check "в меню нет живого запуска — система грузится с диска" $r

echo
echo "  Пройдено: $ok · Провалено: $bad"
echo "  Диск остался в $IMG — его можно запустить: qemu-system-x86_64 -m 3072 -drive file=$IMG,format=raw"
echo
[ "$bad" = 0 ]
