#!/usr/bin/python3

import os

script_dir = os.path.dirname(os.path.abspath(__file__))
gen_warning = """
// WARNING: This is a generated file, do not edit it!
// SPDX-License-Identifier: CC0
"""
rust_imports = """
use crate::{
    bindings::error::Errno,
    cpu::thread::{GpRegfile, SpRegfile},
};
use core::ffi::*;

use super::{
    PID, TID,
    uapi::{
        signal::sigaction, sigset::sigset_t, stat::stat, termios::termios, time::timespec,
        uname::utsname,
    },
    usercopy::{UserPtr, UserPtrMut, UserSlice, UserSliceMut},
};
"""
c_includes = """
#include <abi-bits/pid_t.h>
#include <abi-bits/signal.h>
#include <abi-bits/sigset_t.h>
#include <abi-bits/stat.h>
#include <abi-bits/tid_t.h>
#include <bits/types.h>
#include <stdbool.h>
"""
c_impl_includes = """
#include <sys/syscall.h>
#include <sys/do_syscall_asm.h>

#pragma GCC diagnostic ignored "-Wunused-parameter"
"""
externc_start = """
#ifdef __cplusplus
extern "C" {
#endif
"""
externc_end = """
#ifdef __cplusplus
} // extern "C"
#endif
"""
abi_dir = script_dir + "/../misc"
mashall_dir = script_dir + "/../src/process/syscall"
template_dir = mashall_dir + "/templates"
base_types: dict[str,BaseType] = dict()
result_map: dict[str,str] = dict()
syscalls: dict[int,Syscall] = dict()
namespaces: set[str] = set()

class Type:
    def c_name(self, ident: str|None) -> str:
        raise NotImplementedError()
    
    def rs_name(self, as_param: bool) -> str:
        raise NotImplementedError()
    
    def rs_marshalled(self) -> str:
        raise NotImplementedError()
    
    def marshal(self, name: str) -> tuple[str, str]:
        """
        Returns parameter list followed by marshalling code that produces a local variable with the name `name`.
        The marshalling code should directory return an errno cast to _ (e.g. `return -(Errno::EFAULT as i32) as _`) on failure.
        """
        raise NotImplementedError()

class BaseType(Type):
    def __init__(self, row: dict[str,str]):
        self.name = row["Table"]
        self.result = row.get("Result")
        self.ctype = row["C"]
        self.rstype = row["Rust"]
    
    def c_name(self, ident: str | None) -> str:
        if ident != None:
            return f"{self.ctype} {ident}"
        else:
            return self.ctype
    
    def rs_name(self, as_param: bool) -> str:
        return self.rstype
    
    def marshal(self, name: str) -> tuple[str, str]:
        return (f"    {name}: {self.rstype},\n", "")

class PtrType(Type):
    def __init__(self, mut: bool, opt: bool, inner: BaseType|PtrType):
        self.mut = mut
        self.opt = opt
        self.inner = inner
    
    def c_name(self, ident: str | None) -> str:
        if self.mut:
            return f"{self.inner.c_name(None)} *{ident or ""}"
        else:
            return f"{self.inner.c_name(None)} const *{ident or ""}"
    
    def rs_name(self, as_param: bool) -> str:
        if as_param:
            mut = "Mut" if self.mut else ""
            if self.opt:
                return f"Option<UserPtr{mut}<{self.inner.rs_name(False)}>>"
            else:
                return f"UserPtr{mut}<{self.inner.rs_name(False)}>"
        else:
            mut = "mut" if self.mut else "const"
            return f"*{mut} {self.inner.rs_name(False)}"
    
    def marshal(self, name: str) -> tuple[str, str]:
        ptr = "mut" if self.mut else "const"
        _mut = "_mut" if self.mut else ""
        Mut = "Mut" if self.mut else ""
        _opt = "_nullable" if self.opt else ""
        return (
            f"    {name}: *{ptr} {self.inner.rs_name(False)},\n",
            f"    let {name} = match UserPtr{Mut}::new{_opt}{_mut}({name}) {{\n"+
            f"        Ok({name}) => {name},\n"+
            f"        Err(_) => return -(Errno::EFAULT as i32) as _,\n"+
            f"    }};\n"
        )

