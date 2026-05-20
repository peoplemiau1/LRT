use crate::dex::{DexError, DexResult};
use crate::vm::{Vm, Object};
use scroll::{Pread, LE};

pub fn execute_instruction(
    vm: &mut Vm,
    opcode: u8,
    insn: u16,
    pc: &mut usize,
    registers: &mut [u32],
    code: &crate::dex::CodeItem,
) -> DexResult<()> {
    match opcode {
        0x00 => { *pc += 1; }
        0x01..=0x03 => { 
            let (a, b) = if opcode == 0x01 {
                ((insn as usize >> 8) & 0xF, (insn as usize >> 12) & 0xF)
            } else if opcode == 0x02 {
                let a = (insn as usize >> 8) & 0xFF; *pc += 1; (a, code.insns[*pc] as usize)
            } else {
                *pc += 1; let a = code.insns[*pc] as usize; *pc += 1; (a, code.insns[*pc] as usize)
            };
            registers[a] = registers[b]; *pc += 1;
        }
        0x04..=0x06 => { 
            let (a, b) = if opcode == 0x04 {
                ((insn as usize >> 8) & 0xF, (insn as usize >> 12) & 0xF)
            } else if opcode == 0x05 {
                let a = (insn as usize >> 8) & 0xFF; *pc += 1; (a, code.insns[*pc] as usize)
            } else {
                *pc += 1; let a = code.insns[*pc] as usize; *pc += 1; (a, code.insns[*pc] as usize)
            };
            registers[a] = registers[b]; registers[a+1] = registers[b+1]; *pc += 1;
        }
        0x07..=0x09 => { 
            let (a, b) = if opcode == 0x07 {
                ((insn as usize >> 8) & 0xF, (insn as usize >> 12) & 0xF)
            } else if opcode == 0x08 {
                let a = (insn as usize >> 8) & 0xFF; *pc += 1; (a, code.insns[*pc] as usize)
            } else {
                *pc += 1; let a = code.insns[*pc] as usize; *pc += 1; (a, code.insns[*pc] as usize)
            };
            registers[a] = registers[b]; *pc += 1;
        }
        0x0a..=0x0c => { 
            let a = (insn as usize >> 8) & 0xFF; 
            registers[a] = vm.last_result.unwrap_or(0); 
            *pc += 1; 
        }
        0x0d => { 
            let a = (insn as usize >> 8) & 0xFF;
            registers[a] = vm.last_exception.unwrap_or(0);
            *pc += 1;
        }
        0x0e => { return Err(DexError::Return(None)); }
        0x0f..=0x11 => { let a = (insn as usize >> 8) & 0xFF; return Err(DexError::Return(Some(registers[a]))); }
        0x12 => { let a = (insn as usize >> 8) & 0xF; let b = (insn as i16 >> 12) as i32; registers[a] = b as u32; *pc += 1; }
        0x13 | 0x15 => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1;
            let val = code.insns[*pc] as i16 as i32 as u32;
            let fval = if opcode == 0x13 { val } else { (code.insns[*pc] as u32) << 16 };
            registers[a] = fval; *pc += 1;
        }
        0x14 => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1;
            let l = code.insns[*pc] as u32; *pc += 1;
            let h = code.insns[*pc] as u32;
            registers[a] = (h << 16) | l; *pc += 1;
        }
        0x16..=0x19 => {
            let a = (insn as usize >> 8) & 0xFF; *pc += 1;
            let val;
            if opcode == 0x16 {
                val = code.insns[*pc] as i16 as i64 as u64;
                *pc += 1;
            } else if opcode == 0x17 {
                val = (code.insns[*pc] as u32 | ((code.insns[*pc+1] as u32) << 16)) as i64 as u64;
                *pc += 2;
            } else if opcode == 0x18 {
                let l1 = code.insns[*pc] as u64;
                let l2 = code.insns[*pc+1] as u64;
                let l3 = code.insns[*pc+2] as u64;
                let l4 = code.insns[*pc+3] as u64;
                val = l1 | (l2 << 16) | (l3 << 32) | (l4 << 48);
                *pc += 4;
            } else {
                val = (code.insns[*pc] as u64) << 48;
                *pc += 1;
            }
            registers[a] = (val & 0xFFFFFFFF) as u32; registers[a+1] = (val >> 32) as u32;
        }
        0x1a | 0x1b => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1;
            let idx = if opcode == 0x1a { code.insns[*pc] as u32 } else { let l = code.insns[*pc] as u32; *pc += 1; let h = code.insns[*pc] as u32; (h << 16) | l };
            let s = vm.dex.get_string(idx)?; let obj = vm.alloc(Object::String(s)); registers[a] = obj; *pc += 1;
        }
        0x1c => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; 
            let _type_idx = code.insns[*pc] as u32;
            let obj = vm.alloc(Object::Instance { class_idx: 0xFFFFFFFE, fields: std::collections::HashMap::new() });
            registers[a] = obj; *pc += 1; 
        }
        0x1d => { let a = (insn as usize >> 8) & 0xFF; vm.monitor_enter(registers[a]); *pc += 1; }
        0x1e => { let a = (insn as usize >> 8) & 0xFF; vm.monitor_exit(registers[a]); *pc += 1; }
        0x1f => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF; *pc += 1; let type_idx = code.insns[*pc] as u32;
            let obj_id = registers[b];
            let mut class_idx = 0;
            let mut is_special = false;
            {
                let s = vm.state.lock().unwrap();
                if let Some(obj) = s.heap.get(obj_id as usize) {
                    match obj {
                        Object::Instance { class_idx: ci, .. } => { class_idx = *ci; }
                        Object::String(_) => { is_special = vm.dex.get_type(type_idx).unwrap_or_default() == "Ljava/lang/String;"; }
                        Object::Array { .. } => { is_special = vm.dex.get_type(type_idx).unwrap_or_default().starts_with('['); }
                        _ => {}
                    }
                }
            }
            let matches = if is_special {
                true
            } else if class_idx != 0 {
                let class_name = vm.dex.get_type(class_idx)?;
                let target_name = vm.dex.get_type(type_idx)?;
                vm.is_instance_of(&class_name, &target_name)?
            } else {
                false
            };
            registers[a] = if matches { 1 } else { 0 };
            *pc += 1;
        }
        0x20 => { 
            let a_reg = (insn as usize >> 8) & 0xFF;
            *pc += 1; let type_idx = code.insns[*pc] as u32;
            let mut class_idx = 0;
            if a_reg < registers.len() {
                let obj_id = registers[a_reg];
                {
                    let s = vm.state.lock().unwrap();
                    if let Some(obj) = s.heap.get(obj_id as usize) {
                        match obj {
                            Object::Instance { class_idx: ci, .. } => { class_idx = *ci; }
                            Object::String(_) => { if vm.dex.get_type(type_idx).unwrap_or_default() != "Ljava/lang/String;" { return Err(DexError::Exception(0)); } }
                            _ => {}
                        }
                    }
                }
                if class_idx != 0 {
                    let class_name = vm.dex.get_type(class_idx)?;
                    let target_name = vm.dex.get_type(type_idx)?;
                    if !vm.is_instance_of(&class_name, &target_name)? { return Err(DexError::Exception(0)); }
                }
            }
            *pc += 1;
        }
        0x21 => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF;
            registers[a] = vm.get_array_length(registers[b])? as u32;
            *pc += 1;
        }
        0x22 => { let a = (insn as usize >> 8) & 0xFF; *pc += 1; let idx = code.insns[*pc] as u32; let obj = vm.alloc(Object::Instance { class_idx: idx, fields: std::collections::HashMap::new() }); registers[a] = obj; *pc += 1; }
        0x23 => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF; *pc += 1; let size = registers[b] as usize; let type_idx = code.insns[*pc] as u32;
            let type_name = vm.dex.get_type(type_idx)?;
            let obj = vm.alloc(Object::Array { element_type: type_name, data: vec![0; size] });
            registers[a] = obj; *pc += 1;
        }
        0x24 | 0x25 => {
            let (_count, type_idx, args) = if opcode == 0x24 {
                let c = (insn as usize >> 12) & 0xF; *pc += 1; let ti = code.insns[*pc] as u32; *pc += 1; let r = code.insns[*pc];
                let mut a = Vec::new(); let ri = [(r & 0xF) as usize, ((r >> 4) & 0xF) as usize, ((r >> 8) & 0xF) as usize, ((r >> 12) & 0xF) as usize, (insn as usize >> 8) & 0xF];
                for i in 0..c { a.push(registers[ri[i]]); } (c, ti, a)
            } else {
                let c = (insn as usize >> 8) & 0xFF; *pc += 1; let ti = code.insns[*pc] as u32; *pc += 1; let sr = code.insns[*pc] as usize;
                let mut a = Vec::new(); for i in 0..c { a.push(registers[sr + i]); } (c, ti, a)
            };
            let type_name = vm.dex.get_type(type_idx)?;
            let obj = vm.alloc(Object::Array { element_type: type_name, data: args });
            vm.last_result = Some(obj);
            *pc += 1;
        }
        0x26 => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; let off = (code.insns[*pc] as u32 | ((code.insns[*pc+1] as u32) << 16)) as usize;
            let base_pc = *pc - 1;
            let mut ptr = base_pc + off * 2;
            let magic: u16 = code.insns[ptr]; ptr += 1;
            if magic == 0x0300 { 
                let size: u32 = code.insns[ptr] as u32 | ((code.insns[ptr+1] as u32) << 16); ptr += 2;
                let mut data = Vec::with_capacity(size as usize);
                for _ in 0..size { data.push(code.insns[ptr] as u32 | ((code.insns[ptr+1] as u32) << 16)); ptr += 2; }
                vm.fill_array_data(registers[a], &data)?;
            }
            *pc += 2;
        }
        0x28 => { let off = (insn >> 8) as i8 as i32; *pc = (*pc as i32 + off) as usize; return Ok(()); }
        0x29 => { let base_pc = *pc; *pc += 1; let off = code.insns[*pc] as i16 as i32; *pc = (base_pc as i32 + off) as usize; return Ok(()); }
        0x2a => { let base_pc = *pc; *pc += 1; let off = (code.insns[*pc] as u32 | ((code.insns[*pc+1] as u32) << 16)) as i32; *pc = (base_pc as i32 + off) as usize; return Ok(()); }
        0x2b | 0x2c => {
            let a = (insn as usize >> 8) & 0xFF; let base_pc = *pc; *pc += 1;
            let off = (code.insns[*pc] as u32 | ((code.insns[*pc+1] as u32) << 16)) as i32;
            let table_off = (base_pc as i32 + off) as usize;
            let val = registers[a] as i32; let mut found_off = 0;
            if opcode == 0x2b { 
                let size = code.insns[table_off + 1] as usize; let first = (code.insns[table_off + 2] as u32 | ((code.insns[table_off+3] as u32) << 16)) as i32;
                if val >= first && val < first + size as i32 { let v_idx = table_off + 4 + (val - first) as usize * 2; found_off = (code.insns[v_idx] as u32 | ((code.insns[v_idx+1] as u32) << 16)) as i32; }
            } else { 
                let size = code.insns[table_off + 1] as usize;
                for i in 0..size {
                    let k_idx = table_off + 2 + i * 2; let k = (code.insns[k_idx] as u32 | ((code.insns[k_idx+1] as u32) << 16)) as i32;
                    if k == val { let v_idx = table_off + 2 + size * 2 + i * 2; found_off = (code.insns[v_idx] as u32 | ((code.insns[v_idx+1] as u32) << 16)) as i32; break; }
                }
            }
            if found_off != 0 { *pc = (base_pc as i32 + found_off) as usize; } else { *pc += 2; }
        }
        0x2d..=0x31 => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; let next = code.insns[*pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
            if opcode <= 0x2e { 
                let vb = f32::from_bits(registers[b]); let vc = f32::from_bits(registers[c]);
                registers[a] = if vb < vc { 0xFFFFFFFF } else if vb > vc { 1 } else { 0 };
            } else if opcode <= 0x30 { 
                let vb = f64::from_bits((registers[b] as u64) | ((registers[b+1] as u64) << 32));
                let vc = f64::from_bits((registers[c] as u64) | ((registers[c+1] as u64) << 32));
                registers[a] = if vb < vc { 0xFFFFFFFF } else if vb > vc { 1 } else { 0 };
            } else { 
                let vb = (registers[b] as u64 | ((registers[b+1] as u64) << 32)) as i64;
                let vc = (registers[c] as u64 | ((registers[c+1] as u64) << 32)) as i64;
                registers[a] = if vb < vc { 0xFFFFFFFF } else if vb > vc { 1 } else { 0 };
            }
            *pc += 1;
        }
        0x32..=0x37 => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF; let base_pc = *pc; *pc += 1; let off = code.insns[*pc] as i16 as i32;
            let cond = match opcode { 0x32 => registers[a] == registers[b], 0x33 => registers[a] != registers[b], 0x34 => (registers[a] as i32) < (registers[b] as i32), 0x35 => (registers[a] as i32) >= (registers[b] as i32), 0x36 => (registers[a] as i32) > (registers[b] as i32), 0x37 => (registers[a] as i32) <= (registers[b] as i32), _ => false };
            if cond { *pc = (base_pc as i32 + off) as usize; } else { *pc += 1; }
        }
        0x38..=0x3d => { 
            let a = (insn as usize >> 8) & 0xFF; let base_pc = *pc; *pc += 1; let off = code.insns[*pc] as i16 as i32; let val = registers[a] as i32;
            let cond = match opcode { 0x38 => val == 0, 0x39 => val != 0, 0x3a => val < 0, 0x3b => val >= 0, 0x3c => val > 0, 0x3d => val <= 0, _ => false };
            if cond { *pc = (base_pc as i32 + off) as usize; } else { *pc += 1; }
        }
        0x44..=0x51 => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; let next = code.insns[*pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
            let obj = registers[b]; let idx = registers[c] as usize;
            if opcode <= 0x4a { 
                registers[a] = vm.get_array_element(obj, idx)?;
                if opcode == 0x45 { registers[a+1] = vm.get_array_element(obj, idx+1)?; }
            } else { 
                vm.set_array_element(obj, idx, registers[a])?;
                if opcode == 0x4c { vm.set_array_element(obj, idx+1, registers[a+1])?; }
            }
            *pc += 1;
        }
        0x52..=0x5f => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF; *pc += 1; let f_idx = code.insns[*pc] as u32; let obj = registers[b];
            if opcode <= 0x58 { 
                registers[a] = vm.get_field(obj, f_idx)?;
                if opcode == 0x53 { registers[a+1] = vm.get_field(obj, f_idx+1)?; }
            } else { 
                vm.set_field(obj, f_idx, registers[a])?;
                if opcode == 0x5a { vm.set_field(obj, f_idx+1, registers[a+1])?; }
            }
            *pc += 1;
        }
        0x60..=0x6d => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; let f_idx = code.insns[*pc] as u32;
            let off = vm.dex.header.field_ids_off as usize + (f_idx as usize * 8);
            let f_id: crate::dex::FieldId = vm.dex.data.pread_with(off, LE)?;
            vm.initialize_class(f_id.class_idx as u32)?;
            
            if opcode <= 0x66 { 
                let f_name = vm.dex.get_string(f_id.name_idx).unwrap_or("".into());
                if f_name == "out" { registers[a] = vm.alloc(Object::Instance { class_idx: 0xFFFFFFFE, fields: std::collections::HashMap::new() }); }
                else { 
                    let val = vm.get_static_field(f_id.class_idx as u32, f_idx)?;
                    if opcode == 0x61 { 
                        registers[a] = (val & 0xFFFFFFFF) as u32;
                        registers[a+1] = (val >> 32) as u32;
                    } else {
                        registers[a] = val as u32;
                    }
                }
            } else {
                let val = if opcode == 0x68 { 
                    (registers[a] as u64) | ((registers[a+1] as u64) << 32)
                } else {
                    registers[a] as u64
                };
                vm.set_static_field(f_id.class_idx as u32, f_idx, val)?;
            }
            *pc += 1;
        }
        0x6e..=0x78 => { 
            let (_count, m_idx, args) = if opcode <= 0x72 { 
                let c = (insn as usize >> 12) & 0xF; *pc += 1; let mi = code.insns[*pc] as u32; *pc += 1; let r = code.insns[*pc];
                let mut a = Vec::new(); let ri = [(r & 0xF) as usize, ((r >> 4) & 0xF) as usize, ((r >> 8) & 0xF) as usize, ((r >> 12) & 0xF) as usize, (insn as usize >> 8) & 0xF];
                for i in 0..c { a.push(registers[ri[i]]); } (c, mi, a)
            } else { 
                let c = (insn as usize >> 8) & 0xFF; *pc += 1; let mi = code.insns[*pc] as u32; *pc += 1; let sr = code.insns[*pc] as usize;
                let mut a = Vec::new(); for i in 0..c { a.push(registers[sr + i]); } (c, mi, a)
            };

            let full_sig = vm.dex.get_method_full_signature(m_idx)?;
            let res = if let Some(n) = vm.native_methods.get(&full_sig) {
                n(vm, &args)?
            } else {
                let (c_def_idx, m_to_call) = if opcode == 0x6e || opcode == 0x72 || opcode == 0x74 || opcode == 0x78 {
                    if let Some(&oid) = args.first() {
                        vm.resolve_method(oid, m_idx)?
                    } else {
                        return Err(DexError::Parse("Invoke virtual/interface without this".into()));
                    }
                } else {
                    let off = vm.dex.header.method_ids_off as usize + (m_idx as usize * 8);
                    let m_id: crate::dex::MethodId = vm.dex.data.pread_with(off, LE)?;
                    if let Some(def_idx) = vm.dex.find_class_def(m_id.class_idx as u32)? {
                        (def_idx, m_idx)
                    } else {
                        let class_name = vm.dex.get_type(m_id.class_idx as u32)?;
                        let mut has_in_android = false;
                        if let Some(ref ad) = vm.android_dex {
                            if ad.find_class(&class_name)?.is_some() {
                                has_in_android = true;
                            }
                        }
                        if has_in_android {
                            (0xFFFFFFFE, m_idx)
                        } else {
                            (0xFFFFFFFF, m_idx)
                        }
                    }
                };

                if c_def_idx != 0xFFFFFFFF {
                    vm.execute_method(c_def_idx, m_to_call, &args)?
                } else {
                    None
                }
            };
            vm.last_result = res;
            *pc += 1;
        }
        0x7b..=0x8f => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF; let vb = registers[b] as i32;
            match opcode {
                0x7b => { registers[a] = vb.wrapping_neg() as u32; }
                0x7c => { registers[a] = (!vb) as u32; }
                0x7d => {
                    let val = (registers[b] as u64 | ((registers[b+1] as u64) << 32)) as i64;
                    let res = (-val) as u64;
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x7e => {
                    let val = registers[b] as u64 | ((registers[b+1] as u64) << 32);
                    let res = !val;
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x7f => {
                    let val = f32::from_bits(registers[b]);
                    registers[a] = (-val).to_bits();
                }
                0x80 => {
                    let val = f64::from_bits(registers[b] as u64 | ((registers[b+1] as u64) << 32));
                    let res = (-val).to_bits();
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x81 => {
                    let val = vb as i64 as u64;
                    registers[a] = (val & 0xFFFFFFFF) as u32;
                    registers[a+1] = (val >> 32) as u32;
                }
                0x82 => {
                    let val = vb as f32;
                    registers[a] = val.to_bits();
                }
                0x83 => {
                    let val = vb as f64;
                    let res = val.to_bits();
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x84 => {
                    registers[a] = registers[b];
                }
                0x85 => {
                    let val = (registers[b] as u64 | ((registers[b+1] as u64) << 32)) as i64;
                    registers[a] = (val as f32).to_bits();
                }
                0x86 => {
                    let val = (registers[b] as u64 | ((registers[b+1] as u64) << 32)) as i64;
                    let res = (val as f64).to_bits();
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x87 => {
                    let val = f32::from_bits(registers[b]);
                    registers[a] = (val as i32) as u32;
                }
                0x88 => {
                    let val = f32::from_bits(registers[b]);
                    let res = (val as i64) as u64;
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x89 => {
                    let val = f32::from_bits(registers[b]);
                    let res = (val as f64).to_bits();
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x8a => {
                    let val = f64::from_bits(registers[b] as u64 | ((registers[b+1] as u64) << 32));
                    registers[a] = (val as i32) as u32;
                }
                0x8b => {
                    let val = f64::from_bits(registers[b] as u64 | ((registers[b+1] as u64) << 32));
                    let res = (val as i64) as u64;
                    registers[a] = (res & 0xFFFFFFFF) as u32;
                    registers[a+1] = (res >> 32) as u32;
                }
                0x8c => {
                    let val = f64::from_bits(registers[b] as u64 | ((registers[b+1] as u64) << 32));
                    registers[a] = (val as f32).to_bits();
                }
                0x8d => { registers[a] = (vb as i8) as i32 as u32; }
                0x8e => { registers[a] = (vb as u16) as u32; }
                0x8f => { registers[a] = (vb as i16) as i32 as u32; }
                _ => {}
            }
            *pc += 1;
        }
        0x90..=0xaf => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; let next = code.insns[*pc]; let b = next as usize & 0xFF; let c = next as usize >> 8;
            if opcode >= 0x9b && opcode <= 0xa5 { 
                let vbl = (registers[b] as u64 | ((registers[b+1] as u64) << 32)) as i64;
                let vcl = (registers[c] as u64 | ((registers[c+1] as u64) << 32)) as i64;
                let shift_amt = (registers[c] as u32 & 0x3f) as u64;
                let res = match opcode {
                    0x9b => vbl.wrapping_add(vcl), 0x9c => vbl.wrapping_sub(vcl), 0x9d => vbl.wrapping_mul(vcl),
                    0x9e => vbl.checked_div(vcl).unwrap_or(0), 0x9f => vbl.checked_rem(vcl).unwrap_or(0),
                    0xa0 => vbl & vcl, 0xa1 => vbl | vcl, 0xa2 => vbl ^ vcl,
                    0xa3 => vbl << shift_amt, 0xa4 => vbl >> shift_amt, 0xa5 => ((vbl as u64) >> shift_amt) as i64,
                    _ => 0
                } as u64;
                registers[a] = (res & 0xFFFFFFFF) as u32; registers[a+1] = (res >> 32) as u32;
            } else if opcode >= 0xab && opcode <= 0xae {
                let vbd = f64::from_bits((registers[b] as u64) | ((registers[b+1] as u64) << 32));
                let vcd = f64::from_bits((registers[c] as u64) | ((registers[c+1] as u64) << 32));
                let res = match opcode { 0xab => vbd + vcd, 0xac => vbd - vcd, 0xad => vbd * vcd, 0xae => vbd / vcd, _ => 0.0 }.to_bits();
                registers[a] = (res & 0xFFFFFFFF) as u32; registers[a+1] = (res >> 32) as u32;
            } else if opcode == 0xaf {
                let vbd = f64::from_bits((registers[b] as u64) | ((registers[b+1] as u64) << 32));
                let vcd = f64::from_bits((registers[c] as u64) | ((registers[c+1] as u64) << 32));
                let res = (vbd % vcd).to_bits();
                registers[a] = (res & 0xFFFFFFFF) as u32; registers[a+1] = (res >> 32) as u32;
            } else {
                let vb = registers[b] as i32; let vc = registers[c] as i32;
                registers[a] = match opcode { 
                    0x90 => vb.wrapping_add(vc), 0x91 => vb.wrapping_sub(vc), 0x92 => vb.wrapping_mul(vc), 0x93 => vb.checked_div(vc).unwrap_or(0), 0x94 => vb.checked_rem(vc).unwrap_or(0), 
                    0x95 => vb & vc, 0x96 => vb | vc, 0x97 => vb ^ vc, 0x98 => vb << (vc & 0x1f), 0x99 => vb >> (vc & 0x1f), 0x9a => (vb as u32 >> (vc & 0x1f)) as i32,
                    0xa6 => (f32::from_bits(registers[b] as u32) + f32::from_bits(registers[c] as u32)).to_bits() as i32,
                    0xa7 => (f32::from_bits(registers[b] as u32) - f32::from_bits(registers[c] as u32)).to_bits() as i32,
                    0xa8 => (f32::from_bits(registers[b] as u32) * f32::from_bits(registers[c] as u32)).to_bits() as i32,
                    0xa9 => (f32::from_bits(registers[b] as u32) / f32::from_bits(registers[c] as u32)).to_bits() as i32,
                    0xaa => (f32::from_bits(registers[b] as u32) % f32::from_bits(registers[c] as u32)).to_bits() as i32,
                    _ => 0 
                } as u32;
            }
            *pc += 1;
        }
        0xb0..=0xcf => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF;
            if opcode >= 0xbb && opcode <= 0xc5 { 
                let val_a = (registers[a] as u64 | ((registers[a+1] as u64) << 32)) as i64;
                let val_b = (registers[b] as u64 | ((registers[b+1] as u64) << 32)) as i64;
                let shift_amt = (registers[b] as u32 & 0x3f) as u64;
                let res = match opcode {
                    0xbb => val_a.wrapping_add(val_b), 0xbc => val_a.wrapping_sub(val_b), 0xbd => val_a.wrapping_mul(val_b),
                    0xbe => val_a.checked_div(val_b).unwrap_or(0), 0xbf => val_a.checked_rem(val_b).unwrap_or(0),
                    0xc0 => val_a & val_b, 0xc1 => val_a | val_b, 0xc2 => val_a ^ val_b,
                    0xc3 => val_a << shift_amt, 0xc4 => val_a >> shift_amt, 0xc5 => ((val_a as u64) >> shift_amt) as i64,
                    _ => 0
                } as u64;
                registers[a] = (res & 0xFFFFFFFF) as u32; registers[a+1] = (res >> 32) as u32;
            } else if opcode >= 0xcb && opcode <= 0xce {
                let vad = f64::from_bits((registers[a] as u64) | ((registers[a+1] as u64) << 32));
                let vbd = f64::from_bits((registers[b] as u64) | ((registers[b+1] as u64) << 32));
                let res = match opcode { 0xcb => vad + vbd, 0xcc => vad - vbd, 0xcd => vad * vbd, 0xce => vad / vbd, _ => 0.0 }.to_bits();
                registers[a] = (res & 0xFFFFFFFF) as u32; registers[a+1] = (res >> 32) as u32;
            } else if opcode == 0xcf {
                let vad = f64::from_bits((registers[a] as u64) | ((registers[a+1] as u64) << 32));
                let vbd = f64::from_bits((registers[b] as u64) | ((registers[b+1] as u64) << 32));
                let res = (vad % vbd).to_bits();
                registers[a] = (res & 0xFFFFFFFF) as u32; registers[a+1] = (res >> 32) as u32;
            } else {
                let va = registers[a] as i32; let vb = registers[b] as i32;
                registers[a] = match opcode { 
                    0xb0 => va.wrapping_add(vb), 0xb1 => va.wrapping_sub(vb), 0xb2 => va.wrapping_mul(vb), 0xb3 => va.checked_div(vb).unwrap_or(0), 0xb4 => va.checked_rem(vb).unwrap_or(0), 0xb5 => va & vb, 0xb6 => va | vb, 0xb7 => va ^ vb, 0xb8 => va << (vb & 0x1f), 0xb9 => va >> (vb & 0x1f), 0xba => (va as u32 >> (vb & 0x1f)) as i32,
                    0xc6 => (f32::from_bits(registers[a] as u32) + f32::from_bits(registers[b] as u32)).to_bits() as i32,
                    0xc7 => (f32::from_bits(registers[a] as u32) - f32::from_bits(registers[b] as u32)).to_bits() as i32,
                    0xc8 => (f32::from_bits(registers[a] as u32) * f32::from_bits(registers[b] as u32)).to_bits() as i32,
                    0xc9 => (f32::from_bits(registers[a] as u32) / f32::from_bits(registers[b] as u32)).to_bits() as i32,
                    0xca => (f32::from_bits(registers[a] as u32) % f32::from_bits(registers[b] as u32)).to_bits() as i32,
                    _ => 0 
                } as u32;
            }
            *pc += 1;
        }
        0xd0..=0xd7 => { 
            let a = (insn as usize >> 8) & 0xF; let b = (insn as usize >> 12) & 0xF; *pc += 1; let lit = code.insns[*pc] as i16 as i32;
            let vb = registers[b] as i32;
            registers[a] = match opcode {
                0xd0 => vb.wrapping_add(lit), 0xd1 => lit.wrapping_sub(vb), 0xd2 => vb.wrapping_mul(lit),
                0xd3 => vb.checked_div(lit).unwrap_or(0), 0xd4 => vb.checked_rem(lit).unwrap_or(0),
                0xd5 => vb & lit, 0xd6 => vb | lit, 0xd7 => vb ^ lit,
                _ => 0
            } as u32;
            *pc += 1;
        }
        0xd8..=0xe2 => { 
            let a = (insn as usize >> 8) & 0xFF; *pc += 1; let next = code.insns[*pc]; let b = next as usize & 0xFF; let lit = (next >> 8) as i8 as i32;
            let vb = registers[b] as i32;
            registers[a] = match opcode {
                0xd8 => vb.wrapping_add(lit), 0xd9 => lit.wrapping_sub(vb), 0xda => vb.wrapping_mul(lit),
                0xdb => vb.checked_div(lit).unwrap_or(0), 0xdc => vb.checked_rem(lit).unwrap_or(0),
                0xdd => vb & lit, 0xde => vb | lit, 0xdf => vb ^ lit,
                0xe0 => vb << (lit & 0x1f), 0xe1 => vb >> (lit & 0x1f), 0xe2 => (vb as u32 >> (lit & 0x1f)) as i32,
                _ => 0
            } as u32;
            *pc += 1;
        }
        0xfc | 0xfd => {
            *pc += 4;
            return Err(DexError::Parse(format!("Unsupported API 26+ polymorphic invoke: 0x{:02x}", opcode)));
        }
        0xfe | 0xff => {
            *pc += 3;
            return Err(DexError::Parse(format!("Unsupported API 26+ custom callsite invoke: 0x{:02x}", opcode)));
        }
        0x27 => { let a = (insn as usize >> 8) & 0xFF; return Err(DexError::Exception(registers[a])); }
        _ => return Err(DexError::Parse(format!("Unknown or deprecated opcode: 0x{:02x}", opcode))),
    }
    Ok(())
}
