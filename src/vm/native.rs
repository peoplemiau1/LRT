use std::collections::HashMap;
use crate::dex::DexResult;
use super::{Vm, Object};

pub type NativeMethod = fn(&mut Vm, &[u32]) -> DexResult<Option<u32>>;

/// Register built-in native method implementations used by the VM.
///
/// Returns a HashMap that maps Java native method descriptors (for example
/// `"Ljava/io/PrintStream;->println(I)V"`) to Rust `NativeMethod` function pointers
/// implementing those natives.
///
/// # Examples
///
/// ```
/// let natives = get_native_methods();
/// assert!(natives.contains_key("Ljava/lang/Object;-><init>()V"));
/// assert!(natives.contains_key("Ljava/io/PrintStream;->println(Ljava/lang/String;)V"));
/// ```
pub fn get_native_methods() -> HashMap<String, NativeMethod> {
    let mut m: HashMap<String, NativeMethod> = HashMap::new();
    
    
    m.insert("Ljava/lang/Object;-><init>()V".into(), |_vm, _args| Ok(None));
    m.insert("Ljava/lang/RuntimeException;-><init>(Ljava/lang/String;)V".into(), |_vm, _args| Ok(None));

    
    m.insert("Ljava/io/PrintStream;->println(Ljava/lang/String;)V".into(), |vm, args| {
        vm.native_println(args[1]);
        Ok(None)
    });

    m.insert("Ljava/io/PrintStream;->println(I)V".into(), |_vm, args| {
        println!("[STDOUT]: {}", args[1] as i32);
        Ok(None)
    });

    
    m.insert("Ljava/lang/StringBuilder;-><init>()V".into(), |vm, args| {
        let obj_id = args[0];
        let val_id = vm.alloc(Object::String("".into()));
        if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get_mut(obj_id as usize) {
            fields.insert(0, val_id);
        }
        Ok(None)
    });

    m.insert("Ljava/lang/StringBuilder;-><init>(Ljava/lang/String;)V".into(), |vm, args| {
        let obj_id = args[0];
        let val_id = args[1];
        if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get_mut(obj_id as usize) {
            fields.insert(0, val_id);
        }
        Ok(None)
    });

    m.insert("Ljava/lang/StringBuilder;->append(Ljava/lang/Object;)Ljava/lang/StringBuilder;".into(), |vm, args| {
        let sb_id = args[0];
        let obj_id = args[1];
        let to_add = if obj_id == 0 {
            "null".to_string()
        } else {
            let s = vm.state.lock().unwrap();
            match s.heap.get(obj_id as usize) {
                Some(Object::String(s)) => s.clone(),
                Some(Object::Instance { class_desc, .. }) => format!("Instance of {}", class_desc),
                Some(Object::Array { element_type, .. }) => format!("Array of {}", element_type),
                _ => format!("Object@{}", obj_id),
            }
        };

        let val_id = if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get(sb_id as usize) {
            *fields.get(&0).unwrap_or(&0)
        } else { 0 };

        let mut current_str = vm.get_string_val(val_id).unwrap_or_default();
        current_str.push_str(&to_add);
        let new_id = vm.alloc(Object::String(current_str));

        if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get_mut(sb_id as usize) {
            fields.insert(0, new_id);
        }
        Ok(Some(sb_id))
    });

    m.insert("Ljava/lang/StringBuilder;->append(C)Ljava/lang/Appendable;".into(), |vm, args| {
        let sb_id = args[0];
        let to_add = if let Some(c) = std::char::from_u32(args[1]) { c.to_string() } else { "".to_string() };
        
        let val_id = if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get(sb_id as usize) {
            *fields.get(&0).unwrap_or(&0)
        } else { 0 };

        let mut current_str = vm.get_string_val(val_id).unwrap_or_default();
        current_str.push_str(&to_add);
        let new_id = vm.alloc(Object::String(current_str));

        if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get_mut(sb_id as usize) {
            fields.insert(0, new_id);
        }
        Ok(Some(sb_id))
    });

    m.insert("Ljava/lang/StringBuilder;->append(Ljava/lang/String;)Ljava/lang/StringBuilder;".into(), |vm, args| {
        let sb_id = args[0];
        let str_id = args[1];
        let to_add = vm.get_string_val(str_id).unwrap_or_else(|| "null".into());
        
        let val_id = if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get(sb_id as usize) {
            *fields.get(&0).unwrap_or(&0)
        } else { 0 };

        let mut current_str = vm.get_string_val(val_id).unwrap_or_default();
        current_str.push_str(&to_add);
        let new_id = vm.alloc(Object::String(current_str));

        if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get_mut(sb_id as usize) {
            fields.insert(0, new_id);
        }
        Ok(Some(sb_id))
    });

    m.insert("Ljava/lang/StringBuilder;->append(I)Ljava/lang/StringBuilder;".into(), |vm, args| {
        let sb_id = args[0];
        let to_add = (args[1] as i32).to_string();
        
        let val_id = if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get(sb_id as usize) {
            *fields.get(&0).unwrap_or(&0)
        } else { 0 };

        let mut current_str = vm.get_string_val(val_id).unwrap_or_default();
        current_str.push_str(&to_add);
        let new_id = vm.alloc(Object::String(current_str));

        if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get_mut(sb_id as usize) {
            fields.insert(0, new_id);
        }
        Ok(Some(sb_id))
    });

    m.insert("Ljava/lang/StringBuilder;->toString()Ljava/lang/String;".into(), |vm, args| {
        let sb_id = args[0];
        let val_id = if let Some(Object::Instance { fields, .. }) = vm.state.lock().unwrap().heap.get(sb_id as usize) {
            *fields.get(&0).unwrap_or(&0)
        } else { 0 };

        let res = vm.get_string_val(val_id).unwrap_or_default();
        let id = vm.alloc(Object::String(res));
        Ok(Some(id))
    });

    
    m.insert("Lcom/example/tinyart/GrandTest;->getResourceMock(I)Ljava/lang/String;".into(), |vm, args| {
        let res_id = args[0];
        let text = vm.get_resource_string(res_id).unwrap_or_else(|| format!("MOCK_RES_{:08x}", res_id));
        let id = vm.alloc(Object::String(text));
        Ok(Some(id))
    });

    
    m.insert("Ljava/lang/System;->loadLibrary(Ljava/lang/String;)V".into(), |vm, args| {
        let lib_name_id = args[0];
        let lib_name = vm.get_string_val(lib_name_id).unwrap_or_default();
        println!("[VM] System.loadLibrary(\"{}\") called", lib_name);
        
        if crate::jni::load_library(&lib_name) {
            let pending = crate::jni::drain_pending_calls();
            for (class_desc, method_name, _method_sig) in pending {
                if let Err(e) = vm.call_static_by_name(&class_desc, &method_name) {
                    println!("[VM] JNI pending call error: {:?}", e);
                }
            }
        }
        Ok(None)
    });

    m
}
