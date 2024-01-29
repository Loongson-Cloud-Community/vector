//! Implement syscalls using the vDSO.
//!
//! <https://man7.org/linux/man-pages/man7/vdso.7.html>
//!
//! # Safety
//!
//! Similar to syscalls.rs, this file performs raw system calls, and sometimes
//! passes them uninitialized memory buffers. This file also calls vDSO
//! functions.
#![allow(unsafe_code)]

use super::conv::{c_int, ret};
#[cfg(target_arch = "x86")]
use super::reg::{ArgReg, RetReg, SyscallNumber, A0, A1, A2, A3, A4, A5, R0};
use super::time::types::{ClockId, DynamicClockId, Timespec};
use super::{c, vdso};
use crate::io;
#[cfg(all(asm, target_arch = "x86"))]
use core::arch::asm;
use core::mem::{transmute, MaybeUninit};
use core::ptr::null_mut;
use core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering::Relaxed;
#[cfg(target_pointer_width = "32")]
use linux_raw_sys::general::timespec as __kernel_old_timespec;
use linux_raw_sys::general::{__kernel_clockid_t, __kernel_timespec};

#[inline]
pub(crate) fn clock_gettime(which_clock: ClockId) -> __kernel_timespec {
    // Safety: `CLOCK_GETTIME` contains either null or the address of a
    // function with an ABI like libc `clock_gettime`, and calling it has
    // the side effect of writing to the result buffer, and no others.
    unsafe {
        let mut result = MaybeUninit::<__kernel_timespec>::uninit();
        let callee = match transmute(CLOCK_GETTIME.load(Relaxed)) {
            Some(callee) => callee,
            None => init_clock_gettime(),
        };
        let r0 = callee(which_clock as c::c_int, result.as_mut_ptr());
        assert_eq!(r0, 0);
        result.assume_init()
    }
}

#[inline]
pub(crate) fn clock_gettime_dynamic(which_clock: DynamicClockId<'_>) -> io::Result<Timespec> {
    let id = match which_clock {
        DynamicClockId::Known(id) => id as __kernel_clockid_t,

        DynamicClockId::Dynamic(fd) => {
            // See `FD_TO_CLOCKID` in Linux's `clock_gettime` documentation.
            use crate::backend::fd::AsRawFd;
            const CLOCKFD: i32 = 3;
            ((!fd.as_raw_fd() << 3) | CLOCKFD) as __kernel_clockid_t
        }

        DynamicClockId::RealtimeAlarm => {
            linux_raw_sys::general::CLOCK_REALTIME_ALARM as __kernel_clockid_t
        }
        DynamicClockId::Tai => linux_raw_sys::general::CLOCK_TAI as __kernel_clockid_t,
        DynamicClockId::Boottime => linux_raw_sys::general::CLOCK_BOOTTIME as __kernel_clockid_t,
        DynamicClockId::BoottimeAlarm => {
            linux_raw_sys::general::CLOCK_BOOTTIME_ALARM as __kernel_clockid_t
        }
    };

    // Safety: `CLOCK_GETTIME` contains either null or the address of a
    // function with an ABI like libc `clock_gettime`, and calling it has
    // the side effect of writing to the result buffer, and no others.
    unsafe {
        const EINVAL: c::c_int = -(c::EINVAL as c::c_int);
        let mut timespec = MaybeUninit::<Timespec>::uninit();
        let callee = match transmute(CLOCK_GETTIME.load(Relaxed)) {
            Some(callee) => callee,
            None => init_clock_gettime(),
        };
        match callee(id, timespec.as_mut_ptr()) {
            0 => (),
            EINVAL => return Err(io::Errno::INVAL),
            _ => _rustix_clock_gettime_via_syscall(id, timespec.as_mut_ptr())?,
        }
        Ok(timespec.assume_init())
    }
}

#[cfg(target_arch = "x86")]
pub(super) mod x86_via_vdso {
    use super::{transmute, ArgReg, Relaxed, RetReg, SyscallNumber, A0, A1, A2, A3, A4, A5, R0};
    use crate::backend::arch::asm;

    #[inline]
    pub(in crate::backend) unsafe fn syscall0(nr: SyscallNumber<'_>) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall0(callee, nr)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall1<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
    ) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall1(callee, nr, a0)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall1_noreturn<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
    ) -> ! {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall1_noreturn(callee, nr, a0)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall2<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
        a1: ArgReg<'a, A1>,
    ) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall2(callee, nr, a0, a1)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall3<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
        a1: ArgReg<'a, A1>,
        a2: ArgReg<'a, A2>,
    ) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall3(callee, nr, a0, a1, a2)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall4<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
        a1: ArgReg<'a, A1>,
        a2: ArgReg<'a, A2>,
        a3: ArgReg<'a, A3>,
    ) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall4(callee, nr, a0, a1, a2, a3)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall5<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
        a1: ArgReg<'a, A1>,
        a2: ArgReg<'a, A2>,
        a3: ArgReg<'a, A3>,
        a4: ArgReg<'a, A4>,
    ) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall5(callee, nr, a0, a1, a2, a3, a4)
    }

    #[inline]
    pub(in crate::backend) unsafe fn syscall6<'a>(
        nr: SyscallNumber<'a>,
        a0: ArgReg<'a, A0>,
        a1: ArgReg<'a, A1>,
        a2: ArgReg<'a, A2>,
        a3: ArgReg<'a, A3>,
        a4: ArgReg<'a, A4>,
        a5: ArgReg<'a, A5>,
    ) -> RetReg<R0> {
        let callee = match transmute(super::SYSCALL.load(Relaxed)) {
            Some(callee) => callee,
            None => super::init_syscall(),
        };
        asm::indirect_syscall6(callee, nr, a0, a1, a2, a3, a4, a5)
    }

    // With the indirect call, it isn't meaningful to do a separate
    // `_readonly` optimization.
    pub(in crate::backend) use {
        syscall0 as syscall0_readonly, syscall1 as syscall1_readonly,
        syscall2 as syscall2_readonly, syscall3 as syscall3_readonly,
        syscall4 as syscall4_readonly, syscall5 as syscall5_readonly,
        syscall6 as syscall6_readonly,
    };
}

type ClockGettimeType = unsafe extern "C" fn(c::c_int, *mut Timespec) -> c::c_int;

/// The underlying syscall functions are only called from asm, using the
/// special syscall calling convention to pass arguments and return values,
/// which the signature here doesn't reflect.
#[cfg(target_arch = "x86")]
pub(super) type SyscallType = unsafe extern "C" fn();

/// Initialize `CLOCK_GETTIME` and return its value.
fn init_clock_gettime() -> ClockGettimeType {
    init();
    // Safety: Load the function address from static storage that we
    // just initialized.
    unsafe { transmute(CLOCK_GETTIME.load(Relaxed)) }
}

/// Initialize `SYSCALL` and return its value.
#[cfg(target_arch = "x86")]
fn init_syscall() -> SyscallType {
    init();
    // Safety: Load the function address from static storage that we
    // just initialized.
    unsafe { transmute(SYSCALL.load(Relaxed)) }
}

/// `AtomicPtr` can't hold a `fn` pointer, so we use a `*` pointer to this
/// placeholder type, and cast it as needed.
struct Function;
static mut CLOCK_GETTIME: AtomicPtr<Function> = AtomicPtr::new(null_mut());
#[cfg(target_arch = "x86")]
static mut SYSCALL: AtomicPtr<Function> = AtomicPtr::new(null_mut());

