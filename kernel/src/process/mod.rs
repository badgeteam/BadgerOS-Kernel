// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicI32, AtomicI64, AtomicU32, Ordering},
    usize,
};

use alloc::{collections::btree_map::BTreeMap, ffi::CString, sync::Arc, sync::Weak, vec::Vec};
use signal::Sigtab;
use usercopy::UserSliceMut;

use crate::{
    bindings::{
        self,
        error::{EResult, Errno},
        log::LogLevel,
        mutex::{Mutex, SharedMutexGuard},
        raw::{process_t, sched_current_tid, sched_get_thread, thread_fork, thread_resume},
        thread::Thread,
    },
    cpu,
    filesystem::{self, File, SeekMode, mode, oflags},
    mem::{pmm::PPN, vmm::Memmap},
    process::files::FDTable,
};

mod c_api;
pub mod elf;
pub mod files;
pub mod signal;
pub mod sysimpl;
pub mod usercopy;
pub mod uapi;

pub mod flags {
    pub const STOPPING: u32 = 1 << 0;
}

/// Unique process identifier.
pub type PID = i64;

/// Count of next PID that will be used.
pub static PID_COUNTER: AtomicI64 = AtomicI64::new(1);

/// Map of all processes by PID.
static PROCESSES: Mutex<BTreeMap<PID, Arc<Process>>> =
    unsafe { Mutex::new_static(BTreeMap::new()) };

/// Maximum number of file decriptor table entries.
pub const FILE_MAX: i32 = 256;

/// Process parent-child relations.
struct PCR {
    parent: Weak<Process>,
    children: BTreeMap<PID, Arc<Process>>,
}

/// Process arguments and other info from [`Process::exec`].
#[derive(Clone)]
pub struct Cmdline {
    pub binary: CString,
    pub argv: Vec<CString>,
}

