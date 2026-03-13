
// SPDX-License-Identifier: MIT

#pragma once

#include "attributes.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define FREQUENCY_HZ_MIN INT32_MIN
#define FREQUENCY_HZ_MAX INT32_MAX
#define TIMER_VALUE_MIN  INT64_MIN
#define TIMER_VALUE_MAX  INT64_MAX

typedef int32_t frequency_hz_t;
typedef int64_t timer_value_t;

// Initialise timer and watchdog subsystem.
void timer_init();
// Get the number of hardware timers.
int  timer_count() CONST;
// Get the IRQ number for a timer.
int  timer_get_irq(int timerno);
// Set the counting frequency of a hardware timer.
void timer_set_freq(int timerno, frequency_hz_t frequency);

// Configure and enable timer alarm.
void          timer_alarm_config(int timerno, timer_value_t threshold, bool reset_on_alarm);
// Disable timer alarm.
void          timer_alarm_disable(int timerno);
// Get the current value of timer.
timer_value_t timer_value_get(int timerno);
// Set the current value of timer.
void          timer_value_set(int timerno, timer_value_t value);
// Enable the timer counting.
void          timer_start(int timerno);
// Disable the timer counting.
void          timer_stop(int timerno);

// Check whether timer has interrupts enabled.
bool timer_int_enabled(int timerno);
// Enable / disable timer interrupts.
void timer_int_enable(int timerno, bool enable);
// Check whether timer interrupt had fired.
bool timer_int_pending(int timerno);
// Clear timer interrupt.
void timer_int_clear(int timerno);
