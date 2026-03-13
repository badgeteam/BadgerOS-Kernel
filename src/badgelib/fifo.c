
// SPDX-License-Identifier: MIT

#include "fifo.h"

#include "badge_strings.h"
#include "malloc.h"
#include "spinlock.h"



// A generic thread-safe nonblocking FIFO safe for multi-read, multi-write use.
// To achieve the multi-read, multi-write safety, the FIFO has a "reservation" and "index" each for read and write.
// The reservation in atomically updated to reserve data in the buffer to read or write.
// When the read or write is finished, the index is updated in an ACAS loop.
struct fifo {
    // Data buffer.
    uint8_t      *buffer;
    // Element size.
    size_t        ent_size;
    // Capacity.
    size_t        cap;
    // Send index.
    atomic_size_t send_idx;
    // Send reservation.
    atomic_size_t send_resv;
    // Receive index.
    atomic_size_t recv_idx;
    // Receive reservation.
    atomic_size_t recv_resv;
};



// Create a new FIFO.
fifo_t *fifo_create(size_t ent_size, size_t cap) {
    fifo_t *fifo = calloc(1, sizeof(fifo_t));
    if (!fifo) {
        return NULL;
    }
    fifo->buffer   = calloc(ent_size, cap);
    fifo->ent_size = ent_size;
    fifo->cap      = cap;
    if (!fifo->buffer) {
        free(fifo);
        return NULL;
    }
    return fifo;
}

// Destroy a FIFO.
void fifo_destroy(fifo_t *fifo) {
    free(fifo->buffer);
    free(fifo);
}


// Send multiple elements to a FIFO.
// Returns the actual amount of elements sent.
size_t fifo_send_n(fifo_t *fifo, void const *_data, size_t max_send) {
    uint8_t const *data = _data;

    // Try to reserve space in the FIFO.
    size_t rx = fifo->recv_idx;
    size_t tx = fifo->send_resv;
    size_t send_cap;
    do {
        send_cap = (rx - tx + fifo->cap - 1) % fifo->cap;
        if (send_cap > max_send) {
            send_cap = max_send;
        }
    } while (!atomic_compare_exchange_weak_explicit(
        &fifo->send_resv,
        &tx,
        (tx + send_cap) % fifo->cap,
        memory_order_relaxed,
        memory_order_relaxed
    ));
    if (!send_cap) {
        return 0;
    }

    // Copy the data into the FIFO's buffer.
    size_t start_off = tx * fifo->ent_size;
    size_t end_off   = (tx + send_cap) % fifo->cap * fifo->ent_size;
    if (end_off > start_off) {
        mem_copy(fifo->buffer + start_off, data, send_cap * fifo->ent_size);
    } else {
        size_t first_half = fifo->cap * fifo->ent_size - start_off;
        mem_copy(fifo->buffer + start_off, data, first_half);
        mem_copy(fifo->buffer, data + first_half, end_off);
    }

    // Mark the send as completed.
    size_t fin;
    do {
        fin = tx;
    } while (!atomic_compare_exchange_weak_explicit(
        &fifo->send_idx,
        &fin,
        (tx + send_cap) % fifo->cap,
        memory_order_release,
        memory_order_relaxed
    ));

    return send_cap;
}

// Receive an element from a FIFO.
// Returns the actual amount of elements received..
size_t fifo_recv_n(fifo_t *fifo, void *_data, size_t max_recv) {
    uint8_t *data = _data;

    // Try to reserve data from the FIFO.
    size_t rx = fifo->recv_resv;
    size_t tx = fifo->send_idx;
    size_t recv_cap;
    do {
        recv_cap = (tx - rx + fifo->cap) % fifo->cap;
        if (recv_cap > max_recv) {
            recv_cap = max_recv;
        }
    } while (!atomic_compare_exchange_weak_explicit(
        &fifo->recv_resv,
        &rx,
        (rx + recv_cap) % fifo->cap,
        memory_order_acquire,
        memory_order_relaxed
    ));
    if (!recv_cap) {
        return 0;
    }

    // Copy the data out of the FIFO's buffer.
    size_t start_off = rx * fifo->ent_size;
    size_t end_off   = (rx + recv_cap) % fifo->cap * fifo->ent_size;
    if (end_off > start_off) {
        mem_copy(data, fifo->buffer + start_off, recv_cap * fifo->ent_size);
    } else {
        size_t first_half = fifo->cap * fifo->ent_size - start_off;
        mem_copy(data, fifo->buffer + start_off, first_half);
        mem_copy(data + first_half, fifo->buffer, end_off);
    }

    // Mark the receive as completed.
    size_t fin;
    do {
        fin = rx;
    } while (!atomic_compare_exchange_weak_explicit(
        &fifo->recv_idx,
        &fin,
        (rx + recv_cap) % fifo->cap,
        memory_order_relaxed,
        memory_order_relaxed
    ));

    return recv_cap;
}


// Atomically test how much data can be sent.
size_t fifo_max_send(fifo_t *fifo) {
    size_t rx = fifo->recv_idx;
    size_t tx = fifo->send_resv;
    return (rx - tx + fifo->cap - 1) % fifo->cap;
}

// Atomically test how much data can be received.
size_t fifo_max_recv(fifo_t *fifo) {
    size_t rx = fifo->recv_resv;
    size_t tx = fifo->send_idx;
    return (tx - rx + fifo->cap) % fifo->cap;
}
