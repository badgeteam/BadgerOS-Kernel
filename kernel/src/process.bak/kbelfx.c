
// SPDX-License-Identifier: MIT

#include "assertions.h"
#include "badge_strings.h"
#include "filesystem.h"
#include "malloc.h"
#include "mem/vmm.h"
#include "process/internal.h"
#include "process/process.h"
#include "process/types.h"
#include "usercopy.h"

#include <kbelf.h>



// Measure the length of `str`.
size_t kbelfq_strlen(char const *str) {
    return cstr_length(str);
}

// Copy string from `src` to `dst`.
void kbelfq_strcpy(char *dst, char const *src) {
    cstr_copy(dst, SIZE_MAX, src);
}

// Find last occurrance of `c` in `str`.
char const *kbelfq_strrchr(char const *str, char c) {
    ptrdiff_t off = cstr_last_index(str, c);
    return off == -1 ? NULL : off + str;
}

// Compare string `a` to `b`.
bool kbelfq_streq(char const *a, char const *b) {
    return cstr_equals(a, b);
}

// Copy memory from `src` to `dst`.
void kbelfq_memcpy(void *dst, void const *src, size_t nmemb) {
    mem_copy(dst, src, nmemb);
}

// Fill memory `dst` with `c`.
void kbelfq_memset(void *dst, uint8_t c, size_t nmemb) {
    mem_set(dst, c, nmemb);
}

// Compare memory `a` to `b`.
bool kbelfq_memeq(void const *a, void const *b, size_t nmemb) {
    return mem_equals(a, b, nmemb);
}



// Memory allocator function to use for allocating metadata.
// User-defined.
void *kbelfx_malloc(size_t len) {
    return malloc(len);
}

// Memory allocator function to use for allocating metadata.
// User-defined.
void *kbelfx_realloc(void *mem, size_t len) {
    return realloc(mem, len);
}

// Memory allocator function to use for allocating metadata.
// User-defined.
void kbelfx_free(void *mem) {
    free(mem);
}


// Memory allocator function to use for loading program segments.
// Takes a segment with requested address and permissions and returns a segment with physical and virtual address
// information. Returns success status. User-defined.
bool kbelfx_seg_alloc(kbelf_inst inst, size_t segs_len, kbelf_segment *segs) {
    process_t *proc = proc_get(kbelf_inst_getpid(inst));
    assert_dev_keep(proc != NULL);

    size_t min_addr  = SIZE_MAX;
    size_t max_addr  = 0;
    size_t min_align = 16;

    for (size_t i = 0; i < segs_len; i++) {
        size_t start = segs[i].vaddr_req;
        if (start < min_addr)
            min_addr = start;
        size_t end = segs[i].vaddr_req + segs[i].size;
        if (end > max_addr)
            max_addr = end;
        // logkf(LOG_DEBUG, "Segment %{size;d}: %{size;x} - %{size;x}", i, start, end);
    }
    // logkf(LOG_DEBUG, "Require %{size;d} bytes", max_addr - min_addr);

    errno_size_t vaddr_real = proc_map_raw(proc, min_addr, max_addr - min_addr, min_align, VMM_FLAG_RWX);
    if (vaddr_real < 0) {
        return false;
    }

    if (!kbelf_inst_is_pie(inst) && (size_t)vaddr_real != min_addr) {
        logkf(LOG_ERROR, "Unable to satify virtual address request for non-PIE executable");
        proc_unmap_raw(proc, vaddr_real, max_addr - min_addr);
        return false;
    }

    for (size_t i = 0; i < segs_len; i++) {
        segs[i].vaddr_real   = segs[i].vaddr_req - min_addr + vaddr_real;
        segs[i].paddr        = segs[i].vaddr_real;
        segs[i].laddr        = segs[i].vaddr_real;
        segs[i].alloc_cookie = NULL;
        // logkf(LOG_DEBUG, "Segment %{size;x} mapped to %{size;x}", i, segs[i].vaddr_real);
    }
    segs[0].alloc_cookie = (void *)vaddr_real;

    return true;
}

