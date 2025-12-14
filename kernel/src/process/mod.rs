// SPDX-FileCopyrightText: 2025 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

use core::{
    ops::Deref,
    sync::atomic::{AtomicI32, AtomicI64, AtomicU32, Ordering},
    usize,
};

use alloc::{collections::btree_map::BTreeMap, ffi::CString, sync::Arc, vec::Vec};
use signal::Sigtab;
use usercopy::UserSliceMut;

use crate::{
    bindings::{
        self,
        error::{EResult, Errno},
        log::LogLevel,
        mutex::Mutex,
        raw::{process_t, sched_get_thread, thread_fork, thread_resume},
        thread::Thread,
    },
    filesystem::{self, File, SeekMode, mode, oflags, sysimpl::AT_FDCWD},
    mem::{pmm::PPN, vmm::Memmap},
};

mod c_api;
pub mod elf;
pub mod signal;
pub mod sysimpl;
pub mod usercopy;

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

/// The process descriptor structure.
pub struct Process {
    pub pid: PID,
    flags: AtomicU32,
    wait_status: AtomicI32,
    pub argv: Vec<CString>,
    pub memmap: Memmap,
    sigtab: Mutex<Sigtab>,
    files: Mutex<BTreeMap<i32, Arc<dyn File>>>,
    tid_counter: AtomicI64,
    threads: Mutex<BTreeMap<i64, Thread>>,
}

impl Process {
    pub fn pid(&self) -> PID {
        self.pid
    }

    pub fn argv(&self) -> &[CString] {
        &self.argv
    }

    pub fn pagetable(&self) -> PPN {
        self.memmap.root_ppn()
    }

    /// Implementation of [`Self::load_executable`] for ELF files.
    fn load_executable_elf(&mut self, file: &dyn File, _recursion: u8) -> EResult<usize> {
        elf::load(file, &self.memmap, false)
    }

    /// Implementation of [`Self::load_executable`] for `#!` interpreted executables.
    fn load_executable_shebang(&mut self, file: &dyn File, recursion: u8) -> EResult<usize> {
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
            self.argv.insert(i, CString::new(word).unwrap());
            i += 1;
        }