class ArrType(Type):
    def __init__(self, mut: bool, opt: bool, inner: BaseType|PtrType, len: int|None):
        self.mut = mut
        self.opt = opt
        self.inner = inner
        self.len = len
    
    def c_name(self, ident: str | None) -> str:
        if self.len == None:
            const = "" if self.mut else " const"
            return f"{self.inner.c_name(None)}{const} *{ident}, __mlibc_size {ident}_len"
        else:
            return f"{self.inner.c_name(None)} {ident or ""}[{self.len}]"
    
    def rs_name(self, as_param: bool) -> str:
        if self.len == None:
            if as_param:
                mut = "Mut" if self.mut else ""
                if self.opt:
                    return f"Option<UserSlice{mut}<{self.inner.rs_name(False)}>>"
                else:
                    return f"UserSlice{mut}<{self.inner.rs_name(False)}>"
            else:
                return f"[{self.inner.rs_name(False)}]"
        else:
            if as_param:
                mut = "Mut" if self.mut else ""
                if self.opt:
                    return f"Option<UserPtr{mut}<[{self.inner.rs_name(False)}; {self.len}]>>"
                else:
                    return f"UserPtr{mut}<[{self.inner.rs_name(False)}; {self.len}]>"
            else:
                return f"[{self.inner.rs_name(False)}; {self.len}]"
    
    def marshal(self, name: str) -> tuple[str, str]:
        ptr = "mut" if self.mut else "const"
        _mut = "_mut" if self.mut else ""
        Mut = "Mut" if self.mut else ""
        _opt = "_nullable" if self.opt else ""
        if self.len == None:
            return (
                f"    {name}: *{ptr} {self.inner.rs_name(False)},\n"+
                f"    {name}_len: usize\n",
                f"    let {name} = match UserSlice{Mut}::new{_opt}{_mut}({name}, {name}_len) {{\n"+
                f"        Ok({name}) => {name},\n"+
                f"        Err(_) => return -(Errno::EFAULT as i32) as _,\n"+
                f"    }};\n"
            )
        else:
            return (
                f"    {name}: *{ptr} [{self.inner.rs_name(False)}; {self.len}],\n",
                f"    let {name} = match UserPtr{Mut}::new{_opt}{_mut}({name}) {{\n"+
                f"        Ok({name}) => {name},\n"+
                f"        Err(_) => return -(Errno::EFAULT as i32) as _,\n"+
                f"    }};\n"
            )

def parse_base_type(raw: str) -> BaseType:
    raw = raw.strip()
    if raw == "":
        print("Empty type name")
        exit(1)
    try:
        return base_types[raw]
    except KeyError:
        print(f"Invalid type `{raw}`")
        exit(1)

def parse_inner_type(raw: str) -> BaseType|PtrType:
    type_ = parse_type(raw)
    if type(type_) == BaseType or type(type_) == PtrType:
        return type_
    else:
        print("Cannot create nested array or pointer-to-array type\n")
        exit(1)

def parse_type(raw: str) -> Type:
    raw = raw.strip()
    if raw[0] == '[':
        [len, inner] = raw[1:].split("]", 1)
        inner = inner.strip()
        opt = False
        if inner.startswith("?"):
            opt = True
            inner = inner[1:]
        mut = False
        if inner.startswith("mut "):
            mut = True
            inner = inner[4:]
        if len.strip() == "":
            return ArrType(mut, opt, parse_inner_type(inner), None)
        else:
            try:
                len = int(len.strip())
            except ValueError:
                print(f"Invalid length `{len.strip()}`")
                exit(1)
            return ArrType(mut, opt, parse_inner_type(inner), len)
    
    elif raw[0] == '*':
        inner = raw[1:]
        inner = inner.strip()
        opt = False
        if inner.startswith("?"):
            opt = True
            inner = inner[1:]
        mut = False
        if inner.startswith("mut "):
            mut = True
            inner = inner[4:]
        return PtrType(mut, opt, parse_inner_type(inner))
        
    else:
        return parse_base_type(raw)

class Parameter:
    def __init__(self, name: str, type: Type):
        self.name = name
        self.type = type

def parse_params(raw: str) -> list[Parameter]:
    out = []
    for param in raw.split(','):
        if ':' not in param:
            print("Malformed parameter `{}`", param)
            exit(1)
        [name, type] = param.split(':', 1)
        name = name.strip()
        if not name.isidentifier():
            print(f"Invalid name `{name}`")
            exit(1)
        out += [Parameter(name, parse_type(type))]
    return out

