use scroll::{Pread, LE};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DexError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Scroll error: {0}")]
    Scroll(#[from] scroll::Error),
    #[error("Exception thrown: {0}")]
    Exception(u32),
    #[error("Return")]
    Return(Option<u32>),
}

pub type DexResult<T> = Result<T, DexError>;

#[allow(dead_code)]
#[derive(Debug, Pread, Clone, Copy)]
pub struct Header {
    pub magic: [u8; 8],
    pub checksum: u32,
    pub signature: [u8; 20],
    pub file_size: u32,
    pub header_size: u32,
    pub endian_tag: u32,
    pub link_size: u32,
    pub link_off: u32,
    pub map_off: u32,
    pub string_ids_size: u32,
    pub string_ids_off: u32,
    pub type_ids_size: u32,
    pub type_ids_off: u32,
    pub proto_ids_size: u32,
    pub proto_ids_off: u32,
    pub field_ids_size: u32,
    pub field_ids_off: u32,
    pub method_ids_size: u32,
    pub method_ids_off: u32,
    pub class_defs_size: u32,
    pub class_defs_off: u32,
    pub data_size: u32,
    pub data_off: u32,
}

#[derive(Debug, Pread)]
pub struct ProtoId { pub shorty_idx: u32, pub return_type_idx: u32, pub parameters_off: u32 }
#[derive(Debug, Pread)]
pub struct StringId { pub offset: u32 }
#[derive(Debug, Pread)]
pub struct TypeId { pub descriptor_idx: u32 }
#[derive(Debug, Pread)]
pub struct FieldId { pub class_idx: u16, pub type_idx: u16, pub name_idx: u32 }
#[derive(Debug, Pread)]
pub struct MethodId { pub class_idx: u16, pub proto_idx: u16, pub name_idx: u32 }
#[derive(Debug, Pread)]
pub struct ClassDef { pub class_idx: u32, pub access_flags: u32, pub superclass_idx: u32, pub interfaces_off: u32, pub source_file_idx: u32, pub annotations_off: u32, pub class_data_off: u32, pub static_values_off: u32 }

#[derive(Debug)]
pub struct EncodedField { pub field_idx: u32, pub access_flags: u32 }

#[derive(Debug, Clone)]
pub struct EncodedMethod { pub method_idx: u32, pub access_flags: u32, pub code_off: u32 }

#[derive(Debug, Clone)]
pub enum EncodedValue {
    Byte(i8),
    Short(i16),
    Char(u16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(u32),
    Type(u32),
    Field(u32),
    Method(u32),
    Enum(u32),
    Array(Vec<EncodedValue>),
    Annotation,
    Null,
    Boolean(bool),
}

#[derive(Debug)]
pub struct ClassData { 
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>, 
    pub virtual_methods: Vec<EncodedMethod> 
}

#[derive(Debug, Pread)]
pub struct CodeItemHeader { pub registers_size: u16, pub ins_size: u16, pub outs_size: u16, pub tries_size: u16, pub debug_info_off: u32, pub insns_size: u32 }

#[derive(Debug)]
pub struct TryItem { pub start_addr: u32, pub insn_count: u16, pub handler_off: u16 }

#[derive(Debug)]
pub struct CatchHandler { pub handlers: Vec<(u32, u32)>, pub catch_all: Option<u32> }

#[derive(Debug)]
pub struct CodeItem { pub header: CodeItemHeader, pub insns: Vec<u16>, pub tries: Vec<TryItem>, pub handlers: HashMap<u16, CatchHandler> }

#[derive(Clone, Copy)]
pub struct Dex<'a> { pub header: Header, pub data: &'a [u8] }

impl<'a> Dex<'a> {
    pub fn new(data: &'a [u8]) -> DexResult<Self> {
        let header: Header = data.pread_with(0, LE)?;
        Ok(Dex { header, data })
    }

    pub fn get_string(&self, idx: u32) -> DexResult<String> {
        let off = self.header.string_ids_off as usize + (idx as usize * 4);
        let string_id: StringId = self.data.pread_with(off, LE)?;
        let mut string_off = string_id.offset as usize;
        let (len, bytes_read) = self.read_uleb128(string_off)?;
        string_off += bytes_read;
        let s = std::str::from_utf8(&self.data[string_off..string_off + len as usize]).map_err(|_| DexError::Parse("Invalid UTF-8".into()))?;
        Ok(s.to_string())
    }

    pub fn get_type(&self, idx: u32) -> DexResult<String> {
        let off = self.header.type_ids_off as usize + (idx as usize * 4);
        let type_id: TypeId = self.data.pread_with(off, LE)?;
        self.get_string(type_id.descriptor_idx)
    }

