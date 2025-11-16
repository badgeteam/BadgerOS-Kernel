
// SPDX-License-Identifier: MIT

#include "scheduler/scheduler.h"

#include "arrays.h"
#include "assertions.h"
#include "badge_strings.h"
#include "cpu/interrupt.h"
#include "cpu/isr.h"
#include "cpulocal.h"
#include "housekeeping.h"
#include "interrupt.h"
#include "isr_ctx.h"
#include "list.h"
#include "log.h"
#include "mem/vmm.h"
#include "page_alloc.h"
#include "panic.h"
#include "process/sighandler.h"
#include "process/types.h"
#include "rcu.h"
#include "scheduler/cpu.h"
#include "scheduler/isr.h"
#include "scheduler/types.h"
#include "smp.h"
#include "spinlock.h"
#include "time.h"

#include <malloc.h>



// Number of CPUs with running schedulers.
atomic_int               running_sched_count;
// Maximum CPU to check for work stealing / handoff.
static atomic_int        max_cpu_up = 1;
// CPU-local scheduler structs.
static sched_cpulocal_t *cpu_ctx[64];
// Threads list mutex.
static spinlock_t        threads_lock = SPINLOCK_T_INIT_SHARED;
// Number of threads that exist.
static size_t            threads_len;
// Capacity for thread list.
static size_t            threads_cap;
// Array of all threads that exist.
static sched_thread_t  **threads;
// Thread ID counter.
static atomic_int        tid_counter = 1;
// Unused thread pool mutex.
static spinlock_t        unused_lock;
// Pool of unused thread handles.
static dlist_t           dead_threads;



// Remove the current thread from the runqueue from this CPU.
// Interrupts must be disabled.
sched_thread_t *thread_dequeue_self() {
    isr_ctx_t        *kctx = isr_ctx_get();
    sched_cpulocal_t *info = kctx->cpulocal->sched;
    sched_thread_t   *self = kctx->thread;
    bool              ie   = irq_disable();
    spinlock_take(&info->queue_lock);
    dlist_remove(&info->queue, &self->node);
    spinlock_release(&info->queue_lock);
    irq_enable_if(ie);
    return self;
}

// Hand a thread off to another CPU.
// The thread must not yet be in any runqueue and interrupts must be disabled.
static void thread_handoff(sched_thread_t *thread, int cpu) {
    spinlock_take(&cpu_ctx[cpu]->queue_lock);
    dlist_prepend(&cpu_ctx[cpu]->queue, &thread->node);
    spinlock_release(&cpu_ctx[cpu]->queue_lock);
}

// Set the context switch to a certain thread.
static void set_switch(sched_cpulocal_t *info, sched_thread_t *thread) {
    int pflags = thread->process ? atomic_load(&thread->process->flags) : 0;
    int tflags = atomic_load(&thread->flags);

    // Check for pending signals.
    if (!(tflags & (THREAD_PRIVILEGED | THREAD_SIGHANDLER)) && (pflags & PROC_SIGPEND)) {
        // Process has pending signals to handle first.
        sched_raise_from_isr(thread, false, proc_signal_handler);
    }

    // Set context switch target.
    isr_ctx_t *next = (tflags & THREAD_PRIVILEGED) ? &thread->kernel_isr_ctx : &thread->user_isr_ctx;
    next->cpulocal  = isr_ctx_get()->cpulocal;
    isr_ctx_switch_set(next);

    // Set preemption timer.
    timestamp_us_t now     = time_us();
    timestamp_us_t timeout = now + SCHED_MIN_US + SCHED_INC_US * thread->priority;
    if (timeout > info->load_measure_time) {
        timeout = info->load_measure_time;
    }
    info->last_preempt = now;
    time_set_next_task_switch(timeout);

    assert_dev_drop(!!(tflags & THREAD_PRIVILEGED) == !!(next->flags & ISR_CTX_FLAG_KERNEL));
    rcu_sched_func_callback(&next->cpulocal->rcu);

    // Run arch-specific pre-task-switch code.
    sched_arch_task_switch(thread);
}

