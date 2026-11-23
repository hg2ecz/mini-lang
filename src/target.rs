//! CPU/OS ABI selection is separate from an untrusted MCU memory map.
use crate::device_config::{Cpu, DeviceLayout, DeviceProfile};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arch { X86_64, Aarch64, Armv7, Thumbv7m, Riscv32, Riscv64, Avr8, Mos6510 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Environment { Linux, BareMetal, Commodore64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FloatAbi { None, Soft, Hard }
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub arch: Arch,
    pub environment: Environment,
    pub address_bits: u8,
    /// Calling convention choice; independent of whether the MCU has an FPU.
    pub float_abi: FloatAbi,
    /// Applies only to bare-metal MCUs. No device-specific code in common IR.
    pub layout: Option<DeviceLayout>,
}
impl Target {
    pub(crate) fn parse(triple: &str) -> Option<Self> {
        use Arch::*;
        use Environment::*;
        use FloatAbi::*;
        let (arch, environment, address_bits, float_abi) = match triple {
            "x86_64-linux" => (X86_64, Linux, 64, Hard),
            "aarch64-linux" => (Aarch64, Linux, 64, Hard),
            "armv7-linux-soft" => (Armv7, Linux, 32, Soft),
            "armv7-linux-hard" => (Armv7, Linux, 32, Hard),
            "armv7-linux" => (Armv7, Linux, 32, Soft),
            "armv7-none-soft" => (Armv7, BareMetal, 32, Soft),
            "armv7-none-hard" => (Armv7, BareMetal, 32, Hard),
            "thumbv7m-none" => (Thumbv7m, BareMetal, 32, Soft),
            "riscv32-none" => (Riscv32, BareMetal, 32, Soft),
            "riscv32-linux" => (Riscv32, Linux, 32, Soft),
            "riscv64-linux" => (Riscv64, Linux, 64, Soft),
            "riscv64-none" => (Riscv64, BareMetal, 64, Soft),
            "avr8-none" | "avr8-atmega328p" => (Avr8, BareMetal, 16, None),
            "c64-6510" => (Mos6510, Commodore64, 16, None),
            _ => return Option::None,
        };
        // Compatibility target uses the *same* device map as --device.
        let layout=if triple=="avr8-atmega328p" {
            Some(DeviceProfile::parse(include_str!("../targets/devices/atmega328p.target")).ok()?.layout)
        }else{Option::None};
        Some(Target {arch,environment,address_bits,float_abi,layout})
    }
    pub(crate) fn from_device(profile:&DeviceProfile)->Self {
        match profile.layout.cpu {
            Cpu::Avr5 => Self{arch:Arch::Avr8,environment:Environment::BareMetal,address_bits:16,float_abi:FloatAbi::None,layout:Some(profile.layout)},
            Cpu::CortexM3|Cpu::CortexM4 => Self{arch:Arch::Thumbv7m,environment:Environment::BareMetal,address_bits:32,float_abi:FloatAbi::Soft,layout:Some(profile.layout)},
        }
    }
    pub(crate) fn device_name(&self)->&'static str {self.layout.map_or("none",|m|m.device_id)}
    pub(crate) fn feature(&self, name:&str)->bool { self.layout.is_some_and(|m|m.features.has(name)) }
    pub(crate) fn cpu_name(&self)->&'static str {
        match self.layout.map(|m|m.cpu) {
            Some(Cpu::Avr5)=>"avr5", Some(Cpu::CortexM3)=>"cortex-m3",
            Some(Cpu::CortexM4)=>"cortex-m4", None=>"none",
        }
    }
    pub(crate) fn usize_bits(&self) -> u8 { self.address_bits }
    pub(crate) fn usize_max(&self) -> u64 { match self.usize_bits() {16 => u16::MAX as u64, 32 => u32::MAX as u64, 64 => u64::MAX, _ => unreachable!()} }
    pub(crate) fn is_implemented(&self) -> bool {
        (matches!(self.arch,Arch::X86_64|Arch::Aarch64|Arch::Armv7) && self.environment == Environment::Linux)
            || (self.arch==Arch::Avr8 && self.environment==Environment::BareMetal && self.layout.is_some_and(|m|m.cpu==Cpu::Avr5))
            || (self.arch==Arch::Mos6510 && self.environment==Environment::Commodore64)
            || (self.arch==Arch::Thumbv7m && self.environment==Environment::BareMetal && self.layout.is_some_and(|m|matches!(m.cpu,Cpu::CortexM3|Cpu::CortexM4)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_capabilities() {
        assert!(Target::parse("x86_64-linux").unwrap().is_implemented());
        let avr = Target::parse("avr8-none").unwrap();
        assert_eq!(avr.address_bits,16);
        assert_eq!(avr.usize_bits(),16);
        assert_eq!(avr.usize_max(),65535);
        assert_eq!(avr.float_abi,FloatAbi::None);
        assert!(!avr.is_implemented());
        assert!(Target::parse("avr8-atmega328p").unwrap().is_implemented());
        let other=DeviceProfile::parse(include_str!("../targets/devices/atmega168p.target")).unwrap();
        assert!(Target::from_device(&other).is_implemented());
        let arm=DeviceProfile::parse(include_str!("../targets/devices/stm32f103c8.target")).unwrap();
        let thumb=Target::from_device(&arm);
        assert_eq!(thumb.usize_bits(),32);
        assert!(thumb.is_implemented());
        let c64=Target::parse("c64-6510").unwrap();
        assert!(c64.is_implemented());
        assert_eq!(c64.usize_bits(),16);
        let hard = Target::parse("armv7-linux-hard").unwrap();
        let soft = Target::parse("armv7-linux-soft").unwrap();
        assert!(hard.is_implemented());
        assert!(soft.is_implemented());
        assert_eq!(hard.float_abi,FloatAbi::Hard);
        assert_eq!(soft.float_abi,FloatAbi::Soft);
        assert!(Target::parse("riscv64-none").is_some());
        assert_eq!(Target::parse("armv7-linux").unwrap().usize_bits(),32);
        assert_eq!(Target::parse("aarch64-linux").unwrap().usize_bits(),64);
        assert!(Target::parse("aarch64-linux").unwrap().is_implemented());
        assert_eq!(Target::parse("riscv32-none").unwrap().usize_bits(),32);
    }
}