    pub fn get_method_name(&self, idx: u32) -> DexResult<String> {
        let off = self.header.method_ids_off as usize + (idx as usize * 8);
        let method_id: MethodId = self.data.pread_with(off, LE)?;
        self.get_string(method_id.name_idx)
    }

    pub fn get_class_name(&self, idx: u32) -> DexResult<String> {
        self.get_type(self.get_class_type_idx(idx)?)
    }

    pub fn get_class_type_idx(&self, idx: u32) -> DexResult<u32> {
        let off = self.header.class_defs_off as usize + (idx as usize * 32);
        let class_def: ClassDef = self.data.pread_with(off, LE)?;
        Ok(class_def.class_idx)
    }

    pub fn find_class(&self, name: &str) -> DexResult<Option<u32>> {
        for i in 0..self.header.class_defs_size {
            if self.get_class_name(i)? == name { return Ok(Some(i)); }
        }
        Ok(None)
    }

    pub fn get_field_name(&self, idx: u32) -> DexResult<String> {
        let off = self.header.field_ids_off as usize + (idx as usize * 8);
        let field_id: FieldId = self.data.pread_with(off, LE)?;
        self.get_string(field_id.name_idx)
    }

    pub fn get_field_class(&self, idx: u32) -> DexResult<String> {
        let off = self.header.field_ids_off as usize + (idx as usize * 8);
        let field_id: FieldId = self.data.pread_with(off, LE)?;
        self.get_type(field_id.class_idx as u32)
    }

    pub fn find_class_def(&self, type_idx: u32) -> DexResult<Option<u32>> {
        for i in 0..self.header.class_defs_size {
            let off = self.header.class_defs_off as usize + (i as usize * 32);
            let class_def: ClassDef = self.data.pread_with(off, LE)?;
            if class_def.class_idx == type_idx { return Ok(Some(i)); }
        }
        Ok(None)
    }

    pub fn find_method_in_class(&self, class_idx: u32, name: &str) -> DexResult<Option<u32>> {
        let class_data = self.get_class_data(class_idx)?;
        for m in class_data.direct_methods.iter().chain(class_data.virtual_methods.iter()) {
            if self.get_method_name(m.method_idx)? == name { return Ok(Some(m.method_idx)); }
        }
        Ok(None)
    }

    pub fn get_proto_signature(&self, proto_idx: u32) -> DexResult<String> {
        let off = self.header.proto_ids_off as usize + (proto_idx as usize * 12);
        let proto: ProtoId = self.data.pread_with(off, LE)?;
        let mut sig = "(".to_string();
        if proto.parameters_off != 0 {
            let mut p_off = proto.parameters_off as usize;
            let size: u32 = self.data.pread_with(p_off, LE)?; p_off += 4;
            for _ in 0..size {
                let type_idx: u16 = self.data.pread_with(p_off, LE)?; p_off += 2;
                sig.push_str(&self.get_type(type_idx as u32)?);
            }
        }
        sig.push(')');
        sig.push_str(&self.get_type(proto.return_type_idx)?);
        Ok(sig)
    }

    pub fn get_method_full_signature(&self, meth_idx: u32) -> DexResult<String> {
        let off = self.header.method_ids_off as usize + (meth_idx as usize * 8);
        let m_id: MethodId = self.data.pread_with(off, LE)?;
        let class_name = self.get_type(m_id.class_idx as u32)?;
        let meth_name = self.get_string(m_id.name_idx)?;
        let proto_sig = self.get_proto_signature(m_id.proto_idx as u32)?;
        Ok(format!("{}->{}{}", class_name, meth_name, proto_sig))
    }

    pub fn get_class_data(&self, class_idx: u32) -> DexResult<ClassData> {
        let off = self.header.class_defs_off as usize + (class_idx as usize * 32);
        let class_def: ClassDef = self.data.pread_with(off, LE)?;
        self.parse_class_data(class_def.class_data_off)
    }

    pub fn get_class_interfaces(&self, class_idx: u32) -> DexResult<Vec<u32>> {
        let off = self.header.class_defs_off as usize + (class_idx as usize * 32);
        let class_def: ClassDef = self.data.pread_with(off, LE)?;
        if class_def.interfaces_off == 0 { return Ok(vec![]); }
        let mut i_off = class_def.interfaces_off as usize;
        let size: u32 = self.data.pread_with(i_off, LE)?; i_off += 4;
        let mut interfaces = Vec::new();
        for _ in 0..size {
            let type_idx: u16 = self.data.pread_with(i_off, LE)?; i_off += 2;
            interfaces.push(type_idx as u32);
        }
        Ok(interfaces)
    }

