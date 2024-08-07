BASE_ADDRESS = 0x80000000;

SECTIONS
{
    . = BASE_ADDRESS;

    kernel_start = .;
    .text : {
        *(.text.entry)
        . = ALIGN(4);
        . = ALIGN(4);
        *(.text.strap)
        *(.text .text.*)
    }
    kernel_end = .;

    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    .bss : {
        *(.bss.stack)
        bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
    
    . = ALIGN(4K);
    bss_end = .;

    /DISCARD/ : {
        *(.eh_frame)
    }

    . = ALIGN(4K);
    heap_start = .;
}