use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use libloading::{Library, Symbol};



#[repr(C)]
pub struct JavaVM {
    pub functions: *const JNIInvokeInterface,
}

#[repr(C)]
pub struct JNIInvokeInterface {
    pub funcs: [*mut c_void; 8],
}

#[repr(C)]
pub struct JNIEnv {
    pub functions: *const JNINativeInterface,
}

#[repr(C)]
pub struct JNINativeInterface {
    pub funcs: [*mut c_void; 230],
}

#[repr(C)]
struct JNINativeMethod {
    name: *const c_char,
    signature: *const c_char,
    fn_ptr: *mut c_void,
}



enum JniObject {
    Null,
    Str(String),
    Class(String), 
    VmObject(u32),
}

struct JniGlobals {
    invoke_interface: JNIInvokeInterface,
    java_vm: JavaVM,
    native_interface: JNINativeInterface,
    jni_env: JNIEnv,
    env_ptr: *mut JNIEnv,
    vm_ptr: *mut JavaVM,
    libraries: Vec<Library>,
    
    objects: Vec<JniObject>,
    
    methods: Vec<(usize, String, String)>,
    
    fields: Vec<(usize, String, String)>,
    
    c_strings: Vec<CString>,
    
    pending_calls: Vec<(String, String, String)>,
    
    pub registered_natives: HashMap<String, usize>,
    pending_exception: Option<usize>,
}

static mut GLOBALS: *mut JniGlobals = ptr::null_mut();


fn jni_alloc(obj: JniObject) -> usize {
    unsafe {
        let g = &mut *GLOBALS;
        g.objects.push(obj);
        g.objects.len() 
    }
}


fn get_class_desc(handle: usize) -> String {
    unsafe {
        let g = &*GLOBALS;
        match g.objects.get(handle - 1) {
            Some(JniObject::Class(s)) => s.clone(),
            _ => String::new(),
        }
    }
}






extern "C" fn jni_get_version(_env: *mut JNIEnv) -> i32 {
    0x00010006
}


extern "C" fn jni_find_class(_env: *mut JNIEnv, name: *const c_char) -> usize {
    unsafe {
        let class_name = CStr::from_ptr(name).to_str().unwrap_or("");
        let descriptor = format!("L{};", class_name);
        println!("[JNI] FindClass(\"{}\") -> \"{}\"", class_name, descriptor);
        jni_alloc(JniObject::Class(descriptor))
    }
}


extern "C" fn jni_exception_occurred(_env: *mut JNIEnv) -> usize {
    unsafe {
        if GLOBALS.is_null() { 0 } else { (*GLOBALS).pending_exception.unwrap_or(0) }
    }
}


extern "C" fn jni_exception_describe(_env: *mut JNIEnv) {
    unsafe {
        if !GLOBALS.is_null() {
            if let Some(ex) = (*GLOBALS).pending_exception {
                println!("[JNI Exception] Description: {:?}", resolve_jni_handle(ex));
            }
        }
    }
}


extern "C" fn jni_exception_clear(_env: *mut JNIEnv) {
    unsafe {
        if !GLOBALS.is_null() {
            (*GLOBALS).pending_exception = None;
        }
    }
}


extern "C" fn jni_push_local_frame(_env: *mut JNIEnv, _cap: i32) -> i32 { 0 }


extern "C" fn jni_pop_local_frame(_env: *mut JNIEnv, _result: usize) -> usize { 0 }


extern "C" fn jni_new_global_ref(_env: *mut JNIEnv, obj: usize) -> usize { obj }


extern "C" fn jni_delete_global_ref(_env: *mut JNIEnv, _obj: usize) {}


extern "C" fn jni_delete_local_ref(_env: *mut JNIEnv, _obj: usize) {}


extern "C" fn jni_ensure_local_capacity(_env: *mut JNIEnv, _cap: i32) -> i32 { 0 }


extern "C" fn jni_get_object_class(_env: *mut JNIEnv, _obj: usize) -> usize {
    
    jni_alloc(JniObject::Class("Ljava/lang/Object;".into()))
}


extern "C" fn jni_is_instance_of(_env: *mut JNIEnv, _obj: usize, _clazz: usize) -> u8 {
    1 
}


