#![no_std]
#![feature(format_args_nl)]

use klocks::SpinNoIrqMutex;
use qemu_uart::UART0;

use core::fmt::{Arguments, Result, Write};

/// 标准输出
pub struct Stdout(());

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result {
        UART0.print(s.as_bytes());
        Ok(())
    }
}

pub static STDOUT: SpinNoIrqMutex<Stdout> = SpinNoIrqMutex::new(Stdout(()));

/// 输出到 stdout
#[inline]
fn stdout_puts(fmt: Arguments<'_>) {
    STDOUT.lock().write_fmt(fmt).unwrap();
}

#[inline]
pub fn print(args: Arguments<'_>) {
    stdout_puts(args);
}

/// 打印格式字符串，无换行
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::print(core::format_args!($($arg)*));
    }
}

/// 打印格式字符串，有换行
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        $crate::print(core::format_args_nl!($($arg)*));
    }
}