class Syscall:
    def __init__(self, row: dict[str,str]):
        self.index = int(row["Index"])
        self.namespace = row["Namespace"]
        self.name = row["Name"]
        if row["Regs"] == "Y":
            self.regs = True
        elif row["Regs"] == "N":
            self.regs = False
        else:
            raise ValueError("Set `Regs` to either `Y` or `N`")
        if "Params" in row:
            self.params = parse_params(row["Params"])
        else:
            self.params = []
        if "Returns" in row:
            returns = row["Returns"].strip()
            self.returns = parse_base_type(returns)
            self.true_returns = self.returns
            if returns in result_map:
                self.returns = parse_base_type(result_map[returns])
        else:
            self.returns = base_types["int"]
            self.true_returns = None
    
    def c_name(self):
        return f"__syscall_{self.namespace}_{self.name}"

def strip_or_delete(cell: str) -> str|None:
    cell = cell.strip()
    if cell == "":
        return None
    else:
        return cell

def read_csv_line(line: str, max: int|None) -> list[str|None]:
    return [strip_or_delete(x) for x in line.split(';', max if max != None else -1)]

def read_csv(path: str) -> list[dict[str,str]]:
    out = []
    with open(path, "r") as fd:
        header = ""
        while header == "":
            header = fd.readline()
        header = read_csv_line(header, None)
        for raw in fd.readlines():
            if raw == "": continue
            line = read_csv_line(raw, len(header))
            tmp = {}
            for i in range(min(len(header), len(line))):
                if line[i] != None:
                    tmp[header[i]] = line[i]
            out += [tmp]
    return out

def load_base_types():
    csv = read_csv(abi_dir + "/systype.csv")
    for i in range(len(csv)):
        try:
            type = BaseType(csv[i])
            if type.result != None:
                result_map[type.name] = type.result
            if type.name in base_types:
                print(f"systype.csv: duplicate type with name `{type.name}`")
            base_types[type.name] = type
        except KeyError as k:
            print(f"systype.csv: missing cell `{k.args[0]}` in row {i+1}")
            exit(1)

def load_syscalls():
    csv = read_csv(abi_dir + "/systab.csv")
    for i in range(len(csv)):
        try:
            syscall = Syscall(csv[i])
            if syscall.index in syscalls:
                print(f"systab.csv: duplicate syscall with index {syscall.index}")
            syscalls[syscall.index] = syscall
            namespaces.add(syscall.namespace)
        except KeyError as k:
            print(f"systab.csv: missing cell `{k.args[0]}` in row {i+1}")
            exit()

def gen_c_header():
    with open(abi_dir + "/syscall.h", "w") as fd:
        fd.write(gen_warning)
        fd.write("\n#pragma once\n")
        fd.write(c_includes)
        fd.write(externc_start)
        
        for syscall in syscalls.values():
            fd.write(f"\n{syscall.returns.c_name(None)} __syscall_{syscall.namespace}_{syscall.name}(\n")
            for i in range(len(syscall.params)):
                fd.write("    ")
                fd.write(syscall.params[i].type.c_name(f"__{syscall.params[i].name}"))
                if i < len(syscall.params) - 1:
                    fd.write(',')
                fd.write('\n')
            fd.write(f");\n")
        
        fd.write(externc_end)

def gen_c_wrappers():
    with open(abi_dir + "/syscall.c", "w") as fd:
        fd.write(gen_warning)
        fd.write(c_impl_includes)
        
        for syscall in syscalls.values():
            fd.write(f"\n__attribute__((naked))\n")
            fd.write(f"{syscall.returns.c_name(None)} __syscall_{syscall.namespace}_{syscall.name}(\n")
            for i in range(len(syscall.params)):
                fd.write("    ")
                fd.write(syscall.params[i].type.c_name(f"__{syscall.params[i].name}"))
                if i < len(syscall.params) - 1:
                    fd.write(',')
                fd.write('\n')
            fd.write(f") {{\n")
            fd.write(f"    _DO_SYSCALL_ASM({syscall.index});\n")
            fd.write(f"}}\n")

