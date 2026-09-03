; boot.asm — point d'entrée 32-bit (Multiboot1).
; Vérifie multiboot/cpuid/long mode, met en place la pagination pour le
; long mode, puis saute en 64-bit vers long_mode_start (long_mode.asm).

global start
global pvh_start
extern long_mode_start

; --- En-tête Multiboot 1 (chargement par GRUB) ------------------------
; Émis uniquement si on assemble avec `-d MULTIBOOT` (voir Makefile,
; cible `iso`). `qemu -kernel` REFUSE un ELF64 qui contient un en-tête
; multiboot ("Cannot load x86-64 image, give a 32bit one"), d'où le
; conditionnel : le build par défaut passe par la note PVH ci-dessous.
%ifdef MULTIBOOT
MBALIGN  equ 1<<0
MEMINFO  equ 1<<1
FLAGS    equ MBALIGN | MEMINFO
MAGIC    equ 0x1BADB002
CHECKSUM equ -(MAGIC + FLAGS)

section .multiboot
align 4
    dd MAGIC
    dd FLAGS
    dd CHECKSUM
%endif

; --- Note PVH (Xen XEN_ELFNOTE_PHYS32_ENTRY) --------------------------
; Permet à `qemu-system-x86_64 -kernel` de charger directement cet ELF64
; et d'entrer en 32-bit à `pvh_start`, sans GRUB ni ISO.
section .note.Xen note alloc noexec align=4
    dd 4              ; namesz  = longueur de "Xen\0"
    dd 4              ; descsz  = 4 (adresse d'entrée 32-bit)
    dd 18             ; type    = XEN_ELFNOTE_PHYS32_ENTRY
    db "Xen", 0       ; name
    dd pvh_start      ; desc    = point d'entrée

%macro SERIAL_CHAR 1
    push eax
    push edx
    mov al, %1
    mov dx, 0x3f8
    out dx, al
    pop edx
    pop eax
%endmacro

section .text
bits 32

; Entrée PVH (`qemu -kernel`) : ebx pointe sur hvm_start_info, aucun
; "magic" à vérifier. On ne se sert pas des infos du chargeur ici.
pvh_start:
    mov esp, stack_top
    jmp boot_common

; Entrée Multiboot (GRUB) : eax = 0x2BADB002.
start:
    mov esp, stack_top
%ifdef MULTIBOOT
    call check_multiboot
%endif

boot_common:
    SERIAL_CHAR 'A'
    call check_cpuid
    SERIAL_CHAR 'C'
    call check_long_mode
    SERIAL_CHAR 'D'

    call set_up_page_tables
    SERIAL_CHAR 'E'
    call enable_paging
    SERIAL_CHAR 'F'

    lgdt [gdt64.pointer]
    SERIAL_CHAR 'G'
    jmp gdt64.code_segment:long_mode_start

    ; on ne devrait jamais arriver ici
    SERIAL_CHAR 'X'
    hlt

; eax doit contenir 0x2BADB002 au démarrage (magic passé par le bootloader multiboot1)
check_multiboot:
    cmp eax, 0x2BADB002
    jne .no_multiboot
    ret
.no_multiboot:
    mov al, "0"
    jmp error

; on détecte cpuid en essayant de flipper le bit ID (21) d'EFLAGS
check_cpuid:
    pushfd
    pop eax
    mov ecx, eax
    xor eax, 1 << 21
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    cmp eax, ecx
    je .no_cpuid
    ret
.no_cpuid:
    mov al, "1"
    jmp error

check_long_mode:
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode

    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_long_mode

    ret
.no_long_mode:
    mov al, "2"
    jmp error

; identity-map avec des pages de 2 MiB :
;   - le 1er GiB  (0x00000000..) : P3[0] -> p2_table
;   - le 4e  GiB  (0xC0000000..) : P3[3] -> p2_table_hi
;     (là où QEMU place le framebuffer linéaire de la carte VGA std)
set_up_page_tables:
    mov eax, p3_table
    or eax, 0b11 ; present + writable
    mov [p4_table], eax

    mov eax, p2_table
    or eax, 0b11
    mov [p3_table], eax

    mov eax, p2_table_hi
    or eax, 0b11
    mov [p3_table + 3 * 8], eax

    mov ecx, 0
.map_p2_table:
    mov eax, 0x200000
    mul ecx
    or eax, 0b10000011 ; present + writable + huge page (2MiB)
    mov [p2_table + ecx * 8], eax

    inc ecx
    cmp ecx, 512
    jne .map_p2_table

    mov ecx, 0
.map_p2_hi:
    mov eax, 0x200000
    mul ecx
    add eax, 0xC0000000
    or eax, 0b10000011
    mov [p2_table_hi + ecx * 8], eax

    inc ecx
    cmp ecx, 512
    jne .map_p2_hi

    ret

enable_paging:
    mov eax, p4_table
    mov cr3, eax

    ; PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; long mode bit dans EFER
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ; active la pagination
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ret

; affiche "ERR: X" en rouge sur fond blanc à l'écran et arrête le CPU
error:
    mov dword [0xb8000], 0x4f524f45
    mov dword [0xb8004], 0x4f3a4f52
    mov dword [0xb8008], 0x4f204f20
    mov byte  [0xb800a], al
    hlt

section .rodata
gdt64:
    dq 0 ; descripteur nul
.code_segment: equ $ - gdt64
    dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; segment de code 64-bit
.pointer:
    dw $ - gdt64 - 1
    dq gdt64

section .bss
align 4096
p4_table:
    resb 4096
p3_table:
    resb 4096
p2_table:
    resb 4096
p2_table_hi:
    resb 4096
stack_bottom:
    resb 4096 * 16
stack_top:
