#![allow(dead_code)]

use super::abi;
use super::thread_local_dtor::run_dtors;
use crate::ffi::{CStr, CString};
use crate::io;
use crate::mem;
use crate::num::NonZero;
use crate::ptr;
use crate::time::Duration;

pub type Tid = abi::Tid;

pub struct Thread {
    tid: Tid,
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

pub const DEFAULT_MIN_STACK_SIZE: usize = 1 << 20;

impl Thread {
    pub unsafe fn new_with_coreid(
        stack: usize,
        p: Box<dyn FnOnce()>,
        core_id: isize,
    ) -> io::Result<Thread> {
        let p = Box::into_raw(Box::new(p));
        let tid = abi::spawn2(
            thread_start,
            p.expose_addr(),
            abi::Priority::into(abi::NORMAL_PRIO),
            stack,
            core_id,
        );

        return if tid == 0 {
            // The thread failed to start and as a result p was not consumed. Therefore, it is
            // safe to reconstruct the box so that it gets deallocated.
            drop(Box::from_raw(p));
            Err(io::const_io_error!(io::ErrorKind::Uncategorized, "Unable to create thread!"))
        } else {
            Ok(Thread { tid: tid })
        };

        extern "C" fn thread_start(main: usize) {
            unsafe {
                // Finally, let's run some code.
                Box::from_raw(ptr::with_exposed_provenance::<Box<dyn FnOnce()>>(main).cast_mut())();

                // run all destructors
                run_dtors();
            }
        }
    }

    pub unsafe fn new(stack: usize, p: Box<dyn FnOnce()>) -> io::Result<Thread> {
        Thread::new_with_coreid(stack, p, -1 /* = no specific core */)
    }

    #[inline]
    pub fn yield_now() {
        unsafe {
            abi::yield_now();
        }
    }

    #[inline]
    pub fn set_name(_name: &CStr) {
        // nope
    }

    pub fn get_name() -> Option<CString> {
        None
    }

    #[inline]
    pub fn sleep(dur: Duration) {
        unsafe {
            abi::usleep(dur.as_micros() as u64);
        }
    }

    pub fn join(self) {
        unsafe {
            let _ = abi::join(self.tid);
        }
    }

    #[inline]
    pub fn id(&self) -> Tid {
        self.tid
    }

    #[inline]
    pub fn into_id(self) -> Tid {
        let id = self.tid;
        mem::forget(self);
        id
    }
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    unsafe { Ok(NonZero::new_unchecked(abi::get_processor_count())) }
}
