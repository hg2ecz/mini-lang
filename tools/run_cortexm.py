#!/usr/bin/env python3
"""Run a Mini Cortex-M firmware image on the Unicorn emulator.

Usage: run_cortexm.py DEVICE.target FIRMWARE.elf [--max-steps N]

The firmware is executed from its reset vector and runs until a BKPT
instruction. Since Mini 4.9.1, `BKPT #1` marks the normal end (`program`
returned) and `BKPT #0` a Mini runtime trap. MMIO stores outside Flash/RAM are
logged so tests can check peripheral writes.

Exit status: 0 normal end, 3 trap, 4 step limit, 2 usage or emulator error.
Requires: pip install unicorn
"""
import sys

from unicorn import Uc, UcError, UC_ARCH_ARM, UC_MODE_THUMB, UC_MODE_MCLASS, UC_HOOK_CODE, UC_HOOK_MEM_WRITE, UC_HOOK_MEM_UNMAPPED
from unicorn.arm_const import UC_ARM_REG_SP, UC_ARM_REG_PC, UC_ARM_REG_R0

ELF_IMAGE_OFFSET = 0x100  # Mini's elf32_firmware places the image here.


def parse_range(text):
    start, end = text.split("..")
    return int(start, 0), int(end, 0)


def load_device(path):
    values = {}
    for line in open(path, encoding="utf-8"):
        line = line.split("#", 1)[0].strip()
        if "=" in line:
            key, value = (part.strip() for part in line.split("=", 1))
            values[key] = value
    return parse_range(values["flash"]), parse_range(values["ram"]), int(values.get("vectors", "16"), 0)


def main(argv):
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    max_steps = int(argv[argv.index("--max-steps") + 1]) if "--max-steps" in argv else 2_000_000
    (flash_start, flash_end), (ram_start, ram_end), vectors = load_device(argv[1])
    image = open(argv[2], "rb").read()[ELF_IMAGE_OFFSET:]
    if len(image) > flash_end - flash_start:
        print("run_cortexm: image larger than Flash", file=sys.stderr)
        return 2

    uc = Uc(UC_ARCH_ARM, UC_MODE_THUMB | UC_MODE_MCLASS)
    uc.mem_map(flash_start, (flash_end - flash_start + 0xFFF) & ~0xFFF)
    uc.mem_map(ram_start, (ram_end - ram_start + 0xFFF) & ~0xFFF)
    uc.mem_write(flash_start, image)

    initial_sp = int.from_bytes(image[0:4], "little")
    reset = int.from_bytes(image[4:8], "little") & ~1
    state = {"steps": 0, "result": None, "pc": None, "mmio": []}

    def on_code(emu, address, size, _):
        state["steps"] += 1
        if state["steps"] > max_steps:
            state["result"] = "limit"
            emu.emu_stop()
            return
        word = bytes(emu.mem_read(address, 2)) if size == 2 else b""
        if len(word) == 2 and word[1] == 0xBE:  # BKPT #imm8
            state["result"] = "exit" if word[0] == 1 else "trap"
            state["pc"] = address
            emu.emu_stop()

    def on_write(emu, access, address, size, value, _):
        if not (flash_start <= address < flash_end or ram_start <= address < ram_end):
            state["mmio"].append((address, size, value))

    def on_unmapped(emu, access, address, size, value, _):
        if not (flash_start <= address < flash_end or ram_start <= address < ram_end):
            state["mmio"].append((address, size, value))
            emu.mem_map(address & ~0xFFF, 0x1000)  # model a peripheral page
            return True
        return False

    uc.hook_add(UC_HOOK_CODE, on_code)
    uc.hook_add(UC_HOOK_MEM_WRITE, on_write)
    uc.hook_add(UC_HOOK_MEM_UNMAPPED, on_unmapped)
    uc.reg_write(UC_ARM_REG_SP, initial_sp)
    try:
        uc.emu_start(reset | 1, flash_end, count=max_steps + 10)
    except UcError as error:
        print(f"run_cortexm: emulator error at pc=0x{uc.reg_read(UC_ARM_REG_PC):08x}: {error}", file=sys.stderr)
        return 2

    for address, size, value in state["mmio"]:
        print(f"mmio-write 0x{address:08x} size={size} value=0x{value:x}")
    r0 = uc.reg_read(UC_ARM_REG_R0)
    if state["result"] == "exit":
        print(f"exit r0={r0} steps={state['steps']}")
        return 0
    if state["result"] == "trap":
        print(f"trap pc=0x{state['pc']:08x} steps={state['steps']}")
        return 3
    print(f"step limit reached ({max_steps})")
    return 4


if __name__ == "__main__":
    sys.exit(main(sys.argv))