extern "C" fn jni_get_method_id(
    _env: *mut JNIEnv, clazz: usize, name: *const c_char, sig: *const c_char,
) -> usize {
    unsafe {
        let method_name = CStr::from_ptr(name).to_str().unwrap_or("").to_string();
        let method_sig = CStr::from_ptr(sig).to_str().unwrap_or("").to_string();
        let class_desc = get_class_desc(clazz);
        println!("[JNI] GetMethodID({}, \"{}\", \"{}\")", class_desc, method_name, method_sig);
        let g = &mut *GLOBALS;
        g.methods.push((clazz, method_name, method_sig));
        g.methods.len()
    }
}


extern "C" fn jni_get_field_id(
    _env: *mut JNIEnv, clazz: usize, name: *const c_char, sig: *const c_char,
) -> usize {
    unsafe {
        let field_name = CStr::from_ptr(name).to_str().unwrap_or("").to_string();
        let field_sig = CStr::from_ptr(sig).to_str().unwrap_or("").to_string();
        let class_desc = get_class_desc(clazz);
        println!("[JNI] GetFieldID({}, \"{}\", \"{}\")", class_desc, field_name, field_sig);
        let g = &mut *GLOBALS;
        g.fields.push((clazz, field_name, field_sig));
        g.fields.len()
    }
}


extern "C" fn jni_get_static_method_id(
    _env: *mut JNIEnv, clazz: usize, name: *const c_char, sig: *const c_char,
) -> usize {
    unsafe {
        let method_name = CStr::from_ptr(name).to_str().unwrap_or("").to_string();
        let method_sig = CStr::from_ptr(sig).to_str().unwrap_or("").to_string();
        let class_desc = get_class_desc(clazz);
        println!("[JNI] GetStaticMethodID({}, \"{}\", \"{}\")", class_desc, method_name, method_sig);
        let g = &mut *GLOBALS;
        g.methods.push((clazz, method_name, method_sig));
        g.methods.len()
    }
}


extern "C" fn jni_call_static_object_method(
    _env: *mut JNIEnv, _clazz: usize, method_id: usize,
) -> usize {
    unsafe {
        let g = &*GLOBALS;
        if let Some((ch, mn, ms)) = g.methods.get(method_id - 1) {
            let cd = get_class_desc(*ch);
            println!("[JNI] CallStaticObjectMethod: {}->{}{}  (returning null)", cd, mn, ms);
        }
    }
    0 
}


extern "C" fn jni_call_static_boolean_method(
    _env: *mut JNIEnv, _clazz: usize, _method_id: usize,
) -> u8 {
    0 
}


extern "C" fn jni_call_static_int_method(
    _env: *mut JNIEnv, _clazz: usize, _method_id: usize,
) -> i32 {
    0
}


extern "C" fn jni_call_static_void_method(
    _env: *mut JNIEnv, _clazz: usize, method_id: usize,
) {
    unsafe {
        let g = &mut *GLOBALS;
        if let Some((class_handle, method_name, method_sig)) = g.methods.get(method_id - 1).cloned() {
            let class_desc = get_class_desc(class_handle);
            println!("[JNI] CallStaticVoidMethod: {}->{}{}",
                class_desc, method_name, method_sig);
            g.pending_calls.push((class_desc, method_name, method_sig));
        }
    }
}


extern "C" fn jni_get_static_field_id(
    _env: *mut JNIEnv, clazz: usize, name: *const c_char, sig: *const c_char,
) -> usize {
    unsafe {
        let field_name = CStr::from_ptr(name).to_str().unwrap_or("").to_string();
        let field_sig = CStr::from_ptr(sig).to_str().unwrap_or("").to_string();
        let class_desc = get_class_desc(clazz);
        println!("[JNI] GetStaticFieldID({}, \"{}\", \"{}\")", class_desc, field_name, field_sig);
        let g = &mut *GLOBALS;
        g.fields.push((clazz, field_name, field_sig));
        g.fields.len()
    }
}


extern "C" fn jni_get_static_object_field(
    _env: *mut JNIEnv, _clazz: usize, _field_id: usize,
) -> usize {
    0 
}


extern "C" fn jni_get_int_field(
    _env: *mut JNIEnv, _obj: usize, field_id: usize,
) -> i32 {
    unsafe {
        let g = &*GLOBALS;
        if let Some((_ch, fnm, _fs)) = g.fields.get(field_id - 1) {
             println!("[JNI] GetIntField: {} (returning 0)", fnm);
        }
    }
    0
}


extern "C" fn jni_call_void_method(
    _env: *mut JNIEnv, _obj: usize, method_id: usize,
) {
    unsafe {
        let g = &*GLOBALS;
        if let Some((_ch, mn, ms)) = g.methods.get(method_id - 1) {
             println!("[JNI] CallVoidMethod: {}{}", mn, ms);
        }
    }
}