// Handle non-normal scheduler flags.
static void sw_handle_sched_flags(timestamp_us_t now, int cur_cpu, sched_cpulocal_t *info, int sched_fl) {
    (void)now;

    if (sched_fl & SCHED_DUMPSTATE) {
        sched_debug_print();
        atomic_fetch_and_explicit(&info->flags, ~SCHED_DUMPSTATE, memory_order_relaxed);
    }

    if (!(sched_fl & (SCHED_RUNNING | SCHED_STARTING))) {
        // Mark as starting in the first cycle.
        atomic_fetch_or(&info->flags, SCHED_STARTING);
        atomic_fetch_add_explicit(&running_sched_count, 1, memory_order_relaxed);
        rcu_sched_init_callback(&isr_ctx_get()->cpulocal->rcu);

    } else if (sched_fl & SCHED_STARTING) {
        // Mark as running afterwards so CPU0 can free the stack.
        atomic_fetch_xor_explicit(&info->flags, SCHED_RUNNING | SCHED_STARTING, memory_order_relaxed);

    } else if (sched_fl & SCHED_EXITING) {
        // Exit the scheduler on this CPU.
        spinlock_take(&info->run_lock);
        atomic_store_explicit(&info->load_average, 0, memory_order_relaxed);
        atomic_store_explicit(&info->load_estimate, 0, memory_order_relaxed);
        atomic_fetch_sub_explicit(&running_sched_count, 1, memory_order_relaxed);
        atomic_fetch_and_explicit(&info->flags, ~(SCHED_RUNNING | SCHED_EXITING), memory_order_relaxed);
        rcu_sched_func_callback(&isr_ctx_get()->cpulocal->rcu);

        // Hand all threads over to other CPUs.
        spinlock_take(&info->queue_lock);
        int max = atomic_load_explicit(&max_cpu_up, memory_order_acquire);
        for (int cpu = 0; info->queue.len && cpu < max; cpu++) {
            spinlock_take(&cpu_ctx[cpu]->queue_lock);
            if (atomic_load_explicit(&cpu_ctx[cpu]->flags, memory_order_relaxed) & SCHED_RUNNING) {
                // This CPU has a running scheduler and we can simply dump all threads there.
                dlist_concat(&cpu_ctx[cpu]->queue, &info->queue);
            }
            spinlock_release(&cpu_ctx[cpu]->queue_lock);
        }
        if (info->queue.len) {
            logkf_from_isr(LOG_FATAL, "No valid CPU to hand over threads to");
            panic_abort();
        }
        spinlock_release(&info->queue_lock);
        spinlock_release(&info->run_lock);

        // Power off this CPU.
        assert_dev_keep(smp_poweroff());
    }
}

// Measure load on this CPU.
static void sw_measure_load(sched_cpulocal_t *info) {
    spinlock_take_shared(&info->queue_lock);

    // Measure time usage.
    timestamp_us_t  used_time = 0;
    sched_thread_t *thread    = (sched_thread_t *)info->queue.head;
    while (thread) {
        used_time += thread->timeusage.cycle_time;
        thread     = (sched_thread_t *)thread->node.next;
    }

    timestamp_us_t idle_time               = info->idle_thread.timeusage.cycle_time;
    info->idle_thread.timeusage.cycle_time = 0;
    timestamp_us_t total_time              = used_time + idle_time;

    // Account per-thread CPU usage.
    int total_load = 0;
    thread         = (sched_thread_t *)info->queue.head;
    while (thread) {
        timestamp_us_t cpu_time       = thread->timeusage.cycle_time;
        thread->timeusage.cycle_time  = 0;
        int cpu_permil                = (int)(cpu_time * 10000 / total_time);
        total_load                   += cpu_permil;
        atomic_store(&thread->timeusage.cpu_usage, cpu_permil);
        thread = (sched_thread_t *)thread->node.next;
    }

    info->load_average  = total_load;
    info->load_estimate = total_load;

    spinlock_release_shared(&info->queue_lock);
}

