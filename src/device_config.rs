//! A small, non-executable device map. The device catalogue in the compiler
//! owns vector layout, CPU family and file format; files only constrain memory
//! and declare an allowed subset of CPU instruction capabilities.
use std::{collections::HashMap, fs, path::Path};

type Res<T> = Result<T,String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cpu { Avr5, CortexM3, CortexM4 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Output { Ihex, Elf32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region { pub start: u32, pub end: u32 } // half open
impl Region { pub fn len(self)->u32 { self.end-self.start } }

// No dependency or feature-expression language: strictly named capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Features(pub u32);
impl Features {
    pub const JMP:u32=1<<0;
    pub const CALL:u32=1<<1;
    pub const MUL:u32=1<<2;
    pub const LPM:u32=1<<3;
    pub const THUMB2:u32=1<<4;
    pub const FPV4_SP:u32=1<<5;
    pub const DSP:u32=1<<6;
    pub const UDIV:u32=1<<7;
    pub fn bit(s:&str)->Option<u32>{
        Some(match s {
            "jmp"=>Self::JMP, "call"=>Self::CALL, "mul"=>Self::MUL,
            "lpm"=>Self::LPM, "thumb2"=>Self::THUMB2,
            "fpv4_sp"=>Self::FPV4_SP, "dsp"=>Self::DSP,
            "udiv"=>Self::UDIV, _=>return None,
        })
    }
    pub fn has(self,feature:&str)->bool{Self::bit(feature).is_some_and(|bit|self.0 & bit != 0)}
    pub fn names(self)->Vec<&'static str>{
        ["jmp","call","mul","lpm","thumb2","fpv4_sp","dsp","udiv"]
            .into_iter().filter(|s|self.has(s)).collect()
    }
    pub fn parse(value:&str)->Res<Self>{
        if value=="none" {return Ok(Self(0));}
        let mut bits=0u32;
        for item in value.split(',') {
            let name=item.trim();
            let bit=Self::bit(name).ok_or_else(||format!("unknown CPU feature: {name}"))?;
            if bits & bit !=0{return Err(format!("duplicate CPU feature: {name}"));}
            bits |= bit;
        }
        Ok(Self(bits))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeviceLayout {
    pub device_id: &'static str,
    pub cpu: Cpu,
    pub flash: Region,
    pub ram: Region,
    pub stack_reserve: u32,
    pub vectors: u16, // compiler catalogue, never device map
    pub output: Output, // compiler backend, never device map
    pub features: Features,
}
impl DeviceLayout {
    pub fn stack_top(self)->u16 {(self.ram.end-1) as u16} // AVR only
    pub fn static_limit(self)->u32 {self.ram.end-self.stack_reserve}
}
#[derive(Debug)]
pub(crate) struct DeviceProfile {pub name:String,pub layout:DeviceLayout}

struct DeviceSpec {
    name:&'static str,
    cpu:Cpu,
    vectors:u16,
    output:Output,
    supported:u32,
    required:u32,
}
// Hardware-dependent startup details live here, *not* in the target files.
// Adding a new chip with a distinct IRQ layout needs a reviewed catalogue entry.
pub(crate) fn known_device(name:&str)->bool {device_spec(name).is_some()}
fn device_spec(name:&str)->Option<DeviceSpec>{
    use crate::device_config::Features as F;
    let (cpu,vectors,output,supported,required)=match name {
        "atmega328p"|"atmega168p"=>(Cpu::Avr5,26,Output::Ihex,
            F::JMP|F::CALL|F::MUL|F::LPM,F::JMP|F::CALL),
        "stm32f103c8"=>(Cpu::CortexM3,59,Output::Elf32,
            F::THUMB2|F::UDIV,F::THUMB2),
        "stm32f401cc"=>(Cpu::CortexM4,101,Output::Elf32,
            F::THUMB2|F::UDIV|F::DSP|F::FPV4_SP,F::THUMB2),
        _=>return None,
    };
    let canonical_name=match name {
        "atmega328p"=>"atmega328p", "atmega168p"=>"atmega168p",
        "stm32f103c8"=>"stm32f103c8", "stm32f401cc"=>"stm32f401cc",
        _=>return None,
    };
    Some(DeviceSpec{name:canonical_name,
        cpu,vectors,output,supported,required})
}

fn number(s:&str)->Res<u32>{
    if s.is_empty() || s.starts_with('+') || s.starts_with('-') || s.trim()!=s {
        return Err(format!("invalid unsigned number: {s}"));
    }
    let result=if let Some(hex)=s.strip_prefix("0x") {
        if hex.is_empty() || !hex.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err(format!("invalid hex number: {s}"));
        }
        u32::from_str_radix(hex,16)
    }else{
        if !s.bytes().all(|b|b.is_ascii_digit()) {return Err(format!("invalid decimal number: {s}"));}
        s.parse::<u32>()
    };
    result.map_err(|_|format!("number out of range: {s}"))
}
fn region(s:&str)->Res<Region>{
    let (a,b)=s.split_once("..").ok_or_else(||format!("expected start..end region: {s}"))?;
    let start=number(a.trim())?;
    let end=number(b.trim())?;
    if start>=end {return Err(format!("empty/reversed region: {s}"));}
    Ok(Region{start,end})
}
impl DeviceProfile {
    pub fn load(path:&Path)->Res<Self>{
        let bytes=fs::read(path).map_err(|e|format!("{}: {e}",path.display()))?;
        if bytes.len()>8192{return Err("device map exceeds 8 KiB".into());}
        let s=std::str::from_utf8(&bytes).map_err(|_|"device map must be UTF-8")?;
        Self::parse(s).map_err(|e|format!("{}: {e}",path.display()))
    }
    pub fn parse(src:&str)->Res<Self>{
        if src.len()>8192{return Err("device map exceeds 8 KiB".into());}
        let mut fields=HashMap::new();
        for (line_no,line) in src.lines().enumerate(){
            if line.len()>256{return Err(format!("line {} too long",line_no+1));}
            let text=line.split('#').next().unwrap_or("").trim();
            if text.is_empty(){continue;}
            let (key,value)=text.split_once('=').ok_or_else(||format!("line {}: expected key = value",line_no+1))?;
            let key=key.trim();let value=value.trim();
            if !["device","flash","ram","stack","features"].contains(&key){
                return Err(format!("line {}: unknown device key {key}",line_no+1));
            }
            if value.is_empty() || value.contains('='){
                return Err(format!("line {}: missing/invalid value for {key}",line_no+1));
            }
            if fields.insert(key,value.to_string()).is_some(){
                return Err(format!("line {}: duplicate device key {key}",line_no+1));
            }
        }
        let get=|key:&str|fields.get(key).map(|s|s.as_str()).ok_or_else(||format!("missing device key: {key}"));
        let name=get("device")?;
        if name.len()>48 || name.is_empty() || !name.bytes().all(|b|b.is_ascii_lowercase() || b.is_ascii_digit() || b==b'_') {
            return Err("invalid device identifier".into());
        }
        let spec=device_spec(name).ok_or_else(||format!("unsupported device '{name}' (no reviewed startup/vector definition in compiler)"))?;
        let flash=region(get("flash")?)?;
        let ram=region(get("ram")?)?;
        let stack_reserve=number(get("stack")?)?;
        let features=Features::parse(get("features")?)?;
        if features.0 & !spec.supported != 0{return Err(format!("device '{name}': requested CPU feature not supported by this chip"));}
        if features.0 & spec.required != spec.required{return Err(format!("device '{name}': missing mandatory backend instruction capability"));}
        let layout=DeviceLayout{device_id:spec.name,cpu:spec.cpu,flash,ram,stack_reserve,vectors:spec.vectors,output:spec.output,features};
        layout.validate()?;
        Ok(DeviceProfile{name:spec.name.into(),layout})
    }
}
impl DeviceLayout {
    fn validate(self)->Res<()> {
        let s=self;
        if s.stack_reserve==0 || s.stack_reserve>=s.ram.len(){return Err("stack reserve must fit inside RAM".into());}
        match s.cpu {
            Cpu::Avr5=>{
                if s.output!=Output::Ihex{return Err("avr5 needs ihex output".into());}
                if s.flash.start!=0 || s.flash.end>65536 || s.flash.end%2!=0{
                    return Err("avr5 flash must start at zero and fit in 64 KiB".into());
                }
                if s.ram.start<0x100 || s.ram.start%2!=0 || s.ram.end>65536 || s.ram.end%2!=0{
                    return Err("avr5 data RAM addresses must be even aligned, >= 0x100 and <= 0xFFFF".into());
                }
                if s.stack_reserve<128 || s.stack_reserve%2!=0 || s.static_limit()<=s.ram.start+2{
                    return Err("avr5 needs >=128 byte aligned stack reserve and static RAM".into());
                }
                if s.flash.len()<u32::from(s.vectors)*4+8{return Err("AVR flash too small for vector table".into());}
            }
            Cpu::CortexM3|Cpu::CortexM4=>{
                if s.flash.start<s.ram.end && s.ram.start<s.flash.end{return Err("Cortex-M flash and RAM overlap".into());}
                if s.output!=Output::Elf32{return Err("Cortex-M needs elf32 output".into());}
                if s.flash.start%128!=0 || s.flash.end%4!=0 || s.ram.start%8!=0 || s.ram.end%8!=0{
                    return Err("Cortex-M memory regions misaligned".into());
                }
                if s.stack_reserve<256 || s.stack_reserve%8!=0 || s.vectors<16{
                    return Err("Cortex-M needs >=256 byte aligned stack and >=16 vectors".into());
                }
                if s.flash.len()<u32::from(s.vectors)*4+8{return Err("Cortex-M flash too small for vector table".into());}
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn avr()->&'static str{include_str!("../targets/devices/atmega328p.target")}
    #[test] fn shipped_profiles(){
        let a=DeviceProfile::parse(avr()).unwrap();
        assert_eq!(a.layout.ram,Region{start:0x100,end:0x900});
        assert_eq!(a.layout.stack_top(),0x08ff);
        assert_eq!(a.layout.static_limit(),0x800);
        assert_eq!(a.layout.vectors,26);
        assert_eq!(a.layout.output,Output::Ihex);
        assert!(a.layout.features.has("mul"));
        let b=DeviceProfile::parse(include_str!("../targets/devices/atmega168p.target")).unwrap();
        assert_eq!(b.layout.ram.end,0x500);
        let m=DeviceProfile::parse(include_str!("../targets/devices/stm32f103c8.target")).unwrap();
        assert_eq!(m.layout.cpu,Cpu::CortexM3);
        assert!(m.layout.features.has("thumb2"));
        assert!(!m.layout.features.has("fpv4_sp"));
        let fpu=DeviceProfile::parse(include_str!("../targets/devices/stm32f401cc.target")).unwrap();
        assert_eq!(fpu.layout.cpu,Cpu::CortexM4);
        assert!(fpu.layout.features.has("fpv4_sp"));
        assert_eq!(fpu.layout.vectors,101);
    }
    #[test] fn feature_subset_is_typed(){
        assert!(Features::parse("thumb2,fpv4_sp").unwrap().has("fpv4_sp"));
        assert!(!Features::parse("thumb2").unwrap().has("fpv4_sp"));
        assert!(Features::parse("thumb2,thumb2").is_err());
        assert!(Features::parse("arbitrary_opcode").is_err());
    }
    #[test] fn bad_keys_and_capabilities(){
        assert!(DeviceProfile::parse(&format!("{avr}\nstack = 512",avr=avr())).is_err());
        assert!(DeviceProfile::parse(&avr().replace("features = jmp,call,mul,lpm","features = jmp,call,mul,mul")).is_err());
        assert!(DeviceProfile::parse(&avr().replace("features = jmp,call,mul,lpm","features = jmp,call,fpv4_sp")).is_err());
        assert!(DeviceProfile::parse(&avr().replace("features = jmp,call,mul,lpm","features = mul")).is_err());
        assert!(DeviceProfile::parse(&avr().replace("stack = 256","stack = -1")).is_err());
        assert!(DeviceProfile::parse(&avr().replace("device = atmega328p","device = unknown_mcu")).is_err());
        assert!(DeviceProfile::parse(&avr().replace("flash = 0x0000..0x8000","flash = 0x0000..0x10001")).is_err());
        assert!(DeviceProfile::parse(&format!("{avr}\nvectors = 26",avr=avr())).is_err());
        let arm=include_str!("../targets/devices/stm32f103c8.target");
        assert!(DeviceProfile::parse(&arm.replace("features = thumb2,udiv","features = thumb2,fpv4_sp")).is_err());
    }
}
