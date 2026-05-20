use std::collections::HashMap;
use memmap2::{Mmap, MmapMut};

pub type JitFunction = extern "C" fn(*mut u32) -> u32;

pub struct JitCompiler {
    compiled_methods: HashMap<String, (Mmap, JitFunction)>,
}


fn hw_reg(v: u8) -> (u8, u8) {
    match v {
        0 => (0, 3), 
        1 => (1, 4), 
        2 => (1, 5), 
        3 => (1, 6), 
        4 => (1, 7), 
        5 => (1, 0), 
        6 => (1, 1), 
        7 => (1, 2), 
        8 => (1, 3), 
        9 => (0, 1), 
        10 => (0, 2), 
        _ => unreachable!(),
    }
}

fn emit_mov_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x8b); mc.push(0xc0 | (rg_d << 3) | rg_s);
}

fn emit_add_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x03); mc.push(0xc0 | (rg_d << 3) | rg_s);
}

fn emit_xor_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x33); mc.push(0xc0 | (rg_d << 3) | rg_s); 
}

fn emit_sub_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x2b); mc.push(0xc0 | (rg_d << 3) | rg_s); 
}

fn emit_and_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x23); mc.push(0xc0 | (rg_d << 3) | rg_s); 
}

fn emit_or_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x0b); mc.push(0xc0 | (rg_d << 3) | rg_s); 
}

fn emit_mul_rr(mc: &mut Vec<u8>, dest: u8, src: u8) {
    let (rx_d, rg_d) = hw_reg(dest); let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | (rx_d << 2) | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x0f); mc.push(0xaf); mc.push(0xc0 | (rg_d << 3) | rg_s); 
}

fn emit_add_ri8(mc: &mut Vec<u8>, dest: u8, imm: i8) {
    let (rx_d, rg_d) = hw_reg(dest);
    let rex = 0x40 | rx_d; if rex != 0x40 { mc.push(rex); }
    mc.push(0x83); mc.push(0xc0 | rg_d); mc.push(imm as u8);
}

fn emit_mov_ri32(mc: &mut Vec<u8>, dest: u8, imm: u32) {
    let (rx_d, rg_d) = hw_reg(dest);
    let rex = 0x40 | rx_d; if rex != 0x40 { mc.push(rex); }
    mc.push(0xb8 + rg_d); mc.extend_from_slice(&imm.to_le_bytes());
}

fn emit_cmp_rr(mc: &mut Vec<u8>, left: u8, right: u8) {
    let (rx_l, rg_l) = hw_reg(left); let (rx_r, rg_r) = hw_reg(right);
    let rex = 0x40 | (rx_l << 2) | rx_r; if rex != 0x40 { mc.push(rex); }
    mc.push(0x3b); mc.push(0xc0 | (rg_l << 3) | rg_r);
}

fn emit_ret_val(mc: &mut Vec<u8>, src: u8) {
    let (rx_s, rg_s) = hw_reg(src);
    let rex = 0x40 | rx_s; if rex != 0x40 { mc.push(rex); }
    mc.push(0x8b); mc.push(0xc0 | rg_s);
}

impl JitCompiler {
    pub fn new() -> Self {
        JitCompiler {
            compiled_methods: HashMap::new(),
        }
    }

    pub fn get_compiled(&self, signature: &str) -> Option<JitFunction> {
        self.compiled_methods.get(signature).map(|(_, f)| *f)
    }

