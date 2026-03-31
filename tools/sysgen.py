#!/usr/bin/python3

import os

script_dir = os.path.dirname(os.path.abspath(__file__))
gen_warning = "// WARNING: This is a generated file, do not edit it!\n// SPDX-License-Identifier: CC0\n\n"
imports = """
"""
abi_dir = script_dir + "/../misc"
mashall_dir = script_dir + "/../src/process/syscall"
template_dir = mashall_dir + "/templates"
need_header = { "bits/types.h" }
base_types: dict[str,BaseType] = dict()
result_map: dict[str,str] = dict()
syscalls: dict[int,Syscall] = dict()
namespaces: set[str] = set()

class Type:
    def c_name(self, ident: str|None) -> str:
        raise NotImplementedError()
    
    def rs_name(self, as_param: bool) -> str:
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
        self.header = row.get("Header")
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
    def __init__(self, mut: bool, inner: BaseType|PtrType):
        self.mut = mut
        self.inner = inner
    
    def c_name(self, ident: str | None) -> str:
        if self.mut:
            return f"{self.inner.c_name(None)} *{ident or ""}"
        else:
            return f"{self.inner.c_name(None)} const *{ident or ""}"
    
    def rs_name(self, as_param: bool) -> str:
        if as_param:
            mut = "Mut" if self.mut else ""
            return f"UserPtr{mut}<{self.inner.rs_name(False)}>"
        else:
            mut = "mut" if self.mut else "const"
            return f"*{mut} {self.inner.rs_name(False)}"
    
    def marshal(self, name: str) -> tuple[str, str]:
        ptr = "mut" if self.mut else "const"
        _mut = "_mut" if self.mut else ""
        Mut = "Mut" if self.mut else ""
        return (
            f"    {name}: *{ptr} {self.inner.rs_name(False)},\n",
            f"    let {name} = match UserPtr{Mut}::new{_mut}({name}) {{\n"+
            f"        Ok({name}) => {name},\n"+
            f"        Err(_) => return -(Errno::EFAULT as i32) as _,\n"+
            f"    }};\n"
        )

