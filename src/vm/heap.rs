use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Object {
    Null,
    Instance {
        class_idx: u32,
        fields: HashMap<u32, u32>,
    },
    Array {
        element_type: String,
        data: Vec<u32>,
    },
    String(String),
}