extern "C" fn jni_get_static_int_field(
    _env: *mut JNIEnv, _clazz: usize, field_id: usize,
) -> i32 {
    unsafe {
        let g = &*GLOBALS;
        if let Some((_ch, fnm, _fs)) = g.fields.get(field_id - 1) {
             println!("[JNI] GetStaticIntField: {} (returning 0)", fnm);
        }
    }
    0
}


extern "C" fn jni_new_string_utf(_env: *mut JNIEnv, utf: *const c_char) -> usize {
    unsafe {
        let s = if utf.is_null() {
            String::new()
        } else {
            CStr::from_ptr(utf).to_str().unwrap_or("").to_string()
        };
        println!("[JNI] NewStringUTF(\"{}\")", s);
        jni_alloc(JniObject::Str(s))
    }
}


extern "C" fn jni_get_string_utf_length(_env: *mut JNIEnv, str_ref: usize) -> i32 {
    unsafe {
        let g = &*GLOBALS;
        match g.objects.get(str_ref - 1) {
            Some(JniObject::Str(s)) => s.len() as i32,
            _ => 0,
        }
    }
}


extern "C" fn jni_get_string_utf_chars(
    _env: *mut JNIEnv, str_ref: usize, is_copy: *mut u8,
) -> *const c_char {
    unsafe {
        if !is_copy.is_null() { *is_copy = 1; }
        let g = &mut *GLOBALS;
        let s = match g.objects.get(str_ref - 1) {
            Some(JniObject::Str(s)) => s.clone(),
            _ => String::new(),
        };
        let cs = CString::new(s).unwrap_or_default();
        let ptr = cs.as_ptr();
        g.c_strings.push(cs); 
        ptr
    }
}


extern "C" fn jni_release_string_utf_chars(
    _env: *mut JNIEnv, _str_ref: usize, _chars: *const c_char,
) {
    
}


extern "C" fn jni_register_natives(
    _env: *mut JNIEnv, clazz: usize, methods: *const JNINativeMethod, n_methods: i32,
) -> i32 {
    unsafe {
        let class_desc = get_class_desc(clazz);
        let g = &mut *GLOBALS;
        for i in 0..n_methods as isize {
            let m = &*methods.offset(i);
            let name = CStr::from_ptr(m.name).to_str().unwrap_or("");
            let sig = CStr::from_ptr(m.signature).to_str().unwrap_or("");
            let key = format!("{}->{}{}", class_desc, name, sig);
            println!("[JNI] RegisterNatives: {} @ {:p}", key, m.fn_ptr);
            g.registered_natives.insert(key, m.fn_ptr as usize);
        }
    }
    0 
}


extern "C" fn jni_get_java_vm(_env: *mut JNIEnv, vm: *mut *mut JavaVM) -> i32 {
    unsafe {
        *vm = (*GLOBALS).vm_ptr;
    }
    0
}


extern "C" fn jni_exception_check(_env: *mut JNIEnv) -> u8 {
    unsafe {
        if GLOBALS.is_null() { 0 } else { if (*GLOBALS).pending_exception.is_some() { 1 } else { 0 } }
    }
}

extern "C" fn jni_throw_new(_env: *mut JNIEnv, _clazz: usize, message: *const c_char) -> i32 {
    unsafe {
        let msg = CStr::from_ptr(message).to_str().unwrap_or("").to_string();
        println!("[JNI] ThrowNew: \"{}\"", msg);
        let ex_handle = jni_alloc(JniObject::Str(format!("JNIException: {}", msg)));
        (*GLOBALS).pending_exception = Some(ex_handle);
        0
    }
}






extern "C" fn jni_get_env(_vm: *mut JavaVM, penv: *mut *mut c_void, version: i32) -> i32 {
    println!("[JNI] GetEnv(version=0x{:x})", version);
    unsafe {
        *penv = (*GLOBALS).env_ptr as *mut c_void;
    }
    0
}





