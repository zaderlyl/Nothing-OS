# Makefile — Nothing OS
#
# Cibles utiles :
#   make            construit kernel.bin
#   make run        lance dans QEMU avec un écran (VGA)      [macOS + Linux]
#   make run-headless  pareil sans fenêtre, sortie port série [macOS + Linux]
#   make iso        construit l'ISO GRUB (nothing-os.iso)     [Linux]
#   make run-iso    lance l'ISO GRUB dans QEMU                [Linux]
#   make clean      supprime les fichiers générés
#
# QEMU sait charger directement une image Multiboot avec `-kernel`, sans
# GRUB ni grub-mkrescue : c'est la voie recommandée sur macOS (voir README).

# --- Outils (surchargeables : `make LD=x86_64-elf-ld run`) --------------
ASM        := nasm
LD         ?= ld.lld
CARGO      := cargo
QEMU       := qemu-system-x86_64

# Cible Rust : on compile pour x86_64 Linux (core/std précompilés, dispo
# via `rustup target add x86_64-unknown-linux-gnu`) même sur un Mac ARM.
TARGET     := x86_64-unknown-linux-gnu
KERNEL_LIB := target/$(TARGET)/release/libkernel.a

BOOT_DIR   := boot
BUILD_DIR  := build
KERNEL_BIN := kernel.bin
ISO        := nothing-os.iso

QEMU_FLAGS := -no-reboot -no-shutdown

.PHONY: all kernel iso run run-headless run-iso clean

all: $(KERNEL_BIN)

kernel:
	$(CARGO) build --release --target $(TARGET)

$(BUILD_DIR)/boot.o: $(BOOT_DIR)/boot.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 $< -o $@

$(BUILD_DIR)/long_mode.o: $(BOOT_DIR)/long_mode.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 $< -o $@

$(KERNEL_BIN): kernel $(BUILD_DIR)/boot.o $(BUILD_DIR)/long_mode.o
	$(LD) -n -T $(BOOT_DIR)/linker.ld -o $@ \
		$(BUILD_DIR)/boot.o $(BUILD_DIR)/long_mode.o \
		$(KERNEL_LIB)

# --- Lancement direct (Multiboot, sans GRUB) ---------------------------
run: $(KERNEL_BIN)
	$(QEMU) -kernel $(KERNEL_BIN) -serial stdio $(QEMU_FLAGS)

run-headless: $(KERNEL_BIN)
	$(QEMU) -kernel $(KERNEL_BIN) -display none -serial stdio $(QEMU_FLAGS)

# --- Voie GRUB / ISO (Linux) -----------------------------------------
$(ISO): $(KERNEL_BIN)
	@mkdir -p isodir/boot/grub
	cp $(KERNEL_BIN) isodir/boot/kernel.bin
	cp $(BOOT_DIR)/grub.cfg isodir/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) isodir

iso: $(ISO)

run-iso: $(ISO)
	$(QEMU) -cdrom $(ISO) $(QEMU_FLAGS)

clean:
	rm -rf $(BUILD_DIR) $(KERNEL_BIN) $(ISO) isodir
	$(CARGO) clean
