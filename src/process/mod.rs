// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    cell::UnsafeCell,
    ffi::c_int,
    ptr::{NonNull, addr_eq},
    sync::atomic::{AtomicI32, AtomicI64, AtomicU32, Ordering},
    usize,
};

use alloc::{
    collections::btree_map::BTreeMap,
    ffi::CString,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use elf::AuxvEntry;
use files::FileDesc;
use signal::Sigtab;
use usercopy::{AccessResult, UserSlice, UserSliceMut};

use crate::{
    bindings::{
        device::class::char::CharDevice,
        error::{EResult, Errno},
        log::LogLevel,
    },
    config::{PAGE_SIZE, STACK_SIZE},
    cpu::{self, mmu, thread::GpRegfile, usermode::call_usermode},
    device::builtin_driver::null_instance,
    filesystem::{self, File, SeekMode, device::CharDevFile, mode, oflags},
    kernel::{
        sched::Thread,
        sync::mutex::{Mutex, SharedMutexGuard},
    },
    mem::{
        pmm::PPN,
        vmm::{self, Memmap},
    },
    process::files::FDTable,
};

mod c_api;
pub mod elf;
pub mod files;
pub mod signal;
pub mod syscall;
pub mod sysimpl;
pub mod uapi;
pub mod usercopy;

pub mod flags {
    pub const STOPPING: u32 = 1 << 0;
}

/// Unique process identifier.
pub type PID = c_int;

/// Count of next PID that will be used.
/// TODO: We can actually make this 64-bit but I'll do that later.
pub static PID_COUNTER: AtomicI32 = AtomicI32::new(1);

/// Map of all processes by PID.
static PROCESSES: Mutex<BTreeMap<PID, Arc<Process>>> = Mutex::new(BTreeMap::new());

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
    pub envp: Vec<CString>,
    pub auxv: Vec<AuxvEntry>,
}