pub fn init_jni() {
    unsafe {
        if !GLOBALS.is_null() { return; }

        let mut g = Box::new(JniGlobals {
            invoke_interface: JNIInvokeInterface { funcs: [ptr::null_mut(); 8] },
            java_vm: JavaVM { functions: ptr::null() },
            native_interface: JNINativeInterface { funcs: [ptr::null_mut(); 230] },
            jni_env: JNIEnv { functions: ptr::null() },
            env_ptr: ptr::null_mut(),
            vm_ptr: ptr::null_mut(),
            libraries: Vec::new(),
            objects: Vec::new(),
            methods: Vec::new(),
            fields: Vec::new(),
            c_strings: Vec::new(),
            pending_calls: Vec::new(),
            registered_natives: HashMap::new(),
            pending_exception: None,
        });

        
        for i in 0..8 { g.invoke_interface.funcs[i] = jni_unimplemented as *mut c_void; }
        for i in 0..230 { g.native_interface.funcs[i] = jni_unimplemented as *mut c_void; }

        
        g.invoke_interface.funcs[6] = jni_get_env as *mut c_void;

        
        let ni = &mut g.native_interface.funcs;
        ni[4]   = jni_get_version as *mut c_void;
        ni[6]   = jni_find_class as *mut c_void;
        ni[14]  = jni_throw_new as *mut c_void;
        ni[15]  = jni_exception_occurred as *mut c_void;
        ni[16]  = jni_exception_describe as *mut c_void;
        ni[17]  = jni_exception_clear as *mut c_void;
        ni[19]  = jni_push_local_frame as *mut c_void;
        ni[20]  = jni_pop_local_frame as *mut c_void;
        ni[21]  = jni_new_global_ref as *mut c_void;
        ni[22]  = jni_delete_global_ref as *mut c_void;
        ni[23]  = jni_delete_local_ref as *mut c_void;
        ni[26]  = jni_ensure_local_capacity as *mut c_void;
        ni[31]  = jni_get_object_class as *mut c_void;
        ni[32]  = jni_is_instance_of as *mut c_void;
        ni[33]  = jni_get_method_id as *mut c_void;
        ni[61]  = jni_call_void_method as *mut c_void;
        ni[94]  = jni_get_field_id as *mut c_void;
        ni[100] = jni_get_int_field as *mut c_void;
        ni[113] = jni_get_static_method_id as *mut c_void;
        ni[114] = jni_call_static_object_method as *mut c_void;
        ni[117] = jni_call_static_boolean_method as *mut c_void;
        ni[129] = jni_call_static_int_method as *mut c_void;
        ni[141] = jni_call_static_void_method as *mut c_void;
        ni[144] = jni_get_static_field_id as *mut c_void;
        ni[145] = jni_get_static_object_field as *mut c_void;
        ni[149] = jni_get_static_int_field as *mut c_void;
        ni[167] = jni_new_string_utf as *mut c_void;
        ni[168] = jni_get_string_utf_length as *mut c_void;
        ni[169] = jni_get_string_utf_chars as *mut c_void;
        ni[170] = jni_release_string_utf_chars as *mut c_void;
        ni[215] = jni_register_natives as *mut c_void;
        ni[219] = jni_get_java_vm as *mut c_void;
        ni[228] = jni_exception_check as *mut c_void;

        GLOBALS = Box::into_raw(g);

        (*GLOBALS).java_vm.functions = &(*GLOBALS).invoke_interface;
        (*GLOBALS).jni_env.functions = &(*GLOBALS).native_interface;
        (*GLOBALS).vm_ptr = &mut (*GLOBALS).java_vm;
        (*GLOBALS).env_ptr = &mut (*GLOBALS).jni_env;
    }
}

extern "C" fn jni_unimplemented() {
    println!("[JNI] WARNING: Unimplemented JNI function called!");
}





pub fn load_library(path: &str) -> bool {
    unsafe {
        init_jni();

        let full_path = format!("/home/asadula/LRT/test_build/lib{}.so", path);
        println!("[JNI] Loading library: {}", full_path);
        match Library::new(&full_path) {
            Ok(lib) => {
                println!("[JNI] Loaded! Searching for JNI_OnLoad...");
                let onload: Result<Symbol<unsafe extern "C" fn(*mut JavaVM, *mut c_void) -> i32>, _> = lib.get(b"JNI_OnLoad");
                match onload {
                    Ok(func) => {
                        let version = func((*GLOBALS).vm_ptr, ptr::null_mut());
                        println!("[JNI] JNI_OnLoad returned version 0x{:x}", version);
                    }
                    Err(_) => {
                        println!("[JNI] No JNI_OnLoad found (library loaded anyway).");
                    }
                }
                (*GLOBALS).libraries.push(lib);
                true
            }
            Err(e) => {
                println!("[JNI] Failed to load {}: {}", full_path, e);
                false
            }
        }
    }
}

