; long_mode.asm — première instruction exécutée en 64-bit.
; Recharge les segments, active SSE (nécessaire car la libcore Rust
; précompilée peut émettre des instructions SSE2), puis appelle rust_main.

global long_mode_start
extern rust_main

section .text
bits 64
long_mode_start:
    mov al, 'H'
    mov dx, 0x3f8
    out dx, al

    ; sélecteurs de segment de données à 0 (ignorés en long mode, mais
    ; on nettoie quand même les registres hérités du mode 32-bit)
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    call enable_sse

    mov al, 'I'
    mov dx, 0x3f8
    out dx, al

    call rust_main

    ; si jamais rust_main revient (ne devrait pas), on boucle en hlt
.hang:
    hlt
    jmp .hang

enable_sse:
    mov rax, cr0
    and ax, 0xFFFB   ; efface CR0.EM (bit 2)
    or ax, 0x2       ; met CR0.MP (bit 1)
    mov cr0, rax
    mov rax, cr4
    or ax, 3 << 9    ; CR4.OSFXSR + CR4.OSXMMEXCPT
    mov cr4, rax
    ret