def gen_rust_marshalling():
    if not os.path.isdir(template_dir):
        os.mkdir(template_dir)
    with open(mashall_dir + "/mod.rs", "w") as fd:
        fd.write(gen_warning)
        fd.write(rust_imports)
        
        fd.write("\n")
        for namespace in namespaces:
            fd.write(f"pub mod {namespace};\n")
            with open(f"{template_dir}/{namespace}.rs", "w") as template:
                template.write(gen_warning)
        
        # Syscall dispatcher.
        # Note: `sigret` is special-cased to bypass `regs.set_retval`. Its handler (`exit_signal`)
        # fully replaces `regs` with the previously-interrupted context, and writing a generic
        # return value into `regs.a0` afterward would clobber that just-restored register.
        fd.write("\npub fn dispatch(regs: &mut GpRegfile, sregs: &mut SpRegfile, args: [usize; 6], sysno: usize) {\n")
        fd.write("    let retval: usize;\n")
        fd.write("    match sysno {\n")
        for syscall in syscalls.values():
            no_retval = syscall.namespace == "proc" and syscall.name == "sigret"
            if no_retval:
                fd.write(f"        {syscall.index} => {{ marshal_{syscall.namespace}_{syscall.name}(")
            else:
                fd.write(f"        {syscall.index} => retval = marshal_{syscall.namespace}_{syscall.name}(")
            if syscall.regs:
                fd.write("regs, sregs")
            i = 0
            for param in syscall.params:
                if i or syscall.regs:
                    fd.write(", ")
                if param.type.rs_name(False) == 'bool':
                    fd.write(f"args[{i}] != 0")
                else:
                    if type(param.type) == ArrType and param.type.len == None:
                        fd.write(f"args[{i}] as _, ")
                        i += 1
                    fd.write(f"args[{i}] as _")
                i += 1
            if no_retval:
                fd.write("); return; },\n")
            else:
                fd.write(") as _,\n")
        fd.write("        _ => retval = -(Errno::ENOSYS as i32) as _,\n")
        fd.write("    }\n")
        fd.write("    regs.set_retval(retval);\n")
        fd.write("}\n")
        
        for syscall in syscalls.values():
            # Syscall templates.
            with open(f"{template_dir}/{syscall.namespace}.rs", "a") as template:
                template.write(f"\npub(super) fn {syscall.name}(\n")
                if syscall.regs:
                    template.write(f"    regs: &mut GpRegfile,\n")
                    template.write(f"    sregs: &mut SpRegfile,\n")
                for param in syscall.params:
                    template.write(f"    {param.name}: {param.type.rs_name(True)},\n")
                if syscall.true_returns == None:
                    returns = "()"
                else:
                    returns = syscall.true_returns.rs_name(False)
                template.write(f") -> EResult<{returns}> {{\n")
                template.write(f"    todo!();\n")
                template.write(f"}}\n")
            
            # Syscall marshalling code.
            paramstr = ""
            marshalstr = ""
            marshalparamstr = ""
            if syscall.regs:
                paramstr += "    regs: &mut GpRegfile,\n"
                paramstr += "    sregs: &mut SpRegfile,\n"
                marshalparamstr += "        regs,\n"
                marshalparamstr += "        sregs,\n"
            for param in syscall.params:
                marshalparamstr += f"        {param.name},\n"
                (param, marshal) = param.type.marshal(param.name)
                paramstr += param
                marshalstr += marshal
            retstr = ""
            if syscall.true_returns == None:
                retstr = "        Ok(()) => 0,\n"
            else:
                retstr = f"        Ok(x) => x as {syscall.returns.rs_name(False)},\n"
            fd.write(f"\nfn marshal_{syscall.namespace}_{syscall.name}(\n")
            fd.write(paramstr)
            fd.write(f") -> {syscall.returns.rs_name(False)} {{\n")
            fd.write(marshalstr)
            fd.write(f"    match {syscall.namespace}::{syscall.name}(\n")
            fd.write(marshalparamstr)
            fd.write(f"    ) {{\n")
            fd.write(retstr)
            fd.write(f"        Err(x) => -(x as u32 as {syscall.returns.rs_name(False)}),\n")
            fd.write(f"    }}\n")
            fd.write(f"}}\n")

load_base_types()
load_syscalls()
gen_c_header()
gen_c_wrappers()
gen_rust_marshalling()
