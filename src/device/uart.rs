use super::CharDev;

pub trait Uart: CharDev {
    fn init();
}

/**
 * NS16550 compatible serial driver.
 */
pub mod ns16550 {
    use super::{CharDev, Uart};

    const UART_ADDR: usize = 0x1000_0000;

    pub struct NS16550;

    impl CharDev for NS16550 {
        fn can_read() -> bool {
            false // unimplemented
        }
        fn can_write() -> bool {
            true
        }
        fn put_char(c: u8) {
            let ptr = UART_ADDR as *mut u8;
            unsafe { ptr.write_volatile(c) };
        }
        fn get_char() -> u8 {
            unimplemented!();
        }
    }

    impl Uart for NS16550 {
        fn init() {
            let ptr = UART_ADDR as *mut u8;
            unsafe {
                ptr.add(3).write_volatile(8);
                // activate FIFO
                ptr.add(2).write_volatile(1);
                // activate interruption
                ptr.add(1).write_volatile(1);
                // set interruption frequency for input
                let divisor: u16 = 592;
                let divisor_low: u8 = (divisor & 0xff).try_into().unwrap();
                let divisor_high: u8 = (divisor >> 8).try_into().unwrap();
                let lcr = ptr.add(3).read_volatile();
                ptr.add(3).write_volatile(lcr | (1 << 7));

                ptr.add(0).write_volatile(divisor_low);
                ptr.add(1).write_volatile(divisor_high);
                ptr.add(3).write_volatile(lcr);
            }
        }
    }
}

/**
 * PL011 serial driver.
 */
pub mod pl011 {
    use super::{CharDev, Uart};

    const UART_ADDR: usize = 0x0900_0000;

    pub struct PL011;

    impl CharDev for PL011 {
        fn can_read() -> bool {
            false // unimplemented
        }
        fn can_write() -> bool {
            true
        }
        fn put_char(c: u8) {
            let ptr = UART_ADDR as *mut u8;
            unsafe { ptr.write_volatile(c) };
        }
        fn get_char() -> u8 {
            unimplemented!();
        }
    }

    impl Uart for PL011 {
        fn init() {}
    }
}