// Requests the scheduler to prepare a switch from inside an interrupt routine.
void sched_request_switch_from_isr() {
    check_for_panic();
    assert_dev_drop(!irq_is_enabled());
    timestamp_us_t    now     = time_us();
    int               cur_cpu = smp_cur_cpu();
    sched_cpulocal_t *info    = cpu_ctx[cur_cpu];

    // Check the exiting flag.
    int sched_fl = atomic_load(&info->flags);
    if (sched_fl != SCHED_RUNNING) {
        sw_handle_sched_flags(now, cur_cpu, info, sched_fl);
    }

    // Account thread time usage.
    sched_thread_t *cur_thread = sched_current_thread();
    if (cur_thread) {
        timestamp_us_t used               = now - info->last_preempt;
        cur_thread->timeusage.cycle_time += used;
        if (cur_thread->flags & THREAD_PRIVILEGED) {
            cur_thread->timeusage.kernel_time += used;
        } else {
            cur_thread->timeusage.user_time += used;
        }
    }

    // Check for load measurement timer.
    // Ignored when timestamp is 0 in case timers aren't active yet.
    if (now && now >= info->load_measure_time) {
        // Measure load on this CPU.
        sw_measure_load(info);

        // Set next timestamp to measure load average.
        info->load_measure_time = now + SCHED_LOAD_INTERVAL - (now % SCHED_LOAD_INTERVAL);
    }

    // Check for runnable threads.
    spinlock_take(&info->queue_lock);
    while (info->queue.len) {
        // Take the first thread.
        sched_thread_t *thread = (void *)dlist_pop_front(&info->queue);
        int             flags  = atomic_load(&thread->flags);

        // Check for thread exit conditions.
        bool kill_thread = flags & THREAD_EXITING;
        if (thread->process && (atomic_load(&thread->process->flags) & PROC_EXITING)) {
            kill_thread |= !(flags & THREAD_PRIVILEGED);
        }

        if (kill_thread) {
            // Exiting thread/process; clean up thread.
            spinlock_take(&unused_lock);
            atomic_fetch_or(&thread->flags, THREAD_EXITED);
            atomic_fetch_and(&thread->flags, ~(THREAD_RUNNING | THREAD_EXITING));
            dlist_append(&dead_threads, &thread->node);
            spinlock_release(&unused_lock);

        } else if (((flags & THREAD_KSUSPEND) || !(flags & THREAD_PRIVILEGED)) && (flags & THREAD_SUSPENDING)) {
            // Userspace and/or kernel thread being suspended.
            int newval;
            do {
                if (!((flags & THREAD_KSUSPEND) || !(flags & THREAD_PRIVILEGED)) || !(flags & THREAD_SUSPENDING)) {
                    // Suspend cancelled; set as switch target.
                    dlist_append(&info->queue, &thread->node);
                    spinlock_release(&info->queue_lock);
                    set_switch(info, thread);
                    return;
                }
                newval = flags & ~(THREAD_RUNNING | THREAD_KSUSPEND | THREAD_SUSPENDING);
            } while (!atomic_compare_exchange_strong(&thread->flags, &flags, newval));

        } else {
            // Runnable thread found; perform context switch.
            assert_dev_drop(flags & THREAD_RUNNING);
            dlist_append(&info->queue, &thread->node);
            spinlock_release(&info->queue_lock);
            set_switch(info, thread);
            return;
        }
    }
    spinlock_release(&info->queue_lock);

    // If nothing is running on this CPU, run the idle thread.
    set_switch(info, &info->idle_thread);
}



// Allocate a kernel stack.
// Returns the stack bottom address.
size_t sched_alloc_stack() {
    ppn_t ppn = phys_page_alloc(CONFIG_STACK_SIZE / CONFIG_PAGE_SIZE, false);
    if (!ppn)
        goto err0;

    vpn_t vpn;
    if (vmm_map_k(&vpn, CONFIG_STACK_SIZE / CONFIG_PAGE_SIZE, ppn, VMM_FLAG_RW) < 0)
        goto err1;

    return vpn * CONFIG_PAGE_SIZE;

err1:
    phys_page_free(ppn);
err0:
    return 0;
}

// Free a kernel stack.
// Takes the stack bottom address.
void sched_free_stack(size_t vaddr) {
    virt2phys_t res = vmm_virt2phys(NULL, vaddr);
    vmm_unmap_k(vaddr / CONFIG_PAGE_SIZE, CONFIG_STACK_SIZE / CONFIG_PAGE_SIZE);
    phys_page_free(res.paddr / CONFIG_PAGE_SIZE);
}



// Compare the ID of `sched_thread_t *` to an `int`.
static int tid_int_cmp(void const *a, void const *b) {
    sched_thread_t *thread = *(sched_thread_t **)a;
    tid_t           tid    = (tid_t)(ptrdiff_t)b;
    return thread->id - tid;
}

// Find a thread by TID.
static sched_thread_t *find_thread(tid_t tid) {
    array_binsearch_t res = array_binsearch(threads, sizeof(void *), threads_len, (void *)(ptrdiff_t)tid, tid_int_cmp);
    return res.found ? threads[res.index] : NULL;
}

