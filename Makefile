# Makefile — Nothing OS
#
# Cibles utiles :
#   make            construit kernel.bin + nothing-os.iso
#   make run        construit puis lance dans QEMU avec un écran (VGA)
#   make run-headless  pareil mais sans affichage, sortie sur le port série
#   make clean      supprime les fichiers générés

ASM        := nasm
LD         := ld
CARGO      := cargo

BOOT_DIR   := boot
BUILD_DIR  := build
KERNEL_BIN := kernel.bin
ISO        := nothing-os.iso

.PHONY: all iso run run-headless clean kernel

all: $(ISO)

kernel:
	$(CARGO) build --release

$(BUILD_DIR)/boot.o: $(BOOT_DIR)/boot.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 $< -o $@

$(BUILD_DIR)/long_mode.o: $(BOOT_DIR)/long_mode.asm
	@mkdir -p $(BUILD_DIR)
	$(ASM) -f elf64 $< -o $@

$(KERNEL_BIN): kernel $(BUILD_DIR)/boot.o $(BUILD_DIR)/long_mode.o
	$(LD) -n -T $(BOOT_DIR)/linker.ld -o $@ \
		$(BUILD_DIR)/boot.o $(BUILD_DIR)/long_mode.o \
		target/release/libkernel.a

$(ISO): $(KERNEL_BIN)
	@mkdir -p isodir/boot/grub
	cp $(KERNEL_BIN) isodir/boot/kernel.bin
	cp $(BOOT_DIR)/grub.cfg isodir/boot/grub/grub.cfg
	grub-mkrescue -o $(ISO) isodir

run: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO)

run-headless: $(ISO)
	qemu-system-x86_64 -cdrom $(ISO) -display none -serial stdio -no-reboot

clean:
	rm -rf $(BUILD_DIR) $(KERNEL_BIN) $(ISO) isodir
	$(CARGO) clean
