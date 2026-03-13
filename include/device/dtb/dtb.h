
// SPDX-License-Identifier: MIT

#pragma once

#include "badge_strings.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>



// Minimum supported FDT version.
#define FDT_VERSION_MIN  16
// Maximum supported FDT version.
#define FDT_VERSION_MAX  16
// Magic value for FDT headers.
#define FDT_HEADER_MAGIC 0xd00dfeed

// FDT node types.
typedef enum {
    FDT_BEGIN_NODE = 1,
    FDT_END_NODE,
    FDT_PROP,
    FDT_NOP,
    FDT_END = 9,
} fdt_node_t;

// Struct attributes for FDT.
#define FDT_ATTR __attribute__((scalar_storage_order("big-endian")))

// FDT header struct.
typedef struct FDT_ATTR {
    // This field shall contain 0xd00dfeed.
    uint32_t magic;
    // The size of the entire FDT including this header.
    uint32_t totalsize;
    // Offset in bytes of the structure block.
    uint32_t off_dt_struct;
    // Offset in bytes of the string block.
    uint32_t off_dt_strings;
    // Offset in bytes of the memory reservation block.
    uint32_t off_mem_rsvmap;
    // FDT version.
    uint32_t version;
    // The oldest version with which this FDT is backwards-compatible.
    uint32_t last_comp_version;
    // Booting CPU ID.
    uint32_t boot_cpuid_phys;
    // Size of the string block.
    uint32_t size_dt_strings;
    // Size of the structure block.
    uint32_t size_dt_struct;
} dtb_header_t;

// FDT reserved memory.
typedef struct FDT_ATTR {
    // Base physical address.
    uint64_t paddr;
    // Size in bytes.
    uint64_t size;
} dtb_rsvmem_t;



// DTB prop.
typedef struct dtb_prop_t dtb_prop_t;
// DTB node.
typedef struct dtb_node_t dtb_node_t;

// DTB prop.
typedef struct dtb_prop_t {
    // Parent node.
    dtb_node_t *parent;
    // Prop name.
    char const *name;
    // How deep in the tree this prop is.
    uint8_t     depth;
    // Content length.
    uint32_t    content_len;
    // Content pointer.
    void const *content;
} dtb_prop_t;

// DTB node.
typedef struct dtb_node_t {
    // Parent node.
    dtb_node_t *parent;
    // Node name.
    char const *name;
    // How deep in the tree this node is.
    uint8_t     depth;
    // Number of child nodes.
    size_t      nodes_len;
    // Child nodes.
    dtb_node_t *nodes;
    // Number of child props.
    size_t      props_len;
    // Child props.
    dtb_prop_t *props;
    // Phandle value, if any.
    uint32_t    phandle;
    // Whether a phandle is present.
    bool        has_phandle;
} dtb_node_t;

// DTB reading handle.
typedef struct {
    // DTB pointer.
    dtb_header_t *dtb_hdr;
    // Resolved structure block address.
    uint32_t     *struct_blk;
    // Resolved strings block address.
    char         *string_blk;
    // DTB root node.
    dtb_node_t    root;
    // Number of phandles found.
    size_t        phandles_len;
    // Nodes sorted by phandle.
    dtb_node_t  **phandles;
} dtb_handle_t;



// Interpret the DTB and prepare for reading.
dtb_handle_t *dtb_open(void *dtb_ptr);
// Clean up memory allocated by DTB operations.
void          dtb_close(dtb_handle_t *handle);

// Go to the first node or prop in the DTB.
dtb_node_t *dtb_root_node(dtb_handle_t *handle);

// Get a node with a specific name.
dtb_node_t *dtb_get_node_l(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len);
// Get a prop with a specific name.
dtb_prop_t *dtb_get_prop_l(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len);

// Find a node in the DTB.
dtb_node_t *dtb_find_node(dtb_handle_t *handle, char const *path);
// Get a DTB node by phandle.
dtb_node_t *dtb_phandle_node(dtb_handle_t *handle, uint32_t phandle);


// Read a prop as a single unsigned number.
uintmax_t   dtb_prop_read_uint(dtb_handle_t *handle, dtb_prop_t *prop);
// Read a prop as an array of cells.
uint32_t    dtb_prop_read_cell(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t cell_idx);
// Read an unsigned number from a prop formatted as cells.
uintmax_t   dtb_prop_read_cells(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t cell_idx, uint32_t cell_count);
// Get raw prop contents.
void const *dtb_prop_content(dtb_handle_t *handle, dtb_prop_t *prop, uint32_t *len_out);

// Read a prop as a single unsigned number.
uintmax_t dtb_read_uint_l(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len);
// Read a prop as an array of cells.
uint32_t  dtb_read_cell_l(
     dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, size_t name_len, uint32_t cell_idx
 );
// Read an unsigned number from a prop formatted as cells.
uintmax_t dtb_read_cells_l(
    dtb_handle_t *handle,
    dtb_node_t   *parent_node,
    char const   *name,
    size_t        name_len,
    uint32_t      cell_idx,
    uint32_t      cell_count
);



// Read a prop as a single unsigned number.
static inline uintmax_t dtb_read_uint(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name) {
    return dtb_read_uint_l(handle, parent_node, name, cstr_length(name));
}

// Read a prop as an array of cells.
static inline uint32_t
    dtb_read_cell(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, uint32_t cell_idx) {
    return dtb_read_cell_l(handle, parent_node, name, cstr_length(name), cell_idx);
}

// Read an unsigned number from a prop formatted as cells.
static inline uintmax_t dtb_read_cells(
    dtb_handle_t *handle, dtb_node_t *parent_node, char const *name, uint32_t cell_idx, uint32_t cell_count
) {
    return dtb_read_cells_l(handle, parent_node, name, cstr_length(name), cell_idx, cell_count);
}


// Get a node with a specific name.
static inline dtb_node_t *dtb_get_node(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name) {
    return dtb_get_node_l(handle, parent_node, name, __builtin_strlen(name));
}

// Get a prop with a specific name.
static inline dtb_prop_t *dtb_get_prop(dtb_handle_t *handle, dtb_node_t *parent_node, char const *name) {
    return dtb_get_prop_l(handle, parent_node, name, __builtin_strlen(name));
}
