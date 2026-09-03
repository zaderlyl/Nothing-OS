; boot.asm — point d'entrée 32-bit (Multiboot1).
; Vérifie multiboot/cpuid/long mode, met en place la pagination pour le
; long mode, puis saute en 64-bit vers long_mode_start (long_mode.asm).

global start
extern long_mode_start

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
start:
    mov esp, stack_top
    SERIAL_CHAR 'A'

    call check_multiboot
    SERIAL_CHAR 'B'
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

; identity-map le premier GiB de RAM avec des pages de 2 MiB (P4 -> P3 -> P2)
set_up_page_tables:
    mov eax, p3_table
    or eax, 0b11 ; present + writable
    mov [p4_table], eax

    mov eax, p2_table
    or eax, 0b11
    mov [p3_table], eax

    mov ecx, 0
.map_p2_table:
    mov eax, 0x200000
    mul ecx
    or eax, 0b10000011 ; present + writable + huge page (2MiB)
    mov [p2_table + ecx * 8], eax

    inc ecx
    cmp ecx, 512
    jne .map_p2_table

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
stack_bottom:
    resb 4096 * 16
stack_top:
