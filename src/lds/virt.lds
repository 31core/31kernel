VIRT_ADDR = 0xffffffc040000000;
PHY_ADDR = 0x40000000;
ENTRY(_start)

SECTIONS
{
    . = PHY_ADDR;
    .text.entry : AT(PHY_ADDR) {
        *(.text.entry)
    }
    .data.entry : {
        *(.data.entry)
        . = ALIGN(4K);
        _entry_end = .;
    }

    . = VIRT_ADDR + (_entry_end - PHY_ADDR);
    kernel_start = .;
    .text : AT(PHY_ADDR + (_entry_end - PHY_ADDR)) {
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