// Scheduler housekeeping.
static void sched_housekeeping(int taskno, void *arg) {
    (void)taskno;
    (void)arg;

    irq_disable();
    spinlock_take(&threads_lock);

    // Get list of dead threads.
    spinlock_take(&unused_lock);
    dlist_t         tmp  = DLIST_EMPTY;
    sched_thread_t *node = (void *)dead_threads.head;
    while (node) {
        void *next = (void *)node->node.next;
        if (atomic_load(&node->flags) & THREAD_DETACHED) {
            dlist_remove(&dead_threads, &node->node);
            dlist_append(&tmp, &node->node);
        }
        node = next;
    }
    spinlock_release(&unused_lock);

    // Threads must be removed from the list before being properly reaped because that is preemptible.
    dlist_foreach_node(sched_thread_t, thread, &tmp) {
        array_binsearch_t res =
            array_binsearch(threads, sizeof(void *), threads_len, (void *)(ptrdiff_t)thread->id, tid_int_cmp);
        assert_dev_drop(res.found);
        array_lencap_remove(&threads, sizeof(void *), &threads_len, &threads_cap, NULL, res.index);
    }

    spinlock_release(&threads_lock);
    irq_enable();

    while (tmp.len) {
        sched_thread_t *thread = (void *)dlist_pop_front(&tmp);
        sched_free_stack(thread->kernel_stack_bottom);
        if (thread->name) {
            free(thread->name);
        }
    }
}

// Idle function ran when a CPU has no threads.
static void idle_func(void *arg) {
    (void)arg;
    sched_cpulocal_t *info     = isr_ctx_get()->cpulocal->sched;
    int               this_cpu = smp_cur_cpu();
    while (1) {
        timestamp_us_t now = time_us();

        // Try to steal work from other CPUs.
        int max = atomic_load_explicit(&max_cpu_up, memory_order_acquire);
        for (int cpu = 0; cpu < max; cpu++) {
            if (cpu == this_cpu) {
                continue;
            }
            irq_disable();
            spinlock_take(&cpu_ctx[cpu]->queue_lock);
            sched_thread_t *head = (sched_thread_t *)cpu_ctx[cpu]->queue.head;

            if (cpu_ctx[cpu]->queue.len >= 2 && head->last_migration + SCHED_WORK_STEAL_INTERVAL < now) {
                // Eligible thread found; remove from queue.
                dlist_pop_front(&cpu_ctx[cpu]->queue);
                head->last_migration = now;
                spinlock_release(&cpu_ctx[cpu]->queue_lock);

                // Add to local queue.
                spinlock_take(&info->queue_lock);
                dlist_prepend(&info->queue, &head->node);
                spinlock_release(&info->queue_lock);

                // Break to yield and hopefully start the thread.
                irq_enable();
                break;
            }

            spinlock_release(&cpu_ctx[cpu]->queue_lock);
            irq_enable();
        }

        thread_yield();
        isr_pause();
    }
}

// Global scheduler initialization.
void sched_init() {
    hk_add_repeated_presched(0, 1000000, sched_housekeeping, NULL);
}

// Power on and start scheduler on secondary CPUs.
void sched_start_altcpus() {
    int cpu = smp_cur_cpu();
    if (smp_count > 1) {
        logkf(LOG_INFO, "Starting scheduler on %{d} alt CPU(s)", smp_count - 1);
        for (int i = 0; i < smp_count; i++) {
            if (i != cpu) {
                sched_init_cpu(i);
            }
        }
        atomic_store_explicit(&max_cpu_up, smp_count, memory_order_release);
        for (int i = 0; i < smp_count; i++) {
            if (i != cpu) {
                sched_start_on(i);
            }
        }
    }
}

// Power on and start scheduler on another CPU.
bool sched_start_on(int cpu) {
    static mutex_t start_mutex = MUTEX_T_INIT;
    mutex_acquire(&start_mutex, TIMESTAMP_US_MAX);

    // Tell SMP to power on the other CPU.
    void *tmp_stack  = malloc(CONFIG_STACK_SIZE);
    bool  poweron_ok = smp_poweron(cpu, sched_exec, tmp_stack + CONFIG_STACK_SIZE);
    if (poweron_ok) {
        mutex_acquire(&log_mtx, TIMESTAMP_US_MAX);
        while (!(atomic_load(&cpu_ctx[cpu]->flags) & SCHED_RUNNING)) continue;
        mutex_release(&log_mtx);
    } else {
        logkf(LOG_ERROR, "Starting CPU%{d} failed", cpu);
    }

    free(tmp_stack);
    mutex_release(&start_mutex);

    return poweron_ok;
}

