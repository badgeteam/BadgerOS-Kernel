
# SPDX-License-Identifier: MIT

cmake_minimum_required(VERSION 3.10.0)

macro(config_enum VAR DEFAULT DESCRIPTION)
    list(APPEND ALL_CONFIGS ${VAR})
    set(CONFIG_${VAR} ${DEFAULT} CACHE STRING ${DESCRIPTION})
    set_property(CACHE CONFIG_${VAR} PROPERTY STRINGS ${ARGN})
    set(_tmp_valid_values ${ARGN})
    list(FIND _tmp_valid_values "${CONFIG_${VAR}}" _tmp_index)
    if(_tmp_index EQUAL -1)
        message(WARNING
            "Invalid CONFIG_${VAR}; valid options are: ${ARGN}; setting to ${DEFAULT}"
        )
        set(CONFIG_${VAR} ${DEFAULT} CACHE STRING ${DESCRIPTION} FORCE)
    endif()
    add_definitions(
        -DCONFIG_${VAR}=${CONFIG_${VAR}}
        -DCONFIGSTR_${VAR}="${CONFIG_${VAR}}"
        -DCONFIGENUM_${VAR}_${CONFIG_${VAR}}
    )
endmacro()

macro(config_bool VAR DEFAULT DESCRIPTION)
    config_enum(${VAR} ${DEFAULT} ${DESCRIPTION} true false)
endmacro()

macro(config_const VAR VALUE DESCRIPTION)
    config_enum(${VAR} ${VALUE} ${DESCRIPTION} ${VALUE})
endmacro()
