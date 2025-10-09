MEMORY
{
    RAM : ORIGIN = 0x40000000, LENGTH = 128M
}

ENTRY(_start)

SECTIONS
{
    . = 0x40200000;
    
    .text : {
        KEEP(*(.text._start))
        *(.text .text.*)
    } > RAM
    
    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    } > RAM
    
    .data : ALIGN(8) {
        *(.data .data.*)
    } > RAM
    
    .bss : ALIGN(8) {
        *(.bss .bss.*)
        *(COMMON)
    } > RAM
    
    /DISCARD/ : {
        *(.eh_frame)
        *(.note.*)
    }
}