/// The process descriptor structure.
pub struct Process {
    pub pid: PID,
    flags: AtomicU32,
    wait_status: AtomicI32,
    pcr: Mutex<PCR>,
    cmdline: Mutex<Cmdline>,
    memmap: UnsafeCell<Memmap>,
    sigtab: Mutex<Sigtab>,
    pub files: Mutex<FDTable>,
    tid_counter: AtomicI64,
    threads: Mutex<BTreeMap<i64, Thread>>,
}
impl PartialEq for Process {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}
impl Eq for Process {}
impl PartialOrd for Process {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.pid.partial_cmp(&other.pid)
    }
}
impl Ord for Process {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.pid.cmp(&other.pid)
    }
}
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    /// View the process' command line.
    pub fn cmdline<'a>(&'a self) -> SharedMutexGuard<'a, Cmdline> {
        self.cmdline.lock_shared()
    }

    /// Get the page table's root physical page number.
    pub fn pagetable(&self) -> PPN {
        self.memmap().root_ppn()
    }

    /// Create the init process.
    pub fn new_init() -> EResult<Arc<Process>> {
        // This assert enforces init isn't accidentally created twice.
        assert!(PID_COUNTER.fetch_add(1, Ordering::Relaxed) == 1);

        let init_path = CString::from(c"/sbin/init");
        let file = filesystem::open(None, init_path.as_bytes(), oflags::READ_ONLY)?;
        let proc = Process {
            pid: 1,
            pcr: Mutex::new(PCR {
                parent: Weak::new(),
                children: BTreeMap::new(),
            }),
            memmap: UnsafeCell::new(Memmap::new_user()?),
            files: Mutex::new(FDTable::default()),
            cmdline: Mutex::new(Cmdline {
                binary: init_path.clone(),
                argv: vec![init_path],
            }),
            tid_counter: AtomicI64::new(0),
            threads: Mutex::new(BTreeMap::new()),
            flags: AtomicU32::new(0),
            wait_status: AtomicI32::new(0),
            sigtab: Mutex::new(Sigtab::default()),
        };
        let entry = load_executable(proc.memmap(), &mut *proc.cmdline.lock(), file.as_ref(), 0)?;

        let proc = Arc::try_new(proc)?;
        PROCESSES.lock().insert(1, proc.clone());

        proc.create_thread(entry, 0, Some("main"))?;

        logkf!(LogLevel::Debug, "Process {} started", proc.pid);

        Ok(proc)
    }

    /// Fork this process.
    pub fn fork(self: &Arc<Self>) -> EResult<Arc<Process>> {
        let pid = PID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let child = Process {
            pid,
            pcr: Mutex::new(PCR {
                parent: Arc::downgrade(&self),
                children: BTreeMap::new(),
            }),
            flags: AtomicU32::new(0),
            wait_status: AtomicI32::new(0),
            cmdline: Mutex::new(self.cmdline.lock_shared().clone()),
            memmap: UnsafeCell::new(self.memmap().fork()?),
            sigtab: Mutex::new(*self.sigtab.lock_shared()),
            files: self.files.clone(),
            tid_counter: AtomicI64::new(self.tid_counter.load(Ordering::Relaxed)),
            threads: Mutex::new(BTreeMap::new()),
        };
        let child = Arc::try_new(child)?;
        self.pcr.lock().children.insert(pid, child.clone());
        PROCESSES.lock().insert(pid, child.clone());

        let thread = unsafe {
            let tid = Errno::check_i32(thread_fork(
                self.threads.lock_shared().get(&0).unwrap().tid(),
                child.as_ref() as *const Process as *mut process_t,
            ))?;
            let thread_struct = sched_get_thread(tid);
            // TODO: Portable impl for this after sched rewrite.
            (*thread_struct).user_isr_ctx.regs.pc += 4;
            (*thread_struct).user_isr_ctx.regs.a0 = 0;
            thread_resume(tid);
            Thread::from_id(tid)
        };
        child.threads.lock().insert(0, thread);

        logkf!(
            LogLevel::Debug,
            "Process {} forked into {}",
            self.pid,
            child.pid
        );

        Ok(child)
    }

    /// Execute a new binary, replacing this process.
    pub fn exec(self: &Arc<Self>, mut cmdline: Cmdline) -> EResult<()> {
        let new_mm = Memmap::new_user()?;
        let file = filesystem::open(None, cmdline.binary.as_bytes(), oflags::READ_ONLY)?;

        let entry = load_executable(&new_mm, &mut cmdline, file.as_ref(), 0)?;

        if self.flags.fetch_or(flags::STOPPING, Ordering::Relaxed) != 0 {
            // Some other thread either `exit()`'ed or `exec()`'ed first.
            return Ok(());
        }

        // Commit to replacing the process.
        self.join_all_threads();
        self.files.lock().close_cloexec();
        self.flags.store(0, Ordering::Relaxed);
        self.tid_counter.store(0, Ordering::Relaxed);
        *self.sigtab.lock() = Sigtab::default();
        *self.cmdline.lock() = cmdline;

        unsafe {
            cpu::mmu::set_page_table(new_mm.root_ppn(), 0);
            *self.memmap.as_mut_unchecked() = new_mm;
        }
        self.create_thread(entry, 0, Some("main")).unwrap();

        logkf!(LogLevel::Debug, "Process {} exec'ed", self.pid);

        Ok(())
    }

    /// Subject to replacement.
    pub fn memmap(&self) -> &Memmap {
        unsafe { self.memmap.as_ref_unchecked() }
    }

    /// Kill this process.
    pub fn kill(&self, status: i32) {
        self.wait_status.store(status, Ordering::Relaxed);
        if self.flags.fetch_or(flags::STOPPING, Ordering::Relaxed) & flags::STOPPING != 0 {
            return;
        }

        // TODO: Spawning a full thread for this is somewhat inefficient.
        let arc_self = PROCESSES.lock_shared().get(&self.pid).cloned().unwrap();
        Thread::new_kernel(
            move || {
                arc_self.reap();
                0
            },
            Some("process reaper"),
        )
        .detach();
    }

    /// Join all currently running threads.
    fn join_all_threads(&self) {
        debug_assert!(self.flags.load(Ordering::Relaxed) & flags::STOPPING != 0);
        let mut threads = self.threads.lock();
        let tid_self = unsafe { sched_current_tid() };
        while let Some((_, thread)) = threads.pop_first() {
            if tid_self != thread.tid() {
                thread.join();
            }
        }
        drop(threads);
    }

    /// Reclaim this process' resources.
    fn reap(&self) {
        self.join_all_threads();
        logkf!(LogLevel::Debug, "Process {} stopped", self.pid);

        self.files.lock().clear();
        unsafe { self.memmap().clear() };
    }

    /// Create a new thread in this process.
    pub fn create_thread(
        self: &Arc<Self>,
        entry: usize,
        arg: usize,
        name: Option<&str>,
    ) -> EResult<i64> {
        let tid = self.tid_counter.fetch_add(1, Ordering::Relaxed);
        let thread =
            unsafe { Thread::try_new_user(Arc::into_raw(self.clone()), entry, arg, name)? };
        self.threads.lock().insert(tid, thread);
        Ok(tid)
    }
}

