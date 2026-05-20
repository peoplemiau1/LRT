use scroll::{Pread, LE};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AxmlError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Scroll error: {0}")]
    Scroll(#[from] scroll::Error),
}

pub type AxmlResult<T> = Result<T, AxmlError>;

pub struct AxmlAttribute {
    pub name: String,
    pub id: u32,
    pub value: String,
}

#[allow(dead_code)]
pub struct AxmlElement {
    pub name: String,
    pub attributes: Vec<AxmlAttribute>,
    pub children: Vec<AxmlElement>,
}

#[derive(Debug, Pread)]
struct ChunkHeader {
    typ: u16,
    _header_size: u16,
    size: u32,
}

pub fn parse_manifest(data: &[u8]) -> AxmlResult<AxmlElement> {
    let mut offset = 0;
    let _header: ChunkHeader = data.pread_with(offset, LE)?;
    offset += 8;

    let mut string_pool = Vec::new();
    let mut resource_map = Vec::new();
    let mut stack: Vec<AxmlElement> = Vec::new();
    let mut root = None;

    while offset < data.len() {
        let chunk: ChunkHeader = data.pread_with(offset, LE)?;
        let chunk_start = offset;
        
        match chunk.typ {
            0x0001 => { 
                string_pool = parse_string_pool(&data[offset..offset + chunk.size as usize])?;
            }
            0x0180 => { 
                let count = (chunk.size as usize - 8) / 4;
                for i in 0..count {
                    resource_map.push(data.pread_with::<u32>(offset + 8 + (i * 4), LE)?);
                }
            }
            0x0102 => { 
                let mut off = offset + 8 + 8; 
                let _ns_idx: u32 = data.pread_with(off, LE)?; off += 4;
                let name_idx: u32 = data.pread_with(off, LE)?; off += 4;
                let _attr_start: u16 = data.pread_with(off, LE)?; off += 2;
                let _attr_size: u16 = data.pread_with(off, LE)?; off += 2;
                let attr_count: u16 = data.pread_with(off, LE)?;

                let name = string_pool.get(name_idx as usize).cloned().unwrap_or_default();
                let mut attributes = Vec::new();

                off = offset + 8 + 20; 
                for _ in 0..attr_count {
                    let _attr_ns_idx: u32 = data.pread_with(off, LE)?; off += 4;
                    let attr_name_idx: u32 = data.pread_with(off, LE)?; off += 4;
                    let _attr_raw_val_idx: u32 = data.pread_with(off, LE)?; off += 4;
                    let _attr_type: u16 = data.pread_with(off, LE)?; off += 2;
                    let _attr_data: u32 = data.pread_with(off + 2, LE)?; off += 8;

                    let attr_name = string_pool.get(attr_name_idx as usize).cloned().unwrap_or_default();
                    let attr_id = resource_map.get(attr_name_idx as usize).cloned().unwrap_or(0);
                    
                    let attr_raw_val = string_pool.get(_attr_raw_val_idx as i32 as usize).cloned().unwrap_or_default();
                    let mut attr_value = if _attr_raw_val_idx != 0xFFFFFFFF {
                        attr_raw_val
                    } else {
                        
                        match _attr_type >> 8 { 
                             0x03 => string_pool.get(_attr_data as usize).cloned().unwrap_or_default(),
                             _ => format!("{}", _attr_data)
                        }
                    };

                    
                    
                    if _attr_type == 0x0308 || (_attr_type >> 8 == 0x03) {
                         if let Some(s) = string_pool.get(_attr_data as usize) {
                             attr_value = s.clone();
                         }
                    }

                    attributes.push(AxmlAttribute {
                        name: attr_name,
                        id: attr_id,
                        value: attr_value,
                    });
                }

                stack.push(AxmlElement {
                    name,
                    attributes,
                    children: vec![],
                });
            }
            0x0103 => { 
                if let Some(finished) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(finished);
                    } else {
                        root = Some(finished);
                    }
                }
            }
            _ => {}
        }
        offset = chunk_start + chunk.size as usize;
    }

    root.ok_or_else(|| AxmlError::Parse("No root element found".into()))
}

fn parse_string_pool(data: &[u8]) -> AxmlResult<Vec<String>> {
    let mut offset = 8; 
    let string_count: u32 = data.pread_with(offset, LE)?; offset += 4;
    let _style_count: u32 = data.pread_with(offset, LE)?; offset += 4;
    let flags: u32 = data.pread_with(offset, LE)?; offset += 4;
    let string_start: u32 = data.pread_with(offset, LE)?; offset += 4;
    let _styles_start: u32 = data.pread_with(offset, LE)?;

    let is_utf8 = (flags & (1 << 8)) != 0;
    let mut strings = Vec::with_capacity(string_count as usize);

    for i in 0..string_count {
        let off = 28 + (i as usize * 4);
        let mut str_off = string_start as usize + data.pread_with::<u32>(off, LE)? as usize;
        
        if is_utf8 {
            
            let (_char_len, b1) = read_len_utf8(data, str_off); str_off += b1;
            let (byte_len, b2) = read_len_utf8(data, str_off); str_off += b2;
            let s = String::from_utf8_lossy(&data[str_off..str_off + byte_len as usize]).to_string();
            strings.push(s);
        } else {
            
            let (char_len, b) = read_len_utf16(data, str_off); str_off += b;
            let utf16_data: Vec<u16> = (0..char_len)
                .map(|j| data.pread_with::<u16>(str_off + (j as usize * 2), LE).unwrap())
                .collect();
            strings.push(String::from_utf16_lossy(&utf16_data));
        }
    }

    Ok(strings)
}

fn read_len_utf8(data: &[u8], off: usize) -> (u32, usize) {
    let b: u8 = data.pread(off).unwrap();
    if b & 0x80 != 0 {
        (((b & 0x7f) as u32) << 8 | data.pread::<u8>(off + 1).unwrap() as u32, 2)
    } else {
        (b as u32, 1)
    }
}

fn read_len_utf16(data: &[u8], off: usize) -> (u32, usize) {
    let b: u16 = data.pread_with(off, LE).unwrap();
    if b & 0x8000 != 0 {
        (((b & 0x7fff) as u32) << 16 | data.pread_with::<u16>(off + 2, LE).unwrap() as u32, 4)
    } else {
        (b as u32, 2)
    }
}
