# Makefile — Nothing OS
#
# Cibles utiles :
#   make            construit kernel.bin (image PVH pour `qemu -kernel`)
#   make run        lance dans QEMU avec un écran (VGA)      [macOS + Linux]
#   make run-headless  pareil sans fenêtre, sortie port série [macOS + Linux]
#   make iso        construit l'ISO GRUB (nothing-os.iso)     [Linux]
#   make run-iso    lance l'ISO GRUB dans QEMU                [Linux]
#   make clean      supprime les fichiers générés
#
# Deux façons de démarrer le même noyau :
#  - `qemu -kernel kernel.bin` : QEMU lit la note PVH de l'ELF et entre
#    en 32-bit à `pvh_start`. Aucun GRUB requis → voie par défaut, macOS
#    inclus.
#  - GRUB (cible `iso`) : boot.asm est ré-assemblé avec `-d MULTIBOOT`
#    pour émettre l'en-tête Multiboot 1 (incompatible avec `qemu -kernel`,
#    d'où les deux binaires distincts).

# --- Outils (surchargeables : `make LD=x86_64-elf-ld run`) --------------
# NB : `LD` a une valeur implicite (`ld`) dans make, d'où le `:=` (et pas
# `?=`) ; une affectation en ligne de commande reste prioritaire.
ASM        := nasm
LD         := ld.lld
CARGO      := cargo
QEMU       := qemu-system-x86_64

# Cible Rust : on compile pour x86_64 Linux (core/std précompilés, dispo
# via `rustup target add x86_64-unknown-linux-gnu`) même sur un Mac ARM.
TARGET     := x86_64-unknown-linux-gnu
KERNEL_LIB := target/$(TARGET)/release/libkernel.a

BOOT_DIR   := boot
BUILD_DIR  := build
LINKER     := $(BOOT_DIR)/linker.ld
KERNEL_BIN := kernel.bin
KERNEL_MB  := kernel-mb.bin
ISO        := nothing-os.iso

QEMU_FLAGS := -no-reboot -no-shutdown

.PHONY: all kernel iso run run-headless run-iso clean

all: $(KERNEL_BIN)

kernel:
	$(CARGO) build --release --target $(TARGET)

$(BUILD_DIR)/long_mode.o: $(BOOT_DIR)/long_mode.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 $< -o $@

# Objet boot "PVH" (défaut) et objet boot "Multiboot" (pour l'ISO).
$(BUILD_DIR)/boot.o: $(BOOT_DIR)/boot.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 $< -o $@

$(BUILD_DIR)/boot-mb.o: $(BOOT_DIR)/boot.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 -d MULTIBOOT $< -o $@

$(KERNEL_BIN): kernel $(BUILD_DIR)/boot.o $(BUILD_DIR)/long_mode.o $(LINKER)
	$(LD) -n -T $(LINKER) -o $@ \
		$(BUILD_DIR)/boot.o $(BUILD_DIR)/long_mode.o \
		$(KERNEL_LIB)

$(KERNEL_MB): kernel $(BUILD_DIR)/boot-mb.o $(BUILD_DIR)/long_mode.o $(LINKER)
	$(LD) -n -T $(LINKER) -o $@ \
		$(BUILD_DIR)/boot-mb.o $(BUILD_DIR)/long_mode.o \
		$(KERNEL_LIB)

# --- Lancement direct (PVH, sans GRUB) -------------------------------
run: $(KERNEL_BIN)
	$(QEMU) -kernel $(KERNEL_BIN) -serial stdio $(QEMU_FLAGS)

run-headless: $(KERNEL_BIN)
	$(QEMU) -kernel $(KERNEL_BIN) -display none -serial stdio $(QEMU_FLAGS)

# --- Voie GRUB / ISO (Linux) -----------------------------------------
$(ISO): $(KERNEL_MB)
	@mkdir -p isodir/boot/grub
	cp $(KERNEL_MB) isodir/boot/kernel.bin
	cp $(BOOT_DIR)/grub.cfg isodir/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) isodir

iso: $(ISO)

run-iso: $(ISO)
	$(QEMU) -cdrom $(ISO) $(QEMU_FLAGS)

clean:
	rm -rf $(BUILD_DIR) $(KERNEL_BIN) $(KERNEL_MB) $(ISO) isodir
	$(CARGO) clean