pub fn drain_pending_calls() -> Vec<(String, String, String)> {
    unsafe {
        if GLOBALS.is_null() { return Vec::new(); }
        std::mem::take(&mut (*GLOBALS).pending_calls)
    }
}

pub fn get_registered_natives() -> HashMap<String, usize> {
    unsafe {
        if GLOBALS.is_null() { return HashMap::new(); }
        (*GLOBALS).registered_natives.clone()
    }
}

pub fn get_env_ptr() -> *mut JNIEnv {
    unsafe {
        init_jni();
        (*GLOBALS).env_ptr
    }
}

pub fn get_or_create_vm_object_handle(heap_id: u32) -> usize {
    if heap_id == 0 { return 0; }
    unsafe {
        init_jni();
        let g = &mut *GLOBALS;
        for (i, obj) in g.objects.iter().enumerate() {
            if let JniObject::VmObject(hid) = obj {
                if *hid == heap_id {
                    return i + 1;
                }
            }
        }
        jni_alloc(JniObject::VmObject(heap_id))
    }
}

pub fn get_or_create_string_handle(s: String) -> usize {
    unsafe {
        init_jni();
        let g = &mut *GLOBALS;
        for (i, obj) in g.objects.iter().enumerate() {
            if let JniObject::Str(existing) = obj {
                if existing == &s {
                    return i + 1;
                }
            }
        }
        jni_alloc(JniObject::Str(s))
    }
}

pub fn get_or_create_class_handle(descriptor: &str) -> usize {
    unsafe {
        init_jni();
        let g = &mut *GLOBALS;
        for (i, obj) in g.objects.iter().enumerate() {
            if let JniObject::Class(s) = obj {
                if s == descriptor {
                    return i + 1;
                }
            }
        }
        jni_alloc(JniObject::Class(descriptor.to_string()))
    }
}

#[derive(Debug, Clone)]
pub enum JniToVmMapResult {
    Null,
    String(String),
    Class(String),
    VmObject(u32),
}

pub fn resolve_jni_handle(handle: usize) -> JniToVmMapResult {
    unsafe {
        if handle == 0 || GLOBALS.is_null() { return JniToVmMapResult::Null; }
        let g = &*GLOBALS;
        match g.objects.get(handle - 1) {
            Some(JniObject::Str(s)) => JniToVmMapResult::String(s.clone()),
            Some(JniObject::Class(s)) => JniToVmMapResult::Class(s.clone()),
            Some(JniObject::VmObject(hid)) => JniToVmMapResult::VmObject(*hid),
            _ => JniToVmMapResult::Null,
        }
    }
}

pub unsafe fn invoke_jni_method(
    fn_ptr: usize,
    env: *mut JNIEnv,
    second_arg: usize,
    args: &[u32],
) -> Option<u32> {
    match args.len() {
        0 => {
            let f: extern "C" fn(*mut JNIEnv, usize) -> u64 = unsafe { std::mem::transmute(fn_ptr) };
            Some(f(env, second_arg) as u32)
        }
        1 => {
            let f: extern "C" fn(*mut JNIEnv, usize, u32) -> u64 = unsafe { std::mem::transmute(fn_ptr) };
            Some(f(env, second_arg, args[0]) as u32)
        }
        2 => {
            let f: extern "C" fn(*mut JNIEnv, usize, u32, u32) -> u64 = unsafe { std::mem::transmute(fn_ptr) };
            Some(f(env, second_arg, args[0], args[1]) as u32)
        }
        3 => {
            let f: extern "C" fn(*mut JNIEnv, usize, u32, u32, u32) -> u64 = unsafe { std::mem::transmute(fn_ptr) };
            Some(f(env, second_arg, args[0], args[1], args[2]) as u32)
        }
        4 => {
            let f: extern "C" fn(*mut JNIEnv, usize, u32, u32, u32, u32) -> u64 = unsafe { std::mem::transmute(fn_ptr) };
            Some(f(env, second_arg, args[0], args[1], args[2], args[3]) as u32)
        }
        _ => {
            println!("[JNI] Warning: Native methods with more than 4 arguments are not supported in invoke_jni_method yet");
            None
        }
    }
}

pub fn has_pending_exception() -> bool {
    unsafe {
        if GLOBALS.is_null() { false } else { (*GLOBALS).pending_exception.is_some() }
    }
}

pub fn get_and_clear_pending_exception() -> Option<usize> {
    unsafe {
        if GLOBALS.is_null() { None } else { (*GLOBALS).pending_exception.take() }
    }
}

