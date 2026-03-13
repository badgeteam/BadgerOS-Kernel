
// SPDX-License-Identifier: MIT

#include "time.h"

#include "arrays.h"
#include "assertions.h"
#include "interrupt.h"
#include "spinlock.h"
#include "time_private.h"



/*
// Spinlock for list of unclaimed timer tasks.
static spinlock_t tasks_spinlock = SPINLOCK_T_INIT_SHARED;

// Length of timer tasks array.
static size_t        tasks_len;
// Capacity of timer tasks array.
static size_t const  tasks_cap = 2048;
// Global timer tasks array.
static timertask_t **tasks;


// Timer task counter.
static int64_t taskno_counter;


// Comparator for timer tasks by timestamp.
int timertask_timestamp_cmp(void const *a_ptr, void const *b_ptr) {
    timertask_t const *a = *(void *const *)a_ptr;
    timertask_t const *b = *(void *const *)b_ptr;
    if (a->timestamp < b->timestamp) {
        return 1;
    } else if (a->timestamp > b->timestamp) {
        return -1;
    } else {
        return 0;
    }
}



// Evaluate the timer for this CPU.
static void eval_cpu_timer(time_cpulocal_t *ctx) {
    spinlock_take_shared(&tasks_spinlock);
    if (tasks_len && (ctx->preempt_time <= 0 || tasks[tasks_len - 1]->timestamp < ctx->preempt_time)) {
        // There is a timer task scheduled that will run first.
        ctx->timer_is_preempt = false;
        time_set_cpu_timer(tasks[tasks_len - 1]->timestamp);
    } else {
        // No task or the task will run after the preemption.
        ctx->timer_is_preempt = true;
        time_set_cpu_timer(ctx->preempt_time);
    }
    spinlock_release_shared(&tasks_spinlock);
}

// Sets the alarm time when the next task switch should occur.
void time_set_next_task_switch(timestamp_us_t timestamp) {
    time_cpulocal_t *ctx = &isr_ctx_get()->cpulocal->time;
    ctx->preempt_time    = timestamp;
    eval_cpu_timer(ctx);
}

// Attach a task to a timer interrupt.
// The task with the lowest timestamp is likeliest, but not guaranteed, to run first.
// Returns whether the task was successfully added.
bool time_add_async_task(timertask_t *task) {
    assert_dev_drop(task);

    // Interrupts must be disabled while holding spinlock.
    bool ie = irq_disable();
    spinlock_take(&tasks_spinlock);

    // Allocate a task number.
    task->taskno = taskno_counter++;

    // Insert into task array.
    if (tasks_len >= tasks_cap) {
        goto error;
    }
    array_sorted_insert(tasks, sizeof(timertask_t *), tasks_len, &task, timertask_timestamp_cmp);
    tasks_len++;

    // Release spinlock so it can be re-taken as shared in `eval_cpu_timer`.
    spinlock_release(&tasks_spinlock);

    // Recalculate this CPU's timer.
    time_cpulocal_t *ctx = &isr_ctx_get()->cpulocal->time;
    eval_cpu_timer(ctx);

    // Re-enable interrupts because we're done with the spinlocks.
    irq_enable_if(ie);
    return true;

error:
    // Failed to insert into array.
    spinlock_release(&tasks_spinlock);
    irq_enable_if(ie);
    return false;
}

// Cancel a task created with `time_add_async_task`.
bool time_cancel_async_task(int64_t taskno) {
    // Interrupts must be disabled while holding spinlock.
    bool ie = irq_disable();

    // Take spinlock while removing from the list.
    spinlock_take(&tasks_spinlock);

    bool found = false;
    for (size_t i = 0; i < tasks_len; i++) {
        if (tasks[i]->taskno == taskno) {
            found = true;
            array_remove(tasks, sizeof(void *), tasks_len, NULL, i);
            tasks_len--;
            break;
        }
    }

    // Release spinlock so it can be re-taken as shared in `eval_cpu_timer`.
    spinlock_release(&tasks_spinlock);

    // Recalculate this CPU's timer.
    time_cpulocal_t *ctx = &isr_ctx_get()->cpulocal->time;
    eval_cpu_timer(ctx);

    // Re-enable interrupts because we're done with the spinlocks.
    irq_enable_if(ie);
    return found;
}

// Generic timer init after timer-specific init.
void time_init_generic() {
    tasks = malloc(tasks_cap * sizeof(timertask_t *));
}

// Callback from timer-specific code when the CPU timer fires.
void time_cpu_timer_isr() {
    time_cpulocal_t *ctx = &isr_ctx_get()->cpulocal->time;
    timestamp_us_t   now = time_us();
    if (ctx->timer_is_preempt) {
        // Preemption timer.
        ctx->preempt_time = TIMESTAMP_US_MAX;
        sched_request_switch_from_isr();

    } else {
        timertask_t *task = NULL;

        // Check if there is a task that needs to run right now.
        spinlock_take(&tasks_spinlock);
        if (tasks_len && now >= tasks[tasks_len - 1]->timestamp) {
            task = tasks[--tasks_len];
        }
        spinlock_release(&tasks_spinlock);

        // Run the task, if any.
        if (task) {
            task->callback(task->cookie);
        }
    }

    // Re-evaluate this CPU's timer.
    eval_cpu_timer(ctx);
}
*/

// Generic timer init after timer-specific init.
void time_init_generic() {
}
