#!/usr/bin/env python3

# SPDX-License-Identifier: MIT

from argparse import ArgumentParser
import os, subprocess, re

assert __name__ == "__main__"



def prompt_choice(name: str, options: list[str], preferred: int|None) -> int:
    global use_defaults
    if use_defaults and preferred != None:
        return preferred
    print()
    print(f"Options for {name}:")
    for i in range(len(options)):
        print(f"[{i+1}] {options[i]}")
    while True:
        try:
            if preferred != None:
                resp = input(f"Select {name} [{preferred+1}] ")
            else:
                resp = input(f"Select {name}: ")
            if resp.strip() == "" and preferred != None:
                return preferred
            index = int(resp)
            assert index >= 1 and index <= len(options)
            return index - 1
        except (KeyboardInterrupt, EOFError):
            print()
            exit(1)
        except:
            pass

def prompt_yesno(name: str, default: bool|None = None) -> bool:
    global use_defaults
    if use_defaults and default != None:
        return default
    print()
    while True:
        try:
            if default == True:
                resp = input(f"{name} [Y/n] ")
            elif default == False:
                resp = input(f"{name} [y/N] ")
            else:
                resp = input(f"{name} [y/n] ")
            if resp.strip() == "" and default != None:
                return default
            elif resp.strip().lower() in ["y", "yes"]:
                return True
            elif resp.strip().lower() in ["n", "no"]:
                return False
        except (KeyboardInterrupt, EOFError):
            print()
            exit(1)
        except:
            pass

def query_compiler_arch(path) -> str|None:
    res = subprocess.run([path, '-dumpmachine'], capture_output=True)
    if res.returncode != 0: return None
    return res.stdout.decode().strip()

def test_candidate(prefix: str, compiler_name: str) -> bool:
    global config
    if not os.path.exists(prefix + "objdump") or not os.path.exists(prefix + "addr2line"):
        return False
    cc_arch = query_compiler_arch(prefix + compiler_name)
    if cc_arch == None:
        return False
    return config["cpu"] in cc_arch and ("badgeros" in cc_arch or "linux-gnu" in cc_arch)



known_compiler_names = ["gcc", "cc", "clang"]
architectures = ["riscv64", "x86_64"]

parser = ArgumentParser(usage="Helper program for selecting the toolchain to use for compiling BadgerOS")
parser.add_argument("--arch", choices=architectures, default=None, help="")
parser.add_argument("--default", action="store_true", help="Select default option for all prompts")
args = parser.parse_args()
use_defaults = args.default
config: dict[str, str] = {}

try:
    host_arch = architectures.index(os.uname().machine)
except:
    host_arch = None
config["cpu"] = args.arch or architectures[prompt_choice("architecture", architectures, host_arch)]

candidates: list[str] = []
prefixes: list[str] = []

for dir in os.getenv("PATH").split(os.pathsep): # type: ignore
    try:
        files = os.listdir(dir)
    except:
        continue
    for filename in files:
        for compiler_name in known_compiler_names:
            if filename.endswith(compiler_name):
                if len(filename) > len(compiler_name) and filename[-len(compiler_name)-1] != '-':
                    continue
                prefix = dir + '/' + filename[:-len(compiler_name)]
                if test_candidate(prefix, compiler_name) and prefix + compiler_name not in candidates:
                    candidates.append(prefix + compiler_name)
                    prefixes.append(prefix)
                break

compiler = prompt_choice("compiler", candidates, 0)
config["compiler"] = candidates[compiler]
config["prefix"] = prefixes[compiler]

if os.uname().machine == config["cpu"] and prompt_yesno("Use native ISA?", True):
    config["isa_spec"] = "native"
    if config["cpu"].startswith("riscv"):
        config["kisa_spec"] = "rv" + config["cpu"][5:] + "imac_zicsr_zifencei"
    else:
        config["kisa_spec"] = "x86-64"
elif config["cpu"].startswith("riscv"):
    rv_float_enum = ["none", "single", "double"]
    rv_float_isa  = ["", "f", "fd"]
    float_spec = prompt_choice("float spec", rv_float_enum, 2)
    use_vec = prompt_yesno("Support vector extensions?", True)
    config["rv_float_spec"] = rv_float_enum[float_spec]
    config["rv_use_vector"] = str(use_vec)
    config["isa_spec"] = "rv" + config["cpu"][5:] + "ima" + rv_float_isa[float_spec] + "c" + ("v" if use_vec else "") + "_zicsr_zifencei"
    config["kisa_spec"] = "rv" + config["cpu"][5:] + "imac_zicsr_zifencei"
else:
    x64_arch_enum = ["minimum", "~2008 onwards", "~2013 onwards"]
    x64_arch_isa  = ["x86-64", "x86-64-v2", "x86-64-v3"]
    config["isa_spec"] = x64_arch_isa[prompt_choice("approx. architecture", x64_arch_enum, 2)]
    config["kisa_spec"] = "x86-64"

if config["cpu"].startswith("riscv"):
    config["kabi_spec"] = "lp64"
    config["abi_spec"] = config["kabi_spec"] + ["", "f", "d"][float_spec] # type: ignore
else:
    config["abi_spec"] = "sysv"
    config["kabi_spec"] = "sysv"

if not use_defaults:
    print()
    print()
print(f"Selected architecture {config["cpu"]} and compiler {config["compiler"]}")
print(f"Userspace is using -march={config["isa_spec"]} and -mabi={config["abi_spec"]}")
print(f"Kernel is using -march={config["kisa_spec"]} and -mabi={config["kabi_spec"]}")



config_path = os.path.dirname(os.path.abspath(__file__)) + "/../.config"
os.makedirs(config_path, exist_ok=True)

with open(config_path + "/config.mk", "w") as fd:
    fd.write(f'# WARNING: This is a generated file, do not edit it!\n')
    for opt in config:
        fd.write(f'CONFIG_{opt.upper()} = {config[opt]}\n')

with open(config_path + "/config.cmake", "w") as fd:
    fd.write(f'# WARNING: This is a generated file, do not edit it!\n')
    for opt in config:
        fd.write(f'set(CONFIG_{opt.upper()} {config[opt]})\n')
        if re.match("^\\w+$", config[opt]):
            fd.write(f'add_definitions(-DCONFIG_{opt.upper()}={config[opt]})\n')
            fd.write(f'add_definitions(-DCONFIGENUM_{opt.upper()}_{config[opt]})\n')
        fd.write(f'add_definitions(-DCONFIGSTR_{opt.upper()}="{config[opt]}")\n')
