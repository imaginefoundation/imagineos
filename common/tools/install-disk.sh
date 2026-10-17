#!/bin/sh
set -eu
if [ "$#" -ne 7 ]; then
    printf 'Usage: %s DISK-IMAGE DZIMAGE RAMFS BOOTX64.EFI LIMINE.CONF STARTUP.NSH DFS-FORMATTER\n' "$0" >&2
    exit 2
fi
disk_image=$1
dz_image=$2
ramfs=$3
bootx64=$4
limine_config=$5
startup_script=$6
dfs_formatter=$7
esp_image="${disk_image}.esp"
if [ -e "$disk_image" ] || [ -e "$esp_image" ]; then
    printf 'Refusing to overwrite an existing image or temporary ESP: %s\n' "$disk_image" >&2
    exit 1
fi
for required in "$dz_image" "$ramfs" "$bootx64" "$limine_config" "$startup_script" "$dfs_formatter"; do
    if [ ! -f "$required" ]; then
        printf 'Required installation file not found: %s\n' "$required" >&2
        exit 1
    fi
done
for tool in qemu-img sgdisk mkfs.vfat mmd mcopy dd; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Required installation tool not found: %s\n' "$tool" >&2
        exit 1
    fi
done
cleanup() {
    rm -f -- "$esp_image"
}
trap cleanup EXIT HUP INT TERM
qemu-img create -f raw "$disk_image" 1G
sgdisk --align-end --clear \
    --new=1:2048:+128M --typecode=1:EF00 --change-name=1:IMAGINEOS_ESP \
    --new=2:0:0 --typecode=2:8A7F2C9D-6B31-4E52-9B14-445346530001 \
    --change-name=2:DREAMCORE_DFS "$disk_image"
dd if=/dev/zero of="$esp_image" bs=1M count=128 status=none
mkfs.vfat -F 16 -n IMAGINEOS "$esp_image"
mmd -i "$esp_image" ::/EFI ::/EFI/BOOT ::/boot
mcopy -i "$esp_image" "$bootx64" ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$esp_image" "$limine_config" ::/limine.conf
mcopy -i "$esp_image" "$startup_script" ::/startup.nsh
mcopy -i "$esp_image" "$dz_image" ::/boot/dzImage
mcopy -i "$esp_image" "$ramfs" ::/boot/ramfs.tar
dd if="$esp_image" of="$disk_image" bs=512 seek=2048 conv=notrunc status=none
"$dfs_formatter" "$disk_image" "$ramfs"
printf 'Installed bootable GPT disk image: %s\n' "$disk_image"
printf 'EFI System Partition: 128 MiB FAT16; DFS contains the seeded system root.\n'