    pub fn get_static_values(&self, class_idx: u32) -> DexResult<Vec<EncodedValue>> {
        let off = self.header.class_defs_off as usize + (class_idx as usize * 32);
        let class_def: ClassDef = self.data.pread_with(off, LE)?;
        if class_def.static_values_off == 0 { return Ok(vec![]); }
        
        let mut ptr = class_def.static_values_off as usize;
        let (size, b) = self.read_uleb128(ptr)?; ptr += b;
        let mut values = Vec::new();
        for _ in 0..size {
            let (val, b) = self.read_encoded_value(ptr)?;
            values.push(val);
            ptr += b;
        }
        Ok(values)
    }

    fn read_encoded_value(&self, mut ptr: usize) -> DexResult<(EncodedValue, usize)> {
        let start_ptr = ptr;
        let header: u8 = self.data.pread(ptr)?; ptr += 1;
        let value_type = header & 0x1F;
        let value_arg = (header >> 5) as usize;

        let val = match value_type {
            0x00 => EncodedValue::Byte(self.read_int_bits(ptr, value_arg, true)? as i8),
            0x02 => EncodedValue::Short(self.read_int_bits(ptr, value_arg, true)? as i16),
            0x03 => EncodedValue::Char(self.read_int_bits(ptr, value_arg, false)? as u16),
            0x04 => EncodedValue::Int(self.read_int_bits(ptr, value_arg, true)? as i32),
            0x06 => EncodedValue::Long(self.read_int_bits(ptr, value_arg, true)? as i64),
            0x10 => EncodedValue::Float(f32::from_bits((self.read_int_bits(ptr, value_arg, false)? as u32) << ((3 - value_arg) * 8))),
            0x11 => EncodedValue::Double(f64::from_bits((self.read_int_bits(ptr, value_arg, false)? as u64) << ((7 - value_arg) * 8))),
            0x17 => EncodedValue::String(self.read_int_bits(ptr, value_arg, false)? as u32),
            0x18 => EncodedValue::Type(self.read_int_bits(ptr, value_arg, false)? as u32),
            0x19 => EncodedValue::Field(self.read_int_bits(ptr, value_arg, false)? as u32),
            0x1a => EncodedValue::Method(self.read_int_bits(ptr, value_arg, false)? as u32),
            0x1b => EncodedValue::Enum(self.read_int_bits(ptr, value_arg, false)? as u32),
            0x1c => {
                let (size, b) = self.read_uleb128(ptr)?; ptr += b;
                let mut arr = Vec::new();
                for _ in 0..size {
                    let (v, b) = self.read_encoded_value(ptr)?;
                    arr.push(v);
                    ptr += b;
                }
                return Ok((EncodedValue::Array(arr), ptr - start_ptr));
            }
            0x1d => EncodedValue::Annotation,
            0x1e => EncodedValue::Null,
            0x1f => EncodedValue::Boolean(value_arg != 0),
            _ => return Err(DexError::Parse(format!("Unknown encoded value type: 0x{:02x}", value_type))),
        };

        if value_type <= 0x1b && value_type != 0x1c && value_type != 0x1d && value_type != 0x1e && value_type != 0x1f {
             ptr += value_arg + 1;
        }

        Ok((val, ptr - start_ptr))
    }

    fn read_int_bits(&self, ptr: usize, arg: usize, sign_extend: bool) -> DexResult<u64> {
        let mut val = 0u64;
        for i in 0..=arg {
            let b: u8 = self.data.pread(ptr + i)?;
            val |= (b as u64) << (i * 8);
        }
        if sign_extend {
            let shift = (7 - arg) * 8;
            val = ((val as i64) << shift >> shift) as u64;
        }
        Ok(val)
    }

    pub fn is_instance_of(&self, class_idx: u32, target_type_idx: u32) -> DexResult<bool> {
        let mut current_idx = class_idx;
        loop {
            let off = self.header.class_defs_off as usize + (current_idx as usize * 32);
            let class_def: ClassDef = self.data.pread_with(off, LE)?;
            if class_def.class_idx == target_type_idx { return Ok(true); }
            
            // Check interfaces
            if class_def.interfaces_off != 0 {
                let mut i_off = class_def.interfaces_off as usize;
                let size: u32 = self.data.pread_with(i_off, LE)?; i_off += 4;
                for _ in 0..size {
                    let itype_idx: u16 = self.data.pread_with(i_off, LE)?; i_off += 2;
                    if itype_idx as u32 == target_type_idx { return Ok(true); }
                }
            }

            if class_def.superclass_idx == 0xFFFFFFFF { break; }
            if let Some(s_idx) = self.find_class_def(class_def.superclass_idx)? {
                current_idx = s_idx;
            } else { break; }
        }
        Ok(false)
    }