// Prepare a new scheduler context for this or another CPU.
void sched_init_cpu(int cpu) {
    sched_cpulocal_t *info = calloc(1, sizeof(sched_cpulocal_t));

    // Prepare CPU-local data.
    // TODO: There is no good way to detect whether a scheduler exists on a CPU because of this line.
    // It isn't protected by anything and fills out an important pointer.
    // Maybe use an atomic integer to store whether a scheduler is up?
    // Even that isn't a perfect solution because in the future, BadgerOS needs to support shutting down schedulers on a
    // subset of the CPUs. It might even be necessary to statically allocate the flags and spinlocks, or to allocate the
    // CPU contexts once on `sched_start_altcpus` (before any secondary CPUs are brought up) and never clean them.
    cpu_ctx[cpu]     = info;
    info->run_lock   = SPINLOCK_T_INIT_SHARED;
    info->queue_lock = SPINLOCK_T_INIT_SHARED;

    // Prepare idle thread.
    void *stack = malloc(8192);
    assert_always(stack);
    info->idle_thread.kernel_stack_bottom  = (size_t)stack;
    info->idle_thread.kernel_stack_top     = (size_t)stack + 8192;
    info->idle_thread.kernel_isr_ctx.flags = ISR_CTX_FLAG_KERNEL;
    info->idle_thread.flags                = THREAD_PRIVILEGED;
    sched_prepare_kernel_entry(&info->idle_thread, idle_func, NULL);

    // Fence memory so other CPUs see it.
    atomic_thread_fence(memory_order_release);
}

// Thread function that reports the scheduler to be up and exits.
static int sched_up_reporter(void *ctx) {
    logkf(LOG_INFO, "Scheduler started on CPU%{d}", (int)(size_t)ctx);
    return 0;
}

// Start executing the scheduler on this CPU.
NORETURN void sched_exec() {
    timestamp_us_t now = time_us();
    int            cpu = smp_cur_cpu();

    // Get CPU-local scheduler data.
    sched_cpulocal_t *info         = cpu_ctx[cpu];
    isr_ctx_get()->cpulocal->sched = info;
    tid_t tid                      = thread_new_kernel(NULL, sched_up_reporter, (void *)(size_t)cpu, SCHED_PRIO_NORMAL);
    thread_resume_now(tid);
    thread_detach(tid);

    // Set next timestamp to measure load average.
    info->load_average      = 0;
    info->load_estimate     = 0;
    info->load_measure_time = now + SCHED_LOAD_INTERVAL - (now % SCHED_LOAD_INTERVAL);
    atomic_store_explicit(&info->flags, 0, memory_order_release);

    // Schedule the first thread and switch context to it, destroying the temporary context int the process.
    // This will start handed over threads or idle until one is handed over to this CPU.
    sched_request_switch_from_isr();
    isr_context_switch();
    assert_unreachable();
}

// Exit the scheduler and subsequenty shut down the CPU.
void sched_exit(int cpu) {
    spinlock_take(&cpu_ctx[cpu]->run_lock);
    atomic_fetch_or_explicit(&cpu_ctx[cpu]->flags, SCHED_EXITING, memory_order_relaxed);
    spinlock_release(&cpu_ctx[cpu]->run_lock);
}



// Returns the current thread ID.
tid_t sched_current_tid() {
    return isr_ctx_get()->thread->id;
}

// Returns the current thread struct.
sched_thread_t *sched_current_thread() {
    return isr_ctx_get()->thread;
}

// Returns the associated thread struct.
sched_thread_t *sched_get_thread(tid_t tid) {
    bool ie = irq_disable();
    spinlock_take_shared(&threads_lock);
    sched_thread_t *thread = find_thread(tid);
    spinlock_release_shared(&threads_lock);
    irq_enable_if(ie);
    return thread;
}