        let interp = filesystem::open(None, self.argv[0].as_bytes(), oflags::READ_ONLY)?;
        self.load_executable(interp.as_ref(), recursion + 1)
    }

    /// Load the binary for a process and prepare for its execution.
    fn load_executable(&mut self, file: &dyn File, recursion: u8) -> EResult<usize> {
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
            self.load_executable_elf(file, recursion)
        } else if magic_len >= 2 && magic[0] == b'#' && magic[1] == b'!' {
            self.load_executable_shebang(file, recursion)
        } else {
            Err(Errno::ENOEXEC)
        }
    }

    /// Create the init process.
    pub fn new_init() -> EResult<Arc<Process>> {
        // This assert enforces init isn't accidentally created twice.
        assert!(PID_COUNTER.fetch_add(1, Ordering::Relaxed) == 1);

        let init_path = b"/sbin/init";
        let file = filesystem::open(None, init_path, oflags::READ_ONLY)?;
        let mut proc = Process {
            pid: 1,
            memmap: Memmap::new_user()?,
            files: Mutex::new(BTreeMap::new()),
            argv: Vec::try_with_capacity(1)?,
            tid_counter: AtomicI64::new(0),
            threads: Mutex::new(BTreeMap::new()),
            flags: AtomicU32::new(0),
            wait_status: AtomicI32::new(0),
            sigtab: Mutex::new(Sigtab::default()),
        };
        proc.argv.push(CString::new(init_path).unwrap());
        let entry = proc.load_executable(file.as_ref(), 0)?;

        let proc = Arc::try_new(proc)?;
        PROCESSES.lock().insert(1, proc.clone());

        proc.create_thread(entry, 0, Some("main"))?;

        logkf!(LogLevel::Debug, "Process {} started", proc.pid);

        Ok(proc)
    }

    /// Fork this process.
    pub fn fork(&self) -> EResult<Arc<Process>> {
        let pid = PID_COUNTER.fetch_add(1, Ordering::Relaxed);

        let child = Process {
            pid,
            flags: AtomicU32::new(0),
            wait_status: AtomicI32::new(0),
            argv: self.argv.clone(),
            memmap: self.memmap.fork()?,
            sigtab: Mutex::new(*self.sigtab.lock_shared()),
            files: self.files.clone(),
            tid_counter: AtomicI64::new(self.tid_counter.load(Ordering::Relaxed)),
            threads: Mutex::new(BTreeMap::new()),
        };
        let child = Arc::try_new(child)?;
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

    /// Reclaim this process' resources.
    fn reap(&self) {
        debug_assert!(self.flags.load(Ordering::Relaxed) & flags::STOPPING != 0);

        let mut threads = self.threads.lock();
        while let Some((_, thread)) = threads.pop_first() {
            thread.join();
        }
        drop(threads);

        logkf!(LogLevel::Debug, "Process {} stopped", self.pid);

        self.files.lock().clear();
        unsafe { self.memmap.clear() };
    }

    /// Create a new thread in this process.
    pub fn create_thread(&self, entry: usize, arg: usize, name: Option<&str>) -> EResult<i64> {
        let tid = self.tid_counter.fetch_add(1, Ordering::Relaxed);
        let thread = unsafe { Thread::try_new_user(self, entry, arg, name)? };
        self.threads.lock().insert(tid, thread);
        Ok(tid)
    }

    /// If `fileno` is [`AT_FDCWD`], return `Ok(None)`; otherwise, the same as [`Self::get_file`].
    pub fn get_atfile(&self, fileno: i32) -> EResult<Option<Arc<dyn File>>> {
        if fileno == AT_FDCWD {
            Ok(None)
        } else {
            Ok(Some(self.get_file(fileno)?))
        }
    }

    /// Get a file from the file descriptor table.
    pub fn get_file(&self, fileno: i32) -> EResult<Arc<dyn File>> {
        self.files
            .lock_shared()
            .get(&fileno)
            .cloned()
            .ok_or(Errno::EBADF)
    }

    /// Replace a file descriptor entry.
    pub fn replace_file(&self, fileno: i32, file: Arc<dyn File>) -> EResult<()> {
        if fileno < 0 || fileno >= FILE_MAX {
            return Err(Errno::EMFILE);
        }
        self.files
            .lock()
            .insert(fileno, file)
            .map(|_| ())
            .ok_or(Errno::EBADF)
    }

    /// Insert a file into an empty slot of the file descriptor table.
    pub fn insert_file(&self, file: Arc<dyn File>) -> EResult<i32> {
        let mut guard = self.files.lock();
        let mut fileno = Err(Errno::EMFILE);
        for i in 0..FILE_MAX {
            if !guard.contains_key(&i) {
                fileno = Ok(i);
                break;
            }
        }
        let fileno = fileno?;
        guard.insert(fileno, file);
        Ok(fileno)
    }

    /// Insert two files into an empty slot of the file descriptor table.
    pub fn insert_dual_file(
        &self,
        file0: Arc<dyn File>,
        file1: Arc<dyn File>,
    ) -> EResult<(i32, i32)> {
        let mut guard = self.files.lock();

        let mut fileno0 = Err(Errno::EMFILE);
        let mut fileno1 = Err(Errno::EMFILE);
        for i in 0..FILE_MAX {
            if !guard.contains_key(&i) {
                if fileno0.is_err() {
                    fileno0 = Ok(i);
                } else {
                    fileno1 = Ok(i);
                    break;
                }
            }
        }

        let fileno0 = fileno0?;
        let fileno1 = fileno1?;

        guard.insert(fileno0, file0);
        guard.insert(fileno1, file1);

        Ok((fileno0, fileno1))
    }

    /// Remove a file descriptor from the file descriptor table.
    pub fn remove_file(&self, fileno: i32) -> EResult<()> {
        self.files
            .lock()
            .remove(&fileno)
            .map(|_| ())
            .ok_or(Errno::EBADF)
    }
}

/// Get the current process handle.
pub fn current() -> Option<&'static Process> {
    unsafe {
        let thread = bindings::raw::sched_current_thread();
        if thread.is_null() {
            return None;
        }
        let proc = (*thread).process as *const Process;
        proc.as_ref()
    }
}