unsafe extern "C" fn rustix_clock_gettime_via_syscall(
    clockid: c::c_int,
    res: *mut Timespec,
) -> c::c_int {
    match _rustix_clock_gettime_via_syscall(clockid, res) {
        Ok(()) => 0,
        Err(err) => err.raw_os_error().wrapping_neg(),
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn _rustix_clock_gettime_via_syscall(
    clockid: c::c_int,
    res: *mut Timespec,
) -> io::Result<()> {
    let r0 = syscall!(__NR_clock_gettime64, c_int(clockid), res);
    match ret(r0) {
        Err(io::Errno::NOSYS) => _rustix_clock_gettime_via_syscall_old(clockid, res),
        otherwise => otherwise,
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn _rustix_clock_gettime_via_syscall_old(
    clockid: c::c_int,
    res: *mut Timespec,
) -> io::Result<()> {
    // Ordinarily `rustix` doesn't like to emulate system calls, but in
    // the case of time APIs, it's specific to Linux, specific to
    // 32-bit architectures *and* specific to old kernel versions, and
    // it's not that hard to fix up here, so that no other code needs
    // to worry about this.
    let mut old_result = MaybeUninit::<__kernel_old_timespec>::uninit();
    let r0 = syscall!(__NR_clock_gettime, c_int(clockid), &mut old_result);
    match ret(r0) {
        Ok(()) => {
            let old_result = old_result.assume_init();
            *res = Timespec {
                tv_sec: old_result.tv_sec.into(),
                tv_nsec: old_result.tv_nsec.into(),
            };
            Ok(())
        }
        otherwise => otherwise,
    }
}

#[cfg(target_pointer_width = "64")]
unsafe fn _rustix_clock_gettime_via_syscall(
    clockid: c::c_int,
    res: *mut Timespec,
) -> io::Result<()> {
    ret(syscall!(__NR_clock_gettime, c_int(clockid), res))
}

/// A symbol pointing to an `int 0x80` instruction. This "function" is only
/// called from assembly, and only with the x86 syscall calling convention,
/// so its signature here is not its true signature.
#[cfg(all(asm, target_arch = "x86"))]
#[naked]
unsafe extern "C" fn rustix_int_0x80() {
    asm!("int $$0x80", "ret", options(noreturn))
}

// The outline version of the `rustix_int_0x80` above.
#[cfg(all(not(asm), target_arch = "x86"))]
extern "C" {
    fn rustix_int_0x80();
}

fn minimal_init() {
    // Safety: Store default function addresses in static storage so that if we
    // end up making any system calls while we read the vDSO, they'll work.
    // If the memory happens to already be initialized, this is redundant, but
    // not harmful.
    unsafe {
        CLOCK_GETTIME
            .compare_exchange(
                null_mut(),
                rustix_clock_gettime_via_syscall as *mut Function,
                Relaxed,
                Relaxed,
            )
            .ok();
        #[cfg(target_arch = "x86")]
        {
            SYSCALL
                .compare_exchange(
                    null_mut(),
                    rustix_int_0x80 as *mut Function,
                    Relaxed,
                    Relaxed,
                )
                .ok();
        }
    }
}

fn init() {
    minimal_init();

    if let Some(vdso) = vdso::Vdso::new() {
        // Look up the platform-specific `clock_gettime` symbol as documented
        // [here], except on 32-bit platforms where we look up the
        // `64`-suffixed variant and fail if we don't find it.
        //
        // [here]: https://man7.org/linux/man-pages/man7/vdso.7.html
        #[cfg(target_arch = "x86_64")]
        let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime"));
        #[cfg(target_arch = "arm")]
        let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime64"));
        #[cfg(target_arch = "aarch64")]
        let ptr = vdso.sym(cstr!("LINUX_2.6.39"), cstr!("__kernel_clock_gettime"));
        #[cfg(target_arch = "x86")]
        let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime64"));
        #[cfg(target_arch = "riscv64")]
        let ptr = vdso.sym(cstr!("LINUX_4.15"), cstr!("__vdso_clock_gettime"));
        #[cfg(target_arch = "powerpc64")]
        let ptr = vdso.sym(cstr!("LINUX_2.6.15"), cstr!("__kernel_clock_gettime"));
        #[cfg(target_arch = "mips")]
        let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime64"));
        #[cfg(target_arch = "mips64")]
        let ptr = vdso.sym(cstr!("LINUX_2.6"), cstr!("__vdso_clock_gettime"));

        // On all 64-bit platforms, the 64-bit `clock_gettime` symbols are
        // always available.
        #[cfg(any(target_pointer_width = "64"))]
        let ok = true;

        // On some 32-bit platforms, the 64-bit `clock_gettime` symbols are not
        // available on older kernel versions.
        #[cfg(any(target_arch = "arm", target_arch = "mips", target_arch = "x86"))]
        let ok = !ptr.is_null();

        if ok {
            assert!(!ptr.is_null());

            // Safety: Store the computed function addresses in static storage
            // so that we don't need to compute it again (but if we do, it doesn't
            // hurt anything).
            unsafe {
                CLOCK_GETTIME.store(ptr.cast(), Relaxed);
            }
        }

        // On x86, also look up the vsyscall entry point.
        #[cfg(target_arch = "x86")]
        {
            let ptr = vdso.sym(cstr!("LINUX_2.5"), cstr!("__kernel_vsyscall"));
            assert!(!ptr.is_null());

            // Safety: As above, store the computed function addresses in
            // static storage.
            unsafe {
                SYSCALL.store(ptr.cast(), Relaxed);
            }
        }
    }
}
