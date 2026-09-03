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
QEMU       := qemu-system-x86_64

# `cargo` : le paquet Homebrew `rustup` n'installe pas de proxies dans
# ~/.cargo/bin. On demande son chemin à rustup ; à défaut `cargo` tel
# quel (installation classique via rustup.rs). Il faut aussi que `rustc`
# (juste à côté) soit dans le PATH → CARGO_ENV l'y ajoute au besoin.
RUSTUP_CARGO := $(shell rustup which cargo 2>/dev/null)
CARGO        := $(if $(RUSTUP_CARGO),$(RUSTUP_CARGO),cargo)
CARGO_ENV    := $(if $(RUSTUP_CARGO),PATH="$(dir $(RUSTUP_CARGO)):$$PATH",)

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

.PHONY: all kernel iso run run-fs run-headless run-iso clean

all: $(KERNEL_BIN)

kernel:
	$(CARGO_ENV) $(CARGO) build --release --target $(TARGET)

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
# La souris PS/2 est *relative* : QEMU ne l'envoie au noyau que quand le
# pointeur est "capturé" → clique une fois dans la fenêtre. ⌃⌥G le
# relâche (macOS). En fenêtré c'est plus simple à gérer qu'en plein
# écran, d'où le défaut ci-dessous ; `make run-fs` pour le plein écran.
run: $(KERNEL_BIN)
	$(QEMU) -kernel $(KERNEL_BIN) -vga std -serial stdio $(QEMU_FLAGS)

run-fs: $(KERNEL_BIN)
	$(QEMU) -kernel $(KERNEL_BIN) -vga std -full-screen -serial stdio $(QEMU_FLAGS)

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
	$(CARGO_ENV) $(CARGO) clean