    pub fn compile(&mut self, signature: &str, code: &crate::dex::CodeItem) -> bool {
        if code.header.registers_size > 11 {
            return false; 
        }

        
        let mut pc = 0;
        while pc < code.insns.len() {
            let opcode = (code.insns[pc] & 0xFF) as u8;
            match opcode {
                0x00 | 0x01 | 0x0f | 0x12 | 0x28 | 0xb0 => pc += 1,
                0x13 | 0x15 | 0x90 | 0x91 | 0x92 | 0x95 | 0x96 | 0x97 | 0xd8 | 0x32..=0x37 => pc += 2, 
                0x14 => pc += 3,
                _ => return false,
            }
        }

        let mut pc_to_native_offset = HashMap::new();
        let mut mc = Vec::new();
        
        for pass in 0..2 {
            mc.clear();
            let mut pc = 0;
            
            
            mc.push(0x53); 
            mc.push(0x41); mc.push(0x54); 
            mc.push(0x41); mc.push(0x55); 
            mc.push(0x41); mc.push(0x56); 
            mc.push(0x41); mc.push(0x57); 
            for v in 0..code.header.registers_size {
                let (rx_v, rg_v) = hw_reg(v as u8);
                let rex = 0x40 | (rx_v << 2); if rex != 0x40 { mc.push(rex); }
                mc.push(0x8b); mc.push(0x40 | (rg_v << 3) | 7); mc.push((v * 4) as u8); 
            }

            while pc < code.insns.len() {
                if pass == 0 {
                    pc_to_native_offset.insert(pc, mc.len());
                }
                
                let native_pc = mc.len();
                let insn = code.insns[pc];
                let opcode = (insn & 0xFF) as u8;

                match opcode {
                    0x00 => { 
                        pc += 1;
                    }
                    0x01 => { 
                        let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        pc += 1;
                    }
                    0x0f => { 
                        let a = (insn as usize >> 8) & 0xFF;
                        emit_ret_val(&mut mc, a as u8);
                        
                        for v in 0..code.header.registers_size {
                            let (rx_v, rg_v) = hw_reg(v as u8);
                            let rex = 0x40 | (rx_v << 2); if rex != 0x40 { mc.push(rex); }
                            mc.push(0x89); mc.push(0x40 | (rg_v << 3) | 7); mc.push((v * 4) as u8); 
                        }
                        mc.push(0x41); mc.push(0x5f); 
                        mc.push(0x41); mc.push(0x5e); 
                        mc.push(0x41); mc.push(0x5d); 
                        mc.push(0x41); mc.push(0x5c); 
                        mc.push(0x5b); 
                        mc.push(0xc3); 
                        pc += 1;
                    }
                    0x12 => { 
                        let a = (insn as usize >> 8) & 0xF; let b = (insn >> 12) as i16 as i32 as u32;
                        emit_mov_ri32(&mut mc, a as u8, b);
                        pc += 1;
                    }
                    0x13 | 0x15 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let val = code.insns[pc] as i16 as i32 as u32;
                        let fval = if opcode == 0x13 { val } else { (code.insns[pc] as u32) << 16 };
                        emit_mov_ri32(&mut mc, a as u8, fval);
                        pc += 1;
                    }
                    0x14 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let l = code.insns[pc] as u32; pc += 1;
                        let h = code.insns[pc] as u32;
                        emit_mov_ri32(&mut mc, a as u8, (h << 16) | l);
                        pc += 1;
                    }
                    0x90 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_add_rr(&mut mc, a as u8, c as u8);
                        pc += 1;
                    }
                    0x91 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_sub_rr(&mut mc, a as u8, c as u8);
                        pc += 1;
                    }
                    0x92 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_mul_rr(&mut mc, a as u8, c as u8);
                        pc += 1;
                    }
                    0x95 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_and_rr(&mut mc, a as u8, c as u8);
                        pc += 1;
                    }
                    0x96 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_or_rr(&mut mc, a as u8, c as u8);
                        pc += 1;
                    }
                    0x97 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_xor_rr(&mut mc, a as u8, c as u8);
                        pc += 1;
                    }
                    0xb0 => { 
                        let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF;
                        emit_add_rr(&mut mc, a as u8, b as u8);
                        pc += 1;
                    }
                    0xd8 => { 
                        let a = (insn as usize >> 8) & 0xFF; pc += 1;
                        let next = code.insns[pc]; let b = next as usize & 0xFF; let lit = (next >> 8) as i8;
                        emit_mov_rr(&mut mc, a as u8, b as u8);
                        emit_add_ri8(&mut mc, a as u8, lit);
                        pc += 1;
                    }
                    0x28 => { 
                        let off = (insn as usize >> 8) as i8 as i32;
                        let target_pc = (pc as i32 + off) as usize;
                        mc.push(0xe9); 
                        if pass == 0 {
                            mc.extend_from_slice(&0u32.to_le_bytes());
                        } else {
                            let target_native = pc_to_native_offset[&target_pc];
                            let current_native = native_pc + 5;
                            let rel32 = (target_native as i32 - current_native as i32) as u32;
                            mc.extend_from_slice(&rel32.to_le_bytes());
                        }
                        pc += 1;
                    }
                    0x32..=0x37 => { 
                        let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF;
                        pc += 1; let off = code.insns[pc] as i16 as i32;
                        let target_pc = ((pc - 1) as i32 + off) as usize;
                        
                        emit_cmp_rr(&mut mc, a as u8, b as u8);
                        
                        let jcc = match opcode { 0x32 => 0x84, 0x33 => 0x85, 0x34 => 0x8c, 0x35 => 0x8d, 0x36 => 0x8f, 0x37 => 0x8e, _ => unreachable!() };
                        mc.push(0x0f); mc.push(jcc);
                        if pass == 0 {
                            mc.extend_from_slice(&0u32.to_le_bytes());
                        } else {
                            let target_native = pc_to_native_offset[&target_pc];
                            let current_native = mc.len() + 4; 
                            let rel32 = (target_native as i32 - current_native as i32) as u32;
                            mc.extend_from_slice(&rel32.to_le_bytes());
                        }
                        pc += 1;
                    }
                    _ => unreachable!(),
                }
            }
            if pass == 0 {
                pc_to_native_offset.insert(pc, mc.len());
            }
        }

        let mut mmap = match MmapMut::map_anon(mc.len()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mmap.copy_from_slice(&mc);
        let mmap = match mmap.make_exec() {
            Ok(m) => m,
            Err(_) => return false,
        };

        let func_ptr = mmap.as_ptr();
        let func: JitFunction = unsafe { std::mem::transmute(func_ptr) };

        println!("[JIT] Compiled with 100% NATIVE Register Allocation: {}", signature);
        self.compiled_methods.insert(signature.to_string(), (mmap, func));
        true
    }
}
