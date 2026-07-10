
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.10.0)

set(cpu_src
    ${CMAKE_CURRENT_LIST_DIR}/src/entrypoint.S
    ${CMAKE_CURRENT_LIST_DIR}/src/exception.S
    ${CMAKE_CURRENT_LIST_DIR}/src/usermode.S
)
set(cpu_include ${CMAKE_CURRENT_LIST_DIR}/include)
set(cpu_link)
set(cpu_flags)