// Create a new suspended userland thread.
tid_t thread_new_user(char const *name, process_t *process, size_t user_entrypoint, size_t user_arg, int priority) {
    // Allocate thread.
    sched_thread_t *thread = malloc(sizeof(sched_thread_t));
    if (!thread) {
        return -ENOMEM;
    }
    mem_set(thread, 0, sizeof(sched_thread_t));

    thread->kernel_stack_bottom = sched_alloc_stack();
    if (!thread->kernel_stack_bottom) {
        free(thread);
        return -ENOMEM;
    }

    if (name) {
        size_t name_len = cstr_length(name);
        thread->name    = malloc(name_len + 1);
        if (!thread->name) {
            sched_free_stack(thread->kernel_stack_bottom);
            free(thread);
            return -ENOMEM;
        }
        cstr_copy(thread->name, name_len + 1, name);
    }

    thread->priority              = priority;
    thread->process               = process;
    thread->id                    = atomic_fetch_add(&tid_counter, 1);
    thread->blocking_lock         = SPINLOCK_T_INIT;
    thread->kernel_stack_top      = thread->kernel_stack_bottom + CONFIG_STACK_SIZE;
    thread->kernel_isr_ctx.flags  = ISR_CTX_FLAG_KERNEL;
    thread->kernel_isr_ctx.thread = thread;
    thread->user_isr_ctx.thread   = thread;
    thread->user_isr_ctx.mem_ctx  = &process->memmap.mem_ctx;
    sched_prepare_user_entry(thread, user_entrypoint, user_arg);

    irq_disable();
    spinlock_take(&threads_lock);
    bool success = array_lencap_insert(&threads, sizeof(void *), &threads_len, &threads_cap, &thread, threads_len);
    spinlock_release(&threads_lock);
    irq_enable();
    if (!success) {
        if (thread->name) {
            free(thread->name);
        }
        sched_free_stack(thread->kernel_stack_bottom);
        free(thread);
        return -ENOMEM;
    }

    return thread->id;
}

// Create new suspended kernel thread.
tid_t thread_new_kernel(char const *name, sched_entry_t entrypoint, void *arg, int priority) {
    // Allocate thread.
    sched_thread_t *thread = malloc(sizeof(sched_thread_t));
    if (!thread) {
        return -ENOMEM;
    }
    mem_set(thread, 0, sizeof(sched_thread_t));

    thread->kernel_stack_bottom = sched_alloc_stack();
    if (!thread->kernel_stack_bottom) {
        free(thread);
        return -ENOMEM;
    }

    if (name) {
        size_t name_len = cstr_length(name);
        thread->name    = malloc(name_len + 1);
        if (!thread->name) {
            sched_free_stack(thread->kernel_stack_bottom);
            free(thread);
            return -ENOMEM;
        }
        cstr_copy(thread->name, name_len + 1, name);
    }

    thread->priority               = priority;
    thread->id                     = atomic_fetch_add(&tid_counter, 1);
    thread->blocking_lock          = SPINLOCK_T_INIT;
    thread->kernel_stack_top       = thread->kernel_stack_bottom + CONFIG_STACK_SIZE;
    thread->kernel_isr_ctx.flags   = ISR_CTX_FLAG_KERNEL;
    thread->kernel_isr_ctx.thread  = thread;
    thread->flags                 |= THREAD_PRIVILEGED | THREAD_KERNEL;
    sched_prepare_kernel_entry(thread, entrypoint, arg);

    bool ie = irq_disable();
    spinlock_take(&threads_lock);
    bool success = array_lencap_insert(&threads, sizeof(void *), &threads_len, &threads_cap, &thread, threads_len);
    spinlock_release(&threads_lock);
    irq_enable_if(ie);
    if (!success) {
        if (thread->name) {
            free(thread->name);
        }
        sched_free_stack(thread->kernel_stack_bottom);
        free(thread);
        return -ENOMEM;
    }

    // logkf(LOG_DEBUG, "Kernel thread #%{d} '%{cs}' @0x%{size;x} created", thread->id, thread->name, thread);

    return thread->id;
}

// Do not wait for thread to be joined; clean up immediately.
errno_t thread_detach(tid_t tid) {
    bool ie = irq_disable();
    spinlock_take_shared(&threads_lock);
    sched_thread_t *thread = find_thread(tid);
    errno_t         res;
    if (thread) {
        atomic_fetch_or(&thread->flags, THREAD_DETACHED);
        res = 0;
    } else {
        res = -ENOENT;
    }
    spinlock_release_shared(&threads_lock);
    irq_enable_if(ie);
    return res;
}


// Explicitly yield to the scheduler; the scheduler may run other threads without waiting for preemption.
// Use this function to reduce the CPU time used by a thread.
void thread_yield() {
    irq_disable();
    sched_request_switch_from_isr();
    isr_context_switch();
}

// Resume a thread from a timer ISR.
static void thread_resume_from_timer(void *cookie0) {
    struct {
        tid_t    tid;
        uint64_t ticket;
    } *cookie = cookie0;
    thread_unblock(cookie->tid, cookie->ticket);
}

