mod inner;
mod user;

use alloc::sync::Weak;
use defines::config::{LOW_ADDRESS_END, PAGE_SIZE, USER_STACK_SIZE};
use defines::trap_context::TrapContext;
use memory::{MapPermission, MemorySet, VirtAddr};
use spin::Mutex;

use crate::process::Process;

use self::inner::ThreadInner;

pub use self::inner::ThreadStatus;
pub use self::user::spawn_user_thread;

/// 进程控制块
pub struct Thread {
    pub tid: usize,
    // TODO: 其实也不是一定要用 Weak。完全可以手动释放
    pub process: Weak<Process>,
    inner: Mutex<ThreadInner>,
}

impl Thread {
    pub fn new(process: Weak<Process>, tid: usize, trap_context: TrapContext) -> Self {
        Self {
            tid,
            process,
            inner: Mutex::new(ThreadInner {
                exit_code: 0,
                thread_status: ThreadStatus::Ready,
                trap_context,
            }),
        }
    }

    /// 锁 inner 然后进行操作。这应该是访问 inner 的唯一方式
    pub fn lock_inner<T>(&self, f: impl FnOnce(&mut ThreadInner) -> T) -> T {
        f(&mut self.inner.lock())
    }

    /// 分配用户栈，一般用于创建新线程。返回用户栈高地址
    ///
    /// 注意 `memory_set` 是进程的 `MemorySet`
    pub fn alloc_user_stack(tid: usize, memory_set: &mut MemorySet) -> usize {
        // 分配用户栈
        let ustack_low_addr = Self::user_stack_low_addr(tid);
        log::debug!("stack low addr: {:#x}", ustack_low_addr);
        let ustack_high_addr = ustack_low_addr + USER_STACK_SIZE;
        log::debug!("stack high addr: {:#x}", ustack_high_addr);
        memory_set.insert_framed_area(
            VirtAddr(ustack_low_addr).vpn_floor(),
            VirtAddr(ustack_high_addr).vpn_ceil(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );
        ustack_high_addr
    }

    /// 获取当前线程用户栈的低地址，即高地址减去用户栈大小
    fn user_stack_low_addr(tid: usize) -> usize {
        Self::user_stack_high_addr(tid) - USER_STACK_SIZE
    }

    /// 获取当前线程用户栈的高地址
    fn user_stack_high_addr(tid: usize) -> usize {
        // 注意每个用户栈后都会有一个 Guard Page
        LOW_ADDRESS_END - tid * (USER_STACK_SIZE + PAGE_SIZE)
    }

    /// 释放用户栈。一般是单个线程退出时使用。
    ///
    /// 注意 `memory_set` 是进程的 `MemorySet`
    fn dealloc_user_stack(&self, memory_set: &mut MemorySet) {
        // 手动取消用户栈的映射
        let user_stack_low_addr = VirtAddr(Self::user_stack_low_addr(self.tid));
        memory_set.remove_area_with_start_vpn(user_stack_low_addr.vpn());
    }

    pub async fn yield_now(&self) {
        self.inner.lock().thread_status = ThreadStatus::Ready;
        executor::yield_now().await
    }
}