/// Implementation of [`load_executable`] for ELF files.
fn load_executable_elf(memmap: &Memmap, file: &dyn File, _recursion: u8) -> EResult<usize> {
    elf::load(file, memmap, false)
}

/// Implementation of [`load_executable`] for `#!` interpreted executables.
fn load_executable_shebang(
    memmap: &Memmap,
    cmdline: &mut Cmdline,
    file: &dyn File,
    recursion: u8,
) -> EResult<usize> {
    // Extract the args from the shebang.
    let mut buf = Vec::try_with_capacity(filesystem::PATH_MAX)?;
    buf.resize(filesystem::PATH_MAX, 0u8);
    let file_len = file.read(UserSliceMut::new_kernel_mut(&mut buf))?;
    let line_len = buf.iter().position(|&x| x == b'\n').ok_or(Errno::ENOEXEC)?;
    buf.resize(file_len.min(line_len), 0);

    if buf.contains(&0) {
        // This first line isn't allowed to contain nul bytes.
        return Err(Errno::ENOEXEC);
    }

    // Split at whitespace and prepend onto argv.
    let mut i = 0;
    for word in buf.split(|&x| x == b' ') {
        cmdline.argv.insert(i, CString::new(word).unwrap());
        i += 1;
    }

    let interp = filesystem::open(None, cmdline.argv[0].as_bytes(), oflags::READ_ONLY)?;
    load_executable(memmap, cmdline, interp.as_ref(), recursion + 1)
}

/// Load the binary for a process and prepare for its execution.
fn load_executable(
    memmap: &Memmap,
    cmdline: &mut Cmdline,
    file: &dyn File,
    recursion: u8,
) -> EResult<usize> {
    if recursion > 8 {
        // Too much recursion in the program interpreters.
        return Err(Errno::EMLINK);
    }

    // Must be a regular file.
    let stat = file.stat()?;
    if stat.mode & mode::S_IFMT != mode::S_IFREG {
        return Err(Errno::ENOEXEC);
    }

    // Determine what method of execution to use.
    let mut magic = [0u8; 4];
    file.seek(SeekMode::Set, 0)?;
    let magic_len = file.read(UserSliceMut::new_kernel_mut(&mut magic))?;

    if magic_len >= elf::ELF_MAGIC.len() && magic == elf::ELF_MAGIC {
        load_executable_elf(memmap, file, recursion)
    } else if magic_len >= 2 && magic[0] == b'#' && magic[1] == b'!' {
        load_executable_shebang(memmap, cmdline, file, recursion)
    } else {
        Err(Errno::ENOEXEC)
    }
}

/// Get the current process handle.
pub fn current() -> Option<Arc<Process>> {
    unsafe {
        let thread = bindings::raw::sched_current_thread();
        if thread.is_null() {
            return None;
        }
        let proc = (*thread).process as *const Process;
        let proc = Arc::from_raw(proc);
        // Clone the arc from the thread instead of dropping it.
        core::mem::forget(proc.clone());
        Some(proc)
    }
}