// Memory allocator function to use for loading program segments.
// Takes a previously allocated segment and unloads it.
// User-defined.
void kbelfx_seg_free(kbelf_inst inst, size_t segs_len, kbelf_segment *segs) {
    (void)segs_len;
    (void)segs;
    process_t *proc = proc_get(kbelf_inst_getpid(inst));
    assert_dev_keep(proc != NULL);
    proc_unmap_raw(proc, segs[0].vaddr_real, (size_t)segs[0].alloc_cookie);
}


// Open a binary file for reading.
// User-defined.
void *kbelfx_open(char const *path) {
    errno_file_t fd = fs_open(FILE_NONE, path, cstr_length(path), FS_O_READ_ONLY);
    if (fd.errno < 0) {
        return NULL;
    }
    file_t *box = malloc(sizeof(file_t));
    if (!box) {
        fs_file_drop(fd.file);
        return NULL;
    }
    *box = fd.file;
    return box;
}

// Close a file.
// User-defined.
void kbelfx_close(void *fd) {
    fs_file_drop(*(file_t *)fd);
}

// Reads a single byte from a file.
// Returns byte on success, -1 on error.
// User-defined.
int kbelfx_getc(void *fd) {
    char      buf;
    errno64_t len = fs_read(*(file_t *)fd, &buf, 1);
    return len > 0 ? buf : -1;
}

// Reads a number of bytes from a file.
// Returns the number of bytes read, or less than that on error.
// User-defined.
long kbelfx_read(void *fd, void *buf, long buf_len) {
    return fs_read(*(file_t *)fd, buf, buf_len);
}

// Reads a number of bytes from a file to a virtual address in the program.
// Returns the number of bytes read, or less than that on error.
// User-defined.
long kbelfx_load(kbelf_inst inst, void *fd, kbelf_laddr laddr, long len) {
    if (len < 0)
        return -1;
    process_t *proc    = proc_get_unsafe(kbelf_inst_getpid(inst));
    long       tmp_cap = !CONFIG_NOMMU ? 2097152 : 16384;
    tmp_cap            = tmp_cap < len ? tmp_cap : len;
    void *tmp          = malloc(tmp_cap);
    long  total        = 0;
    while (len > tmp_cap) {
        total += fs_read(*(file_t *)fd, tmp, tmp_cap);
        copy_to_user_raw(proc, laddr, tmp, tmp_cap);
        laddr += tmp_cap;
        len   -= tmp_cap;
    }
    total += fs_read(*(file_t *)fd, tmp, len);
    copy_to_user_raw(proc, laddr, tmp, len);
    free(tmp);
    return total;
}

// Sets the absolute offset in the file.
// Returns 0 on success, -1 on error.
// User-defined.
int kbelfx_seek(void *fd, long pos) {
    errno64_t q = fs_seek(*(file_t *)fd, SEEK_SET, pos);
    return pos == q ? 0 : -1;
}



// Read bytes from a load address in the program.
bool kbelfx_copy_from_user(kbelf_inst inst, void *buf, kbelf_laddr laddr, size_t len) {
    process_t *proc = proc_get_unsafe(kbelf_inst_getpid(inst));
    return copy_from_user_raw(proc, buf, laddr, len);
}

// Write bytes to a load address in the program.
bool kbelfx_copy_to_user(kbelf_inst inst, kbelf_laddr laddr, void *buf, size_t len) {
    process_t *proc = proc_get_unsafe(kbelf_inst_getpid(inst));
    return copy_to_user_raw(proc, laddr, buf, len);
}

// Get string length from a load address in the program.
ptrdiff_t kbelfx_strlen_from_user(kbelf_inst inst, kbelf_laddr laddr) {
    process_t *proc = proc_get_unsafe(kbelf_inst_getpid(inst));
    return strlen_from_user_raw(proc, laddr, SIZE_MAX);
}



// Find and open a dynamic library file.
// Returns non-null on success, NULL on error.
// User-defined.
kbelf_file kbelfx_find_lib(char const *needed) {
    (void)needed;
    return NULL;
}



// Number of built-in libraries.
// Optional user-defined.
size_t                   kbelfx_builtin_libs_len = 0;
// Array of built-in libraries.
// Optional user-defined.
kbelf_builtin_lib const *kbelfx_builtin_libs     = NULL;