// Sleep for an amount of microseconds.
void thread_sleep(timestamp_us_t delay) {
    irq_disable();
    struct {
        tid_t    tid;
        uint64_t ticket;
    } cookie = {
        sched_current_tid(),
        thread_block(),
    };
    timertask_t task = {
        .callback  = thread_resume_from_timer,
        .cookie    = &cookie,
        .timestamp = time_us() + delay,
    };
    time_add_async_task(&task);
    thread_yield();
}

// Block this thread and return a blocking ticket.
uint64_t thread_block() {
    assert_dev_drop(!irq_is_enabled());

    sched_thread_t *self = thread_dequeue_self();
    spinlock_take(&self->blocking_lock);

    self->unblock_cpu = smp_cur_cpu();
    uint64_t ticket   = ++self->blocking_ticket;
    atomic_fetch_or(&self->flags, THREAD_BLOCKED);

    spinlock_release(&self->blocking_lock);

    return ticket;
}

// Unblock a thread.
bool thread_unblock(tid_t tid, uint64_t ticket) {
    bool ie = irq_disable();
    spinlock_take_shared(&threads_lock);

    sched_thread_t *thread  = find_thread(tid);
    bool            success = false;

    if (thread) {
        spinlock_take(&thread->blocking_lock);

        success =
            thread->blocking_ticket == ticket && (atomic_fetch_and(&thread->flags, ~THREAD_BLOCKED) & THREAD_BLOCKED);
        if (success) {
            thread_handoff(thread, thread->unblock_cpu);
        }

        spinlock_release(&thread->blocking_lock);
    }

    spinlock_release_shared(&threads_lock);
    irq_enable_if(ie);

    return success;
}



// Pauses execution of a thread.
// If `suspend_kernel` is false, the thread won't be suspended until it enters user mode.
errno_t thread_suspend(tid_t tid, bool suspend_kernel) {
    sched_thread_t *self = sched_current_thread();
    sched_thread_t *thread;

    irq_disable();
    if (tid == self->id) {
        // Suspending this thread.
        thread = self;
    } else {
        // Suspending another thread.
        spinlock_take_shared(&threads_lock);
        thread = find_thread(tid);
    }

    errno_t res;
    if (thread) {
        if ((thread->flags & THREAD_KERNEL) && !suspend_kernel) {
            res = -EINVAL;
        } else {
            int setfl = THREAD_SUSPENDING + suspend_kernel * THREAD_KSUSPEND;
            int exp   = atomic_load(&thread->flags);
            do {
                if (!(exp & THREAD_RUNNING)) {
                    break;
                }
            } while (!atomic_compare_exchange_strong(&thread->flags, &exp, exp | setfl));
            res = 0;
        }
    } else {
        res = -ENOENT;
    }

    if (tid == self->id) {
        if (suspend_kernel) {
            // Yield to suspend and implicitly re-enable IRQs.
            thread_yield();
        } else {
            // Re-enable IRQs and wait for the drop to user mode to suspend.
            irq_enable();
        }
    } else {
        // If suspending another thread, release mutex.
        spinlock_release_shared(&threads_lock);
        irq_enable();
    }

    return res;
}

// Try to mark a thread as running if a thread is allowed to be resumed.
static bool thread_try_mark_running(sched_thread_t *thread, bool now) {
    int cur = atomic_load(&thread->flags);
    int nextval;
    do {
        if (cur & (THREAD_EXITED | THREAD_EXITING | THREAD_RUNNING)) {
            return false;
        }
        nextval = (cur | THREAD_RUNNING) & ~THREAD_SUSPENDING;
        if (now) {
            nextval |= THREAD_STARTNOW;
        }
    } while (!atomic_compare_exchange_strong(&thread->flags, &cur, nextval));
    return !(cur & THREAD_RUNNING);
}

// Resumes a previously suspended thread or starts it.
static errno_t thread_resume_impl(tid_t tid, bool now) {
    bool ie = irq_disable();
    spinlock_take_shared(&threads_lock);
    sched_thread_t *thread = find_thread(tid);
    errno_t         res;
    if (thread) {
        if (thread_try_mark_running(thread, now)) {
            thread_handoff(thread, smp_cur_cpu());
        }
        res = 0;
    } else {
        res = -ENOENT;
    }
    spinlock_release_shared(&threads_lock);
    irq_enable_if(ie);
    return res;
}

// Resumes a previously suspended thread or starts it.
errno_t thread_resume(tid_t tid) {
    return thread_resume_impl(tid, false);
}

