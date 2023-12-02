use core::{
    arch::asm,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::sync::Arc;
use defines::{config::HART_NUM, trap_context::TrapContext};
use memory::KERNEL_SPACE;
use riscv::register::sstatus;

use crate::{process::Process, thread::Thread};

core::arch::global_asm!(include_str!("entry.S"));

static mut HARTS: [Hart; HART_NUM] = [const { Hart::new() }; HART_NUM];

/// 可以认为代表一个处理器。存放一些 per-hart 的数据
///
/// 因此，一般可以假定不会被并行访问
#[repr(align(32))]
pub struct Hart {
    hart_id: usize,
    //TODO: 内核线程是不是会不太一样？
    /// 当前 hart 上正在运行的线程。
    thread: Option<Arc<Thread>>,
}

impl Hart {
    pub const fn new() -> Hart {
        Hart {
            hart_id: 0,
            thread: None,
        }
    }

    pub const fn hart_id(&self) -> usize {
        self.hart_id
    }

    #[track_caller]
    pub fn trap_context(&self) -> *mut TrapContext {
        self.thread
            .as_ref()
            .expect("Only user task has trap context")
            .lock_inner(|inner| &mut inner.trap_context as _)
    }

    pub fn replace_thread(&mut self, new_thread: Option<Arc<Thread>>) -> Option<Arc<Thread>> {
        core::mem::replace(&mut self.thread, new_thread)
    }

    pub fn curr_thread(&self) -> &Thread {
        self.thread.as_ref().unwrap()
    }

    pub fn curr_process(&self) -> Arc<Process> {
        self.curr_thread().process.upgrade().unwrap()
    }
}

static INIF_HART: AtomicBool = AtomicBool::new(true);
static INIT_FINISHED: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn __hart_entry(hart_id: usize) -> ! {
    if INIF_HART
        .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        // 清理 bss 段
        extern "C" {
            fn sbss();
            fn ebss();
        }
        unsafe {
            core::slice::from_raw_parts_mut(
                sbss as usize as *mut u8,
                ebss as usize - sbss as usize,
            )
            .fill(0);

            memory::init();
            KERNEL_SPACE.activate();
            set_local_hart(hart_id);
        }

        info!("Init hart {hart_id} started");
        INIT_FINISHED.store(true, Ordering::SeqCst);

        // 将下面的代码取消注释即可启动多核
        // for i in 0..HART_NUM {
        //     if i == hart_id {
        //         continue;
        //     }
        //     utils::arch::hart_start(i, utils::config::HART_START_ADDR);
        // }
    } else {
        while !INIT_FINISHED.load(Ordering::SeqCst) {
            core::hint::spin_loop()
        }
        unsafe {
            set_local_hart(hart_id);
        }
        KERNEL_SPACE.activate();
        info!("Hart {hart_id} started");
    }

    // 允许在内核态下访问用户数据
    // TODO: 这个应该做成只在需要访问时设置，以防止意外
    unsafe { sstatus::set_sum() };

    crate::kernel_loop();
}

/// 设置当前 hart 的 `Hart` 结构，将 `tp` 设置为其地址
///
/// # Safety
///
/// 需保证由不同 hart 调用
unsafe fn set_local_hart(hart_id: usize) {
    let hart = &mut HARTS[hart_id];
    hart.hart_id = hart_id;
    let hart_addr = hart as *const _ as usize;
    asm!("mv tp, {}", in(reg) hart_addr);
}

pub fn local_hart() -> *const Hart {
    let tp: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) tp);
    }
    tp as *const Hart
}

pub fn local_hart_mut() -> *mut Hart {
    let tp: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) tp);
    }
    tp as *mut Hart
}

pub fn curr_process() -> Arc<Process> {
    unsafe { (*local_hart()).curr_process() }
}
