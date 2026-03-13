
// SPDX-License-Identifier: MIT

#include <stdint.h>



// Output a byte to an I/O port
__attribute__((always_inline)) static inline void outb(uint16_t port, uint8_t value) {
    asm volatile("out dx, al" : : "d"(port), "a"(value));
}

// Input a byte from an I/O port
__attribute__((always_inline)) static inline uint8_t inb(uint16_t port) {
    uint8_t value;
    asm volatile("in al, dx" : "=a"(value) : "d"(port));
    return value;
}



// Output a word to an I/O port
__attribute__((always_inline)) static inline void outw(uint16_t port, uint16_t value) {
    asm volatile("out dx, ax" : : "d"(port), "a"(value));
}

// Input a word from an I/O port
__attribute__((always_inline)) static inline uint16_t inw(uint16_t port) {
    uint8_t value;
    asm volatile("in ax, dx" : "=a"(value) : "d"(port));
    return value;
}



// Output a dword to an I/O port
__attribute__((always_inline)) static inline void outd(uint16_t port, uint32_t value) {
    asm volatile("out dx, eax" : : "d"(port), "a"(value));
}

// Input a dword from an I/O port
__attribute__((always_inline)) static inline uint32_t ind(uint16_t port) {
    uint8_t value;
    asm volatile("in eax, dx" : "=a"(value) : "d"(port));
    return value;
}