class ArrType(Type):
    def __init__(self, mut: bool, inner: BaseType|PtrType, len: int|None):
        self.mut = mut
        self.inner = inner
        self.len = len
    
    def c_name(self, ident: str | None) -> str:
        if self.len == None:
            const = "" if self.mut else " const"
            return f"{self.inner.c_name(None)}{const} *{ident}, __mlibc_usize {ident}_len"
        else:
            return f"{self.inner.c_name(None)}{ident or ""}[{self.len}]"
    
    def rs_name(self, as_param: bool) -> str:
        if as_param:
            mut = "Mut" if self.mut else ""
            return f"UserPtr{mut}<[{self.inner.rs_name(False)}; {self.len}]>"
        else:
            return f"[{self.inner.rs_name(False)}; {self.len}]"
    
    def marshal(self, name: str) -> tuple[str, str]:
        ptr = "mut" if self.mut else "const"
        _mut = "_mut" if self.mut else ""
        Mut = "Mut" if self.mut else ""
        if self.len == None:
            return (
                f"    {name}: *{ptr} {self.inner.rs_name(False)},\n"+
                f"    {name}_len: usize\n",
                f"    let {name} = match UserSlice{Mut}::new{_mut}({name}, {name}_len) {{\n"+
                f"        Ok({name}) => {name},\n"+
                f"        Err(_) => return -(Errno::EFAULT as i32) as _,\n"+
                f"    }};\n"
            )
        else:
            return (
                f"    {name}: *{ptr} [{self.inner.rs_name(False)}; {self.len}],\n",
                f"    let {name} = match UserPtr{Mut}::new{_mut}({name}) {{\n"+
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
        mut = False
        if inner.startswith("mut "):
            mut = True
            inner = inner[4:]
        if len.strip() == "":
            return ArrType(mut, parse_inner_type(inner), None)
        else:
            try:
                len = int(len.strip())
            except ValueError:
                print(f"Invalid length `{len.strip()}`")
                exit(1)
            return ArrType(mut, parse_inner_type(inner), len)
    
    elif raw[0] == '*':
        inner = raw[1:]
        inner = inner.strip()
        mut = False
        if inner.startswith("mut "):
            mut = True
            inner = inner[4:]
        return PtrType(mut, parse_inner_type(inner))
        
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
        out += [parse_type(type)]
    return out

class Syscall:
    def __init__(self, row: dict[str,str]):
        self.index = int(row["Index"])
        self.namespace = row["Namespace"]
        self.name = row["Name"]
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
            self.returns = base_types["i32"]
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
            if type.header != None:
                need_header.add(type.header)
            base_types[type.name] = type
        except KeyError as k:
            print(f"systype.csv: missing cell `{k.args[0]}` in row {i+1}")
            exit(1)

def load_syscalls():
    csv = read_csv(abi_dir + "/systab.csv")
    for i in range(len(csv)):
        try:
            syscall = Syscall(csv[i])
            syscalls[syscall.index] = syscall
            namespaces.add(syscall.namespace)
        except KeyError as k:
            print(f"systab.csv: missing cell `{k.args[0]}` in row {i+1}")
            exit()

def gen_c_header():
    with open(abi_dir + "/syscall.h", "w") as fd:
        fd.write(gen_warning)

def gen_c_wrappers():
    with open(abi_dir + "/syscall.c", "w") as fd:
        fd.write(gen_warning)

def gen_rust_marshalling():
    os.mkdir(template_dir)
    with open(mashall_dir + "/mod.rs", "w") as marshal:
        marshal.write(gen_warning)
        marshal.write(imports)
        
        for namespace in namespaces:
            marshal.write(f"mod {namespace};\n")
            with open(f"{template_dir}/{namespace}.rs", "w") as template:
                template.write(gen_warning)
        
        # Syscall dispatcher.
        marshal.write("\npub fn dispatch(regs: &mut GpRegfile, sregs: &mut SpRegfile, args: [usize; 6], sysno: usize) {\n")
        marshal.write("    match sysno {\n")
        for syscall in syscalls.values():
            marshal.write(f"        {syscall.index} => regs.set_retval(marshal_{syscall.namespace}_{syscall.name}(")
            for i in range(len(syscall.params)):
                if i:
                    marshal.write(", ")
                marshal.write(f"args[{i}] as _")
            marshal.write(") as _),\n")
        marshal.write("        _ => regs.set_retval(-(Errno::ENOSYS as i32) as _),\n")
        marshal.write("    }\n")
        marshal.write("}\n")
        
        for syscall in syscalls.values():
            # Syscall templates.
            with open(f"{template_dir}/{syscall.namespace}.rs", "w") as template:
                template.write(f"\npub(super) fn {syscall.name}(\n")
                for param in syscall.params:
                    template.write(f"    {param.name}: {param.type.rs_name(False)}")
                if syscall.true_returns == None:
                    returns = ""
                else:
                    returns = syscall.true_returns.rs_name(False)
                template.write(f") -> EResult<{returns}> {{\n")
                template.write(f"    todo!();\n")
                template.write(f"}}\n")
            
            # Syscall marshalling code.
            paramstr = ""
            marshalstr = ""
            for param in syscall.params:
                (param, martial) = param.type.marshal(param.name)
                paramstr += param
                marshalstr += martial
            marshal.write(f"\nfn marshal_{syscall.namespace}_{syscall.name}(\n")
            marshal.write(paramstr)
            marshal.write(f") -> {syscall.returns.rs_name(False)} {{")
            marshal.write(marshalstr)
            marshal.write("}\n")

load_base_types()
print(base_types)
load_syscalls()
print(syscalls)
gen_c_header()
gen_c_wrappers()
gen_rust_marshalling()
