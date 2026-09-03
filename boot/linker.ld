ENTRY(start)

/* Deux en-têtes de programme :
 *  - `load` : le seul segment chargeable (on ne sépare pas texte/données,
 *    c'est un noyau jouet).
 *  - `note` : rend la note PVH visible dans un PT_NOTE, ce que
 *    `qemu -kernel` lit pour trouver le point d'entrée 32-bit.
 * La section .note.Xen est rattachée aux DEUX (`:load :note`).
 */
PHDRS
{
    load PT_LOAD FLAGS(7);
    note PT_NOTE FLAGS(4);
}

SECTIONS
{
    . = 1M;

    .note.Xen : { *(.note.Xen) } :load :note

    .boot :
    {
        *(.multiboot)
    } :load

    .text :
    {
        *(.text .text.*)
    } :load

    .rodata :
    {
        *(.rodata .rodata.*)
    } :load

    .data :
    {
        *(.data .data.*)
    } :load

    .bss :
    {
        *(.bss .bss.*)
        *(COMMON)
    } :load
}