/// Process threads list.
struct ProcThreads {
    threads: BTreeMap<i64, Arc<Thread>>,
    detached: Vec<Weak<Thread>>,
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
    threads: Mutex<ProcThreads>,
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
        self.cmdline.unintr_lock_shared()
    }

    /// Get the page table's root physical page number.
    pub fn pagetable(&self) -> PPN {
        self.memmap().root_ppn()
    }

    /// Prepare the entrypoint stack.
    fn create_entry_stack(&self, stack_top: *mut (), auxv: &[AuxvEntry]) -> AccessResult<*mut ()> {
        let cmdline = self.cmdline();
        let mut argv = Vec::<usize>::new();
        let mut envp = Vec::<usize>::new();

        let mut stack_top = stack_top as *mut u8;

        for arg in &cmdline.argv {
            let arg = arg.as_bytes_with_nul();
            stack_top = stack_top.wrapping_sub(arg.len());
            UserSlice::new_mut(stack_top, arg.len())?.write_multiple(0, arg)?;
            argv.push(stack_top as usize);
        }
        argv.push(0);

        for arg in &cmdline.envp {
            let arg = arg.as_bytes_with_nul();
            stack_top = stack_top.wrapping_sub(arg.len());
            UserSlice::new_mut(stack_top, arg.len())?.write_multiple(0, arg)?;
            envp.push(stack_top as usize);
        }
        envp.push(0);

        let mut pointers = Vec::<usize>::new();
        pointers.push(argv.len() - 1);
        pointers.extend_from_slice(&argv);
        pointers.extend_from_slice(&envp);
        for ent in auxv {
            pointers.push(ent.type_);
            pointers.push(ent.value);
        }
        pointers.push(0);

        let stack_top = stack_top as usize - size_of::<usize>() * pointers.len();
        let stack_top = stack_top - stack_top % 16;
        UserSlice::new_mut(stack_top as *mut usize, pointers.len())?
            .write_multiple(0, &pointers)?;

        Ok(stack_top as *mut ())
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
                envp: Vec::new(),
                auxv: Vec::new(),
            }),
            tid_counter: AtomicI64::new(0),
            threads: Mutex::new(ProcThreads {
                threads: BTreeMap::new(),
                detached: Vec::new(),
            }),
            flags: AtomicU32::new(0),
            wait_status: AtomicI32::new(0),
            sigtab: Mutex::new(Sigtab::default()),
        };
        proc.prefill_stdio_fds()?;
        let entry = load_executable(
            proc.memmap(),
            &mut proc.cmdline.unintr_lock(),
            file.as_ref(),
            0,
        )?;

        let proc = Arc::try_new(proc)?;
        PROCESSES.unintr_lock().insert(1, proc.clone());

        let proc2 = proc.clone();
        proc.create_thread(
            move |stack_top| {
                let stack_top = proc2
                    .create_entry_stack(stack_top, &proc2.cmdline().auxv)
                    .unwrap();
                (entry as *const (), stack_top)
            },
            Some("U: main".into()),
        )?;

        logkf!(LogLevel::Debug, "Process {} started", proc.pid);

        Ok(proc)
    }

    /// Fork this process.
    pub fn fork(self: &Arc<Self>, regs: &GpRegfile) -> EResult<Arc<Process>> {
        let pid = PID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let child = Process {
            pid,
            pcr: Mutex::new(PCR {
                parent: Arc::downgrade(&self),
                children: BTreeMap::new(),
            }),
            flags: AtomicU32::new(0),
            wait_status: AtomicI32::new(0),
            cmdline: Mutex::new(self.cmdline.lock_shared()?.clone()),
            memmap: UnsafeCell::new(self.memmap().fork()?),
            sigtab: Mutex::new(*self.sigtab.lock_shared()?),
            files: Mutex::new(self.files.lock()?.clone()),
            tid_counter: AtomicI64::new(self.tid_counter.load(Ordering::Relaxed)),
            threads: Mutex::new(ProcThreads {
                threads: BTreeMap::new(),
                detached: Vec::new(),
            }),
        };
        let child = Arc::try_new(child)?;
        let mut pcr = self.pcr.unintr_lock();
        let mut processes = PROCESSES.unintr_lock();

        let mut regs2 = regs.clone();
        let thread = Thread::new(
            move || {
                // Clone the calling thread and start it.
                regs2.set_retval(0);
                call_usermode(&regs2);

                // TODO: Clean up the stack.
            },
            Some(child.clone()),
            None,
        )?;
        child.threads.unintr_lock().threads.insert(0, thread);

        pcr.children.insert(pid, child.clone());
        processes.insert(pid, child.clone());
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
        // TODO: From this point on, errors are to kill the process instead of returning an error code.
        self.join_all_threads();
        self.files.unintr_lock().close_cloexec();
        self.flags.store(0, Ordering::Relaxed);
        self.tid_counter.store(0, Ordering::Relaxed);
        *self.sigtab.unintr_lock() = Sigtab::default();
        *self.cmdline.unintr_lock() = cmdline;
        self.prefill_stdio_fds().unwrap();

        unsafe {
            cpu::mmu::set_page_table(new_mm.root_ppn(), 0);
            *self.memmap.as_mut_unchecked() = new_mm;
        }
        let proc2 = self.clone();
        self.create_thread(
            move |stack_top| {
                let stack_top = proc2
                    .create_entry_stack(stack_top, &proc2.cmdline().auxv)
                    .unwrap();
                (entry as *const (), stack_top)
            },
            Some("U: main".into()),
        )
        .unwrap();

        logkf!(LogLevel::Debug, "Process {} exec'ed", self.pid);

        Ok(())
    }

    /// Set up FDs 0, 1 and 2 if they are not present.
    fn prefill_stdio_fds(&self) -> EResult<()> {
        let mut fds = self.files.unintr_lock();

        if fds.inner.contains_key(&0) && fds.inner.contains_key(&1) && fds.inner.contains_key(&2) {
            return Ok(());
        }

        let mut serial_devs = CharDevice::filter(Default::default())?;
        let stdio_dev = serial_devs.try_remove(0).unwrap_or_else(|| null_instance());
        let file = Arc::try_new(CharDevFile::new_raw(stdio_dev))?;

        let _ = fds.inner.try_insert(
            0,
            FileDesc {
                flags: AtomicU32::new(0),
                file: file.clone(),
            },
        );
        let _ = fds.inner.try_insert(
            1,
            FileDesc {
                flags: AtomicU32::new(0),
                file: file.clone(),
            },
        );
        let _ = fds.inner.try_insert(
            2,
            FileDesc {
                flags: AtomicU32::new(0),
                file: file.clone(),
            },
        );

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
        let arc_self = PROCESSES
            .unintr_lock_shared()
            .get(&self.pid)
            .cloned()
            .unwrap();
        Thread::new(
            move || {
                arc_self.reap();
            },
            None,
            Some(format!("Process {} reaper", self.pid)),
        )
        .unwrap();
    }

    /// Join all currently running threads.
    fn join_all_threads(&self) {
        debug_assert!(self.flags.load(Ordering::Relaxed) & flags::STOPPING != 0);
        let mut threads = self.threads.unintr_lock();
        let thread_self = Thread::current();

        // Kindly ask all threads to stop.
        for thread in &threads.threads {
            thread.1.stop();
        }
        for thread in &threads.detached {
            if let Some(thread) = thread.upgrade() {
                thread.stop();
            }
        }

        // Now wait until all threads have indeed stopped.
        while let Some((_, thread)) = threads.threads.pop_first() {
            if !addr_eq(thread_self, thread.as_ref()) {
                let _ = thread.join();
            }
        }
        while let Some(thread) = threads.detached.pop() {
            if let Some(thread) = thread.upgrade() {
                if !addr_eq(thread_self, thread.as_ref()) {
                    let _ = thread.join();
                }
            }
        }

        drop(threads);
    }

    /// Reclaim this process' resources.
    fn reap(&self) {
        self.join_all_threads();
        logkf!(LogLevel::Debug, "Process {} stopped", self.pid);

        self.files.unintr_lock().clear();
        unsafe { self.memmap().clear() };
    }

    /// Create a new thread in this process.
    /// The `setup` function accepts the stack top and returns the entrypoint and the stack pointer to start into.
    pub fn create_thread(
        self: &Arc<Self>,
        setup: impl FnOnce(*mut ()) -> (*const (), *mut ()) + Send + 'static,
        name: Option<String>,
    ) -> EResult<i64> {
        let tid = self.tid_counter.fetch_add(1, Ordering::Relaxed);
        let stack_pages = (STACK_SIZE / PAGE_SIZE) as usize;
        // TODO: Safe and owning API for memory objects?
        let u_stack = unsafe { self.memmap().map_ram(None, stack_pages, vmm::flags::RW) }?;
        let proc_self = self.clone();
        let thread = Thread::new(
            move || {
                // Set up things on the stack.
                let u_stack_top = ((u_stack + stack_pages) * PAGE_SIZE as usize) as *mut ();
                let (pc, sp) = setup(u_stack_top);

                // Call user mode.
                let mut regs = GpRegfile::default();
                regs.set_pc(pc as _);
                regs.set_stack(sp as _);
                call_usermode(&regs);

                // Clean up the stack.
                unsafe {
                    proc_self.memmap().unmap(u_stack..u_stack + stack_pages);
                }
            },
            Some(self.clone()),
            name,
        );
        if thread.is_err() {
            unsafe {
                self.memmap().unmap(u_stack..u_stack + stack_pages);
            }
        }
        self.threads.unintr_lock().threads.insert(tid, thread?);
        Ok(tid)
    }
}

/// Implementation of [`load_executable`] for ELF files.
fn load_executable_elf(
    memmap: &Memmap,

    auxv: &mut Vec<AuxvEntry>,
    file: &dyn File,
    _recursion: u8,
) -> EResult<usize> {
    let thread = Thread::current();
    unsafe {
        (*thread).mm_override = Some(NonNull::from_ref(memmap));
        cpu::mmu::set_page_table(memmap.root_ppn(), 0);
        mmu::vmem_fence(None, None);
    }
    let res = elf::load(file, memmap, auxv);
    unsafe {
        (*thread).mm_override = None;
        cpu::mmu::set_page_table(memmap.root_ppn(), 0);
        mmu::vmem_fence(None, None);
    }
    res
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
        load_executable_elf(memmap, &mut cmdline.auxv, file, recursion)
    } else if magic_len >= 2 && magic[0] == b'#' && magic[1] == b'!' {
        load_executable_shebang(memmap, cmdline, file, recursion)
    } else {
        Err(Errno::ENOEXEC)
    }
}

/// Get the current process handle.
pub fn current() -> Option<Arc<Process>> {
    let thread = Thread::current();
    if thread.is_null() {
        return None;
    }
    unsafe { &*thread }.process.clone()
}