    fn parse_class_data(&self, class_data_off: u32) -> DexResult<ClassData> {
        if class_data_off == 0 { return Ok(ClassData { static_fields: vec![], instance_fields: vec![], direct_methods: vec![], virtual_methods: vec![] }); }
        let mut offset = class_data_off as usize;
        let (sf_size, b) = self.read_uleb128(offset)?; offset += b;
        let (if_size, b) = self.read_uleb128(offset)?; offset += b;
        let (dm_size, b) = self.read_uleb128(offset)?; offset += b;
        let (vm_size, b) = self.read_uleb128(offset)?; offset += b;

        let mut static_fields = Vec::new();
        let mut prev = 0;
        for _ in 0..sf_size {
            let (d, b) = self.read_uleb128(offset)?; offset += b;
            let (f, b) = self.read_uleb128(offset)?; offset += b;
            prev += d;
            static_fields.push(EncodedField { field_idx: prev, access_flags: f });
        }

        let mut instance_fields = Vec::new();
        prev = 0;
        for _ in 0..if_size {
            let (d, b) = self.read_uleb128(offset)?; offset += b;
            let (f, b) = self.read_uleb128(offset)?; offset += b;
            prev += d;
            instance_fields.push(EncodedField { field_idx: prev, access_flags: f });
        }

        let mut dm = Vec::new(); prev = 0;
        for _ in 0..dm_size { let (d, b) = self.read_uleb128(offset)?; offset += b; let (f, b) = self.read_uleb128(offset)?; offset += b; let (c, b) = self.read_uleb128(offset)?; offset += b; prev += d; dm.push(EncodedMethod { method_idx: prev, access_flags: f, code_off: c }); }
        let mut vm = Vec::new(); prev = 0;
        for _ in 0..vm_size { let (d, b) = self.read_uleb128(offset)?; offset += b; let (f, b) = self.read_uleb128(offset)?; offset += b; let (c, b) = self.read_uleb128(offset)?; offset += b; prev += d; vm.push(EncodedMethod { method_idx: prev, access_flags: f, code_off: c }); }
        Ok(ClassData { static_fields, instance_fields, direct_methods: dm, virtual_methods: vm })
    }

    pub fn get_code_item(&self, code_off: u32) -> DexResult<CodeItem> {
        if code_off == 0 { return Err(DexError::Parse("No code".into())); }
        let header: CodeItemHeader = self.data.pread_with(code_off as usize, LE)?;
        let mut off = code_off as usize + 16;
        let mut insns = Vec::new();
        for _ in 0..header.insns_size { insns.push(self.data.pread_with(off, LE)?); off += 2; }
        let mut tries = Vec::new();
        let mut handlers = HashMap::new();
        if header.tries_size > 0 {
            if (off % 4) != 0 { off += 2; }
            let _tries_start = off;
            for _ in 0..header.tries_size {
                let start: u32 = self.data.pread_with(off, LE)?;
                let count: u16 = self.data.pread_with(off + 4, LE)?;
                let h_off: u16 = self.data.pread_with(off + 6, LE)?;
                tries.push(TryItem { start_addr: start, insn_count: count, handler_off: h_off });
                off += 8;
            }
            let h_base = off;
            let (h_count, b) = self.read_uleb128(h_base)?;
            let mut h_ptr = h_base + b;
            for _ in 0..h_count {
                let current_h_off = (h_ptr - h_base) as u16;
                let (size_raw, b) = self.read_sleb128(h_ptr)?; h_ptr += b;
                let mut type_addrs = Vec::new();
                for _ in 0..size_raw.unsigned_abs() {
                    let (t_idx, b) = self.read_uleb128(h_ptr)?; h_ptr += b;
                    let (addr, b) = self.read_uleb128(h_ptr)?; h_ptr += b;
                    type_addrs.push((t_idx, addr));
                }
                let catch_all = if size_raw <= 0 { let (a, b) = self.read_uleb128(h_ptr)?; h_ptr += b; Some(a) } else { None };
                handlers.insert(current_h_off, CatchHandler { handlers: type_addrs, catch_all });
            }
        }
        Ok(CodeItem { header, insns, tries, handlers })
    }

    fn read_uleb128(&self, mut offset: usize) -> DexResult<(u32, usize)> {
        let mut res = 0u32; let mut shift = 0; let mut read = 0;
        loop {
            let b: u8 = self.data.pread(offset)?; offset += 1; read += 1;
            res |= ((b & 0x7f) as u32) << shift;
            if (b & 0x80) == 0 { break; }
            shift += 7;
        }
        Ok((res, read))
    }

    fn read_sleb128(&self, mut offset: usize) -> DexResult<(i32, usize)> {
        let mut res = 0i32; let mut shift = 0; let mut read = 0;
        loop {
            let b: u8 = self.data.pread(offset)?; offset += 1; read += 1;
            res |= ((b & 0x7f) as i32) << shift; shift += 7;
            if (b & 0x80) == 0 {
                if shift < 32 && (b & 0x40) != 0 { res |= -(1 << shift); }
                break;
            }
        }
        Ok((res, read))
    }
}
