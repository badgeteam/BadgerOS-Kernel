
// SPDX-License-Identifier: MIT

#include "device/dtb/dtb.h"

#include "arrays.h"
#include "assertions.h"
#include "badge_strings.h"
#include "log.h"
#include "panic.h"

#if __BYTE_ORDER == __LITTLE_ENDIAN
#define htobe32(x) __builtin_bswap32(x)
#define be32toh(x) __builtin_bswap32(x)
#else
#define htobe32(x) (x)
#define be32toh(x) (x)
#endif



// Sort phandles by index.
static int phandle_cmp(void const *a, void const *b) {
    dtb_node_t const *const *phandle_a = a;
    dtb_node_t const *const *phandle_b = b;
    return (**phandle_a).phandle - (**phandle_b).phandle;
}

// Clean up memory allocated by a DTB node.
static void dtb_free_node(dtb_node_t *node) {
    free(node->props);
    for (size_t i = 0; i < node->nodes_len; i++) {
        dtb_free_node(node->nodes + i);
    }
    free(node->nodes);
}

// Parse a DTB prop.
static bool dtb_parse_prop(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t *offset_ptr) {
    mem_set(prop, 0, sizeof(dtb_prop_t));
    uint32_t offset = *offset_ptr;

    // Skip NOPs.
    while (handle->struct_blk[offset] == htobe32(FDT_NOP)) {
        offset++;
    }
    // Match prop token.
    if (handle->struct_blk[offset++] != htobe32(FDT_PROP)) {
        return false;
    }

    prop->content_len  = be32toh(handle->struct_blk[offset++]);
    prop->name         = handle->string_blk + be32toh(handle->struct_blk[offset++]);
    prop->content      = (void const *)(handle->struct_blk + offset);
    offset            += (prop->content_len + 3) / 4;

    *offset_ptr = offset;
    return true;
}

// Parse a DTB node.
static bool dtb_parse_node(dtb_handle_t *handle, dtb_node_t *node, uint32_t *offset_ptr, uint8_t depth) {
    mem_set(node, 0, sizeof(dtb_node_t));
    uint32_t offset = *offset_ptr;
    node->depth     = depth;

    // Skip NOPs.
    while (handle->struct_blk[offset] == htobe32(FDT_NOP)) {
        offset++;
    }
    // Match begin node token.
    if (handle->struct_blk[offset++] != htobe32(FDT_BEGIN_NODE)) {
        return false;
    }
    // Read node name.
    node->name  = (void *)(handle->struct_blk + offset);
    offset     += cstr_length(node->name) / 4 + 1;

    // Parse child props.
    dtb_prop_t prop;
    while (dtb_parse_prop(handle, &prop, &offset)) {
        prop.depth = depth + 1;
        if (!array_len_insert(&node->props, sizeof(dtb_prop_t), &node->props_len, &prop, node->props_len)) {
            logk(LOG_FATAL, "Out of memory");
            panic_abort();
        }
    }

    // Parse child nodes.
    dtb_node_t child;
    while (dtb_parse_node(handle, &child, &offset, depth + 1)) {
        if (!array_len_insert(&node->nodes, sizeof(dtb_node_t), &node->nodes_len, &child, node->nodes_len)) {
            logk(LOG_FATAL, "Out of memory");
            panic_abort();
        }
    }

    // Skip NOPs.
    while (handle->struct_blk[offset] == htobe32(FDT_NOP)) {
        offset++;
    }
    // Match end node token.
    if (handle->struct_blk[offset++] != htobe32(FDT_END_NODE)) {
        return false;
    }

    *offset_ptr = offset;
    return true;
}

// Set relative pointers.
static void dtb_build_refs(dtb_handle_t *handle, dtb_node_t *node) {
    // Update child prop relative pointers.
    for (size_t i = 0; i < node->props_len; i++) {
        node->props[i].parent = node;
    }

    // Update child node relative pointers.
    for (size_t i = 0; i < node->nodes_len; i++) {
        node->nodes[i].parent = node;
    }

    // Check for phandles.
    dtb_prop_t *phandle = dtb_get_prop(handle, node, "phandle");
    if (phandle) {
        node->phandle     = dtb_prop_read_uint(handle, phandle);
        node->has_phandle = true;
        if (!array_len_sorted_insert(&handle->phandles, sizeof(void *), &handle->phandles_len, &node, phandle_cmp)) {
            logk(LOG_FATAL, "Out of memory");
            panic_abort();
        }
    }

    // Build reference pointers and tables.
    for (size_t i = 0; i < node->nodes_len; i++) {
        dtb_build_refs(handle, node->nodes + i);
    }
}

