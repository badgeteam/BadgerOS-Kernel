
// #include "syscall.h"

// #define NANOPRINTF_IMPLEMENTATION
// #define NANOPRINTF_USE_FIELD_WIDTH_FORMAT_SPECIFIERS 1
// #define NANOPRINTF_USE_PRECISION_FORMAT_SPECIFIERS   0
// #define NANOPRINTF_USE_FLOAT_FORMAT_SPECIFIERS       0
// #define NANOPRINTF_USE_LARGE_FORMAT_SPECIFIERS       0
// #define NANOPRINTF_USE_SMALL_FORMAT_SPECIFIERS       1
// #define NANOPRINTF_USE_BINARY_FORMAT_SPECIFIERS      0
// #define NANOPRINTF_USE_WRITEBACK_FORMAT_SPECIFIERS   0
// #define NANOPRINTF_USE_ALT_FORM_FLAG                 1
// #include "nanoprintf.h"



// static void putchar_impl(int c, void *ignored) {
//     (void)ignored;
//     char c_char = (char)c;
//     syscall_temp_write(&c_char, 1);
// }

// __attribute__((format(printf, 1, 2))) void printf(char const *fmt, ...) {
//     va_list vl;
//     va_start(vl, fmt);
//     npf_vpprintf(putchar_impl, NULL, fmt, vl);
//     va_end(vl);
// }



// #ifdef __riscv
// #pragma GCC disagnostic "-Wno-unused-parameter"
// #define SYSCALL_DEF(no, enum, name, returns, ...) \
//     __attribute__((naked)) returns name(__VA_ARGS__) { \
//         asm volatile("li a7, %0; ecall; ret" ::"i"(no)); \
//     }
// #include "syscall_defs.inc"
// #endif

// #ifdef __x86_64__
// #pragma GCC disagnostic "-Wno-unused-parameter"
// #define SYSCALL_DEF(no, enum, name, returns, ...) \
//     __attribute__((naked)) returns name(__VA_ARGS__) { \
//         asm volatile("mov r10, rcx; mov rax, %0; syscall" ::"i"(no)); \
//     }
// #include "syscall_defs.inc"
// #endif
