
// SPDX-License-Identifier: MIT

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// A generic thread-safe nonblocking FIFO safe for multi-read, multi-write use.
// To achieve the multi-read, multi-write safety, the FIFO has a "reservation" and "index" each for read and write.
// The reservation in atomically updated to reserve data in the buffer to read or write.
// When the read or write is finished, the index is updated in an ACAS loop.
typedef struct fifo fifo_t;

// Create a new FIFO.
fifo_t *fifo_create(size_t ent_size, size_t capacity);
// Destroy a FIFO.
void    fifo_destroy(fifo_t *fifo);

// Send multiple elements to a FIFO.
// Returns the actual amount of elements sent.
size_t fifo_send_n(fifo_t *fifo, void const *data, size_t max_send);
// Receive an element from a FIFO.
// Returns the actual amount of elements received..
size_t fifo_recv_n(fifo_t *fifo, void *data, size_t max_recv);

// Atomically test how much data can be sent.
size_t fifo_max_send(fifo_t *fifo);
// Atomically test how much data can be received.
size_t fifo_max_recv(fifo_t *fifo);

// Try to send a single element to a FIFO.
static inline bool fifo_send_1(fifo_t *fifo, void const *data) {
    return fifo_send_n(fifo, data, 1);
}
// Try to receive a single element from a FIFO.
static inline bool fifo_recv_1(fifo_t *fifo, void *data) {
    return fifo_recv_n(fifo, data, 1);
}