// Interpret the DTB and prepare for reading.
dtb_handle_t *dtb_open(void *dtb_ptr) {
    dtb_handle_t *handle = malloc(sizeof(dtb_handle_t));
    mem_set(handle, 0, sizeof(dtb_handle_t));
    dtb_header_t *hdr = (dtb_header_t *)dtb_ptr;
    handle->dtb_hdr   = hdr;

    // Magic check.
    if (hdr->magic != FDT_HEADER_MAGIC) {
        logk_from_isr(LOG_ERROR, "Invalid magic");
        free(handle);
        return NULL;
    }
    handle->string_blk = (char *)dtb_ptr + hdr->off_dt_strings;
    handle->struct_blk = (uint32_t *)((char *)dtb_ptr + hdr->off_dt_struct);

    // Parse the root node.
    uint32_t offset = 0;
    if (!dtb_parse_node(handle, &handle->root, &offset, 0)) {
        free(handle->phandles);
        free(handle);
        return NULL;
    }
    handle->root.name = "/";
    // Skip NOPs.
    while (handle->struct_blk[offset] == htobe32(FDT_NOP)) {
        offset++;
    }
    // Match FDT END token.
    if (handle->struct_blk[offset] != htobe32(FDT_END)) {
        dtb_free_node(&handle->root);
        free(handle->phandles);
        free(handle);
        return NULL;
    }

    // Set relative pointers.
    dtb_build_refs(handle, &handle->root);

    return handle;
}

// Clean up memory allocated by DTB operations.
void dtb_close(dtb_handle_t *handle) {
    dtb_free_node(&handle->root);
    free(handle->phandles);
    free(handle);
}


// Go to the first node or prop in the DTB.
dtb_node_t *dtb_root_node(dtb_handle_t *handle) {
    return handle ? &handle->root : NULL;
}


// Get a node with a specific name.
dtb_node_t *dtb_get_node_l(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len) {
    (void)handle;
    for (size_t i = 0; i < parent_node->nodes_len; i++) {
        dtb_node_t *node = &parent_node->nodes[i];
        if (cstr_prefix_equals(node->name, name, name_len) && node->name[name_len] == 0) {
            return node;
        }
    }
    return NULL;
}

// Get a prop with a specific name.
dtb_prop_t *dtb_get_prop_l(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len) {
    (void)handle;
    for (size_t i = 0; i < parent_node->props_len; i++) {
        dtb_prop_t *prop = &parent_node->props[i];
        if (cstr_prefix_equals(prop->name, name, name_len) && prop->name[name_len] == 0) {
            return prop;
        }
    }
    return NULL;
}


// Find a node in the DTB.
dtb_node_t *dtb_find_node(dtb_handle_t *handle, char const *path) {
    (void)handle;
    (void)path;
    logk(LOG_WARN, "TODO: dtb_find_node");
    return NULL;
}

// Get a DTB node by phandle.
dtb_node_t *dtb_phandle_node(dtb_handle_t *handle, uint32_t phandle) {
    dtb_node_t const        dummy     = {.phandle = phandle};
    dtb_node_t const *const dummy_ptr = &dummy;
    array_binsearch_t       res =
        array_binsearch(handle->phandles, sizeof(void *), handle->phandles_len, &dummy_ptr, phandle_cmp);
    return res.found ? handle->phandles[res.index] : NULL;
}



// Read a prop as a single unsigned number.
uintmax_t dtb_prop_read_uint(dtb_handle_t *handle, dtb_prop_t *prop) {
    (void)handle;
    uintmax_t      val = 0;
    uint8_t const *ptr = prop->content;
    for (size_t i = 0; i < prop->content_len; i++) {
        val <<= 8;
        val  |= ptr[i];
    }
    return val;
}

// Read a prop as an array of cells.
uint32_t dtb_prop_read_cell(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t cell_idx) {
    return dtb_prop_read_cells(handle, prop, cell_idx, 1);
}

// Read an unsigned number from a prop formatted as cells.
uintmax_t dtb_prop_read_cells(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t cell_idx, uint32_t cell_count) {
    (void)handle;
    uintmax_t       val = 0;
    uint32_t const *ptr = prop->content;
    for (size_t i = 0; i < cell_count; i++) {
        val <<= 32;
        val  |= be32toh(ptr[i + cell_idx]);
    }
    return val;
}

// Get raw prop contents.
void const *dtb_prop_content(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t *len_out) {
    (void)handle;
    if (!prop) {
        return NULL;
    }
    if (len_out) {
        *len_out = prop->content_len;
    }
    return prop->content;
}


// Read a prop as a single unsigned number.
uintmax_t dtb_read_uint_l(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len) {
    dtb_prop_t *prop = dtb_get_prop_l(handle, parent_node, name, name_len);
    return prop ? dtb_prop_read_uint(handle, prop) : 0;
}

// Read a prop as an array of cells.
uint32_t dtb_read_cell_l(
    dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len, uint32_t cell_idx
) {
    dtb_prop_t *prop = dtb_get_prop_l(handle, parent_node, name, name_len);
    return prop ? dtb_prop_read_cell(handle, prop, cell_idx) : 0;
}

// Read an unsigned number from a prop formatted as cells.
uintmax_t dtb_read_cells_l(
    dtb_handle_t *handle,
    dtb_node_t   *parent_node,
    char const   *name,
    size_t        name_len,
    uint32_t      cell_idx,
    uint32_t      cell_count
) {
    dtb_prop_t *prop = dtb_get_prop_l(handle, parent_node, name, name_len);
    return prop ? dtb_prop_read_cells(handle, prop, cell_idx, cell_count) : 0;
}
