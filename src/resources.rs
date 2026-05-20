use scroll::{Pread, LE};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResourceError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Scroll error: {0}")]
    Scroll(#[from] scroll::Error),
}

pub type ResourceResult<T> = Result<T, ResourceError>;

#[derive(Debug, Pread)]
struct ChunkHeader {
    typ: u16,
    _header_size: u16,
    size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ResConfig {
    pub mcc: u16,
    pub mnc: u16,
    pub language: [u8; 2],
    pub country: [u8; 2],
    pub orientation: u8,
    pub touchscreen: u8,
    pub density: u16,
    pub keyboard: u8,
    pub navigation: u8,
    pub inputFlags: u8,
    pub screenWidth: u16,
    pub screenHeight: u16,
    pub sdkVersion: u16,
    pub minorVersion: u16,
    pub screenLayout: u8,
    pub uiMode: u8,
    pub smallestScreenWidthDp: u16,
    pub screenWidthDp: u16,
    pub screenHeightDp: u16,
}

pub struct ResourceTable {
    pub strings: Vec<String>,
    pub resource_map: HashMap<u32, Vec<(ResConfig, ResourceValue)>>,
}

impl ResourceTable {
    pub fn get(&self, id: u32, config: &ResConfig) -> Option<ResourceValue> {
        let entries = self.resource_map.get(&id)?;
        let mut best_match = None;
        let mut best_score = -1;

        for (c, val) in entries {
            
            if c.sdkVersion > config.sdkVersion && c.sdkVersion != 0 { continue; }
            if c.language != [0, 0] && c.language != config.language { continue; }
            if c.country != [0, 0] && c.country != config.country { continue; }
            
            
            let mut score = 0;
            if c.language == config.language && c.language != [0, 0] {
                score += 1000;
                if c.country == config.country && c.country != [0, 0] {
                    score += 500;
                }
            }
            
            if c.sdkVersion == config.sdkVersion && c.sdkVersion != 0 {
                score += 100;
            } else if c.sdkVersion < config.sdkVersion && c.sdkVersion != 0 {
                score += 50 + c.sdkVersion as i32;
            }

            if c.density == config.density && c.density != 0 {
                score += 200;
            } else if c.density != 0 {
                let diff = (c.density as i32 - config.density as i32).abs();
                score += (160 - diff).max(0); 
            }

            if c == &ResConfig::default() {
                score = 0;
            }

            if score > best_score {
                best_score = score;
                best_match = Some(val.clone());
            }
        }
        best_match
    }
}

#[derive(Debug, Clone)]
pub enum ResourceValue {
    String(String),
    Integer(u32),
    Boolean(bool),
    Reference(u32),
}

pub struct Package {
    pub id: u32,
    pub name: String,
    pub type_names: Vec<String>,
    pub key_names: Vec<String>,
}

pub fn parse_resources(data: &[u8]) -> ResourceResult<ResourceTable> {
    let mut offset = 0;
    let table_header: ChunkHeader = data.pread_with(offset, LE)?;
    if table_header.typ != 0x0002 {
        return Err(ResourceError::Parse("Not a resource table".into()));
    }
    offset += 8;
    let _package_count: u32 = data.pread_with(offset, LE)?;
    offset += 4;

    let mut global_strings = Vec::new();
    let mut resource_map: HashMap<u32, Vec<(ResConfig, ResourceValue)>> = HashMap::new();
    let mut current_package: Option<Package> = None;

    while offset < data.len() {
        let chunk: ChunkHeader = data.pread_with(offset, LE)?;
        let chunk_start = offset;

        match chunk.typ {
            0x0001 => { 
                let strings = parse_string_pool(&data[offset..offset + chunk.size as usize])?;
                if global_strings.is_empty() {
                    global_strings = strings;
                } else if let Some(ref mut pkg) = current_package {
                    pkg.key_names = strings;
                }
            }
            0x0200 => { 
                let mut poff = offset + 8;
                let id: u32 = data.pread_with(poff, LE)?; poff += 4;
                let mut name_bytes = [0u16; 128];
                for i in 0..128 { name_bytes[i] = data.pread_with(poff + (i * 2), LE)?; }
                let name = String::from_utf16_lossy(&name_bytes).trim_matches('\0').to_string();
                
                current_package = Some(Package {
                    id,
                    name,
                    type_names: vec![],
                    key_names: vec![],
                });
            }
            0x0201 => { 
                if let Some(ref pkg) = current_package {
                    let mut toff = offset + 8;
                    let type_id: u8 = data.pread(toff)?; toff += 1;
                    let _res0: u8 = data.pread(toff)?; toff += 1;
                    let _res1: u16 = data.pread_with(toff, LE)?; toff += 2;
                    let entry_count: u32 = data.pread_with(toff, LE)?; toff += 4;
                    let entries_start: u32 = data.pread_with(toff, LE)?; toff += 4;
                    
                    
                    let config_start = toff;
                    let config_size: u32 = data.pread_with(config_start, LE)?;
                    let mut config = ResConfig::default();
                    if config_size >= 28 {
                        config.mcc = data.pread_with(config_start + 4, LE)?;
                        config.mnc = data.pread_with(config_start + 6, LE)?;
                        config.language = [data.pread(config_start + 8)?, data.pread(config_start + 9)?];
                        config.country = [data.pread(config_start + 10)?, data.pread(config_start + 11)?];
                        config.orientation = data.pread(config_start + 12)?;
                        config.touchscreen = data.pread(config_start + 13)?;
                        config.density = data.pread_with(config_start + 14, LE)?;
                        config.keyboard = data.pread(config_start + 16)?;
                        config.navigation = data.pread(config_start + 17)?;
                        config.inputFlags = data.pread(config_start + 18)?;
                        config.screenWidth = data.pread_with(config_start + 20, LE)?;
                        config.screenHeight = data.pread_with(config_start + 22, LE)?;
                        config.sdkVersion = data.pread_with(config_start + 24, LE)?;
                        config.minorVersion = data.pread_with(config_start + 26, LE)?;
                    }
                    if config_size >= 32 {
                        config.screenLayout = data.pread(config_start + 28)?;
                        config.uiMode = data.pread(config_start + 29)?;
                        config.smallestScreenWidthDp = data.pread_with(config_start + 30, LE)?;
                    }
                    if config_size >= 36 {
                        config.screenWidthDp = data.pread_with(config_start + 32, LE)?;
                        config.screenHeightDp = data.pread_with(config_start + 34, LE)?;
                    }
                    toff += config_size as usize;
                    
                    let base_id = (pkg.id << 24) | ((type_id as u32) << 16);
                    
                    for i in 0..entry_count {
                        let entry_idx_off = toff + (i as usize * 4);
                        if entry_idx_off >= chunk_start + chunk.size as usize { break; }
                        let entry_off: u32 = data.pread_with(entry_idx_off, LE)?;
                        
                        if entry_off != 0xFFFFFFFF {
                            let mut e_ptr = chunk_start + entries_start as usize + entry_off as usize;
                            let _size: u16 = data.pread_with(e_ptr, LE)?; e_ptr += 2;
                            let flags: u16 = data.pread_with(e_ptr, LE)?; e_ptr += 2;
                            let _key_idx: u32 = data.pread_with(e_ptr, LE)?; e_ptr += 4;
                            
                            if (flags & 0x0001) == 0 { 
                                let _val_size: u16 = data.pread_with(e_ptr, LE)?; e_ptr += 2;
                                let _val_res: u8 = data.pread(e_ptr)?; e_ptr += 1;
                                let val_type: u8 = data.pread(e_ptr)?; e_ptr += 1;
                                let val_data: u32 = data.pread_with(e_ptr, LE)?;
                                
                                let res_id = base_id | (i as u32);
                                let val = match val_type {
                                    0x03 => Some(ResourceValue::String(global_strings.get(val_data as usize).cloned().unwrap_or_default())),
                                    0x10..=0x11 => Some(ResourceValue::Integer(val_data)),
                                    0x12 => Some(ResourceValue::Boolean(val_data != 0)),
                                    0x01 => Some(ResourceValue::Reference(val_data)),
                                    _ => None,
                                };
                                
                                if let Some(v) = val {
                                    resource_map.entry(res_id).or_insert_with(Vec::new).push((config.clone(), v));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        offset = chunk_start + chunk.size as usize;
    }

    Ok(ResourceTable { strings: global_strings, resource_map })
}

fn parse_string_pool(data: &[u8]) -> ResourceResult<Vec<String>> {
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
            let mut utf16_data = Vec::with_capacity(char_len as usize);
            for j in 0..char_len {
                utf16_data.push(data.pread_with::<u16>(str_off + (j as usize * 2), LE).unwrap());
            }
            strings.push(String::from_utf16_lossy(&utf16_data));
        }
    }
    Ok(strings)
}

fn read_len_utf8(data: &[u8], off: usize) -> (u32, usize) {
    let b: u8 = data.pread(off).unwrap_or(0);
    if b & 0x80 != 0 {
        (((b & 0x7f) as u32) << 8 | data.pread::<u8>(off + 1).unwrap_or(0) as u32, 2)
    } else {
        (b as u32, 1)
    }
}

fn read_len_utf16(data: &[u8], off: usize) -> (u32, usize) {
    let b: u16 = data.pread_with(off, LE).unwrap_or(0);
    if b & 0x8000 != 0 {
        (((b & 0x7fff) as u32) << 16 | data.pread_with::<u16>(off + 2, LE).unwrap_or(0) as u32, 4)
    } else {
        (b as u32, 2)
    }
}