// Resumes a previously suspended thread or starts it.
// Immediately schedules the thread instead of putting it in the queue first.
errno_t thread_resume_now(tid_t tid) {
    return thread_resume_impl(tid, true);
}

// Resumes a previously suspended thread or starts it from an ISR.
errno_t thread_resume_from_isr(tid_t tid) {
    assert_dev_drop(!irq_is_enabled());
    return thread_resume_impl(tid, false);
}

// Resumes a previously suspended thread or starts it from an ISR.
// Immediately schedules the thread instead of putting it in the queue first.
errno_t thread_resume_now_from_isr(tid_t tid) {
    assert_dev_drop(!irq_is_enabled());
    return thread_resume_impl(tid, true);
}

// Returns whether a thread is running; it is neither suspended nor has it exited.
errno_bool_t thread_is_running(tid_t tid) {
    irq_disable();
    spinlock_take_shared(&threads_lock);
    sched_thread_t *thread = find_thread(tid);
    errno_bool_t    res    = false;
    if (thread) {
        res = !!(atomic_load(&thread->flags) & THREAD_RUNNING);
    } else {
        res = -ENOENT;
    }
    spinlock_release_shared(&threads_lock);
    irq_enable();
    return res;
}


// Exits the current thread.
// If the thread is detached, resources will be cleaned up.
void thread_exit(int code) {
    irq_disable();
    sched_thread_t *thread = isr_ctx_get()->thread;
    thread->exit_code      = code;
    atomic_fetch_or(&thread->flags, THREAD_EXITING);
    sched_request_switch_from_isr();
    isr_context_switch();
    assert_unreachable();
}

// Wait for another thread to exit.
int thread_join(tid_t tid) {
    // TODO: This can be done more efficiently.
    while (1) {
        irq_disable();
        spinlock_take_shared(&threads_lock);
        sched_thread_t *thread = find_thread(tid);
        if (thread) {
            if (atomic_load(&thread->flags) & THREAD_EXITED) {
                int res = thread->exit_code;
                atomic_fetch_or(&thread->flags, THREAD_DETACHED);
                spinlock_release_shared(&threads_lock);
                irq_enable();
                return res;
            }
        } else {
            spinlock_release_shared(&threads_lock);
            irq_enable();
            return -ENOENT;
        }
        spinlock_release_shared(&threads_lock);
        // No need to re-enable IRQs because the yield implicitly does so.
        thread_yield();
    }
}



// Dump scheduler state for debug purposes.
void sched_debug_print() {
    bool ie      = irq_disable();
    int  max_cpu = atomic_load(&max_cpu_up);
    spinlock_take(&threads_lock);

    for (int cpu = 0; cpu < max_cpu; cpu++) {
        spinlock_take(&cpu_ctx[cpu]->queue_lock);
    }

    logkf_from_isr(LOG_DEBUG, "%{size;d} threads:", threads_len);
    for (size_t i = 0; i < threads_len; i++) {
#ifdef __riscv
#define PC_REG pc
#else
#define PC_REG rip
#endif
        if (threads[i]->name) {
            logkf_from_isr(
                LOG_DEBUG,
                "  %{d} FLAGS=0x%{x} PC=0x%{size;x} \"%{cs}\"",
                threads[i]->id,
                threads[i]->flags,
                threads[i]->kernel_isr_ctx.regs.PC_REG,
                threads[i]->name
            );
        } else {
            logkf_from_isr(
                LOG_DEBUG,
                "  %{d} FLAGS=0x%{x} PC=0x%{size;x}",
                threads[i]->id,
                threads[i]->flags,
                threads[i]->kernel_isr_ctx.regs.PC_REG
            );
        }
#undef PC_REG
    }

    for (int cpu = 0; cpu < max_cpu; cpu++) {
        logkf_from_isr(LOG_DEBUG, "CPU%{d} has %{size;d} threads:", cpu, cpu_ctx[cpu]->queue.len);
        sched_thread_t *cur = (sched_thread_t *)cpu_ctx[cpu]->queue.head;
        while (cur) {
            logkf_from_isr(LOG_DEBUG, "  %{d}", cur->id);
            cur = (sched_thread_t *)cur->node.next;
        }
    }

    for (int cpu = 0; cpu < max_cpu; cpu++) {
        spinlock_release(&cpu_ctx[cpu]->queue_lock);
    }

    spinlock_release(&threads_lock);
    irq_enable_if(ie);
}