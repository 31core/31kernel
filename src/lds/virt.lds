BASE_ADDRESS = 0x80000000;

SECTIONS
{
    . = BASE_ADDRESS;

    kernel_start = .;
    .text : {
        *(.text.entry)
        . = ALIGN(4);
        *(.text.strap)
        *(.text .text.*)
    }

    . = ALIGN(4K);
    kernel_end = .;
    rodata_start = .;

    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    rodata_end = .;
    data_start = .;

    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    data_end = .;
    bss_start = .;

    .bss : {
        *(.bss.stack)
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