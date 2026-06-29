pub mod heap;
pub mod native;
pub mod interpreter;
pub mod jit;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use scroll::{Pread, LE};
use crate::dex::{Dex, DexResult, DexError, EncodedValue};
use crate::resources::ResourceTable;
use jit::JitCompiler;
pub use heap::Object;
pub use native::NativeMethod;

pub struct SharedState {
    pub heap: Vec<Object>,
    pub free_list: Vec<u32>,
    pub monitors: HashMap<u32, Arc<Mutex<()>>>,
    pub static_fields: HashMap<(usize, u32), HashMap<u32, u64>>,
    pub initialized_classes: HashSet<String>,
}

pub struct Vm<'a> {
    pub dex: &'a Dex<'a>,
    pub extra_dexes: Vec<Dex<'a>>,
    pub state: Arc<Mutex<SharedState>>,
    pub last_result: Option<u32>,
    pub native_methods: HashMap<String, NativeMethod>,
    pub resources: Option<ResourceTable>,
    pub current_config: crate::resources::ResConfig,
    pub jit: JitCompiler,
    pub gc_threshold: usize,
    pub thread_id: usize,
    pub last_exception: Option<u32>,
    pub android_dex: Option<Dex<'a>>,
}

impl<'a> Vm<'a> {
    /// Creates a new VM instance using the given primary DEX.
    ///
    /// The returned VM is initialized with an empty shared heap (index 0 reserved as null), a fresh
    /// shared state wrapped in an `Arc<Mutex<_>>`, loaded native methods, default resource/config
    /// settings, a fresh JIT compiler, and a default GC threshold of 65536.
    ///
    /// # Examples
    ///
    /// ```
    /// // given a `dex: Dex<'_>` value:
    /// let vm = vm::Vm::new(&dex);
    /// ```
    pub fn new(dex: &'a Dex<'a>) -> Self {
        let state = SharedState {
            heap: vec![Object::Null], 
            free_list: Vec::new(),
            monitors: HashMap::new(),
            static_fields: HashMap::new(),
            initialized_classes: HashSet::new(),
        };

        Self {
            dex,
            extra_dexes: Vec::new(),
            state: Arc::new(Mutex::new(state)),
            last_result: None,
            native_methods: native::get_native_methods(),
            resources: None,
            current_config: crate::resources::ResConfig::default(),
            jit: JitCompiler::new(),
            gc_threshold: 65536,
            thread_id: 0,
            last_exception: None,
            android_dex: None,
        }
    }

    /// Create a VM instance configured for a new VM thread that reuses existing shared state.
    ///
    /// The returned VM shares heap, static fields, monitors and other global state via `state`
    /// and is assigned the provided thread identifier `id`.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `dex: &Dex` and `state: std::sync::Arc<std::sync::Mutex<SharedState>>` are available
    /// let thread_vm = Vm::new_thread(dex, state.clone(), 1);
    /// assert_eq!(thread_vm.thread_id, 1);
    /// ```
    pub fn new_thread(dex: &'a Dex<'a>, state: Arc<Mutex<SharedState>>, id: usize) -> Self {
        Self {
            dex,
            extra_dexes: Vec::new(),
            state,
            last_result: None,
            native_methods: native::get_native_methods(),
            resources: None,
            current_config: crate::resources::ResConfig::default(),
            jit: JitCompiler::new(),
            gc_threshold: 65536,
            thread_id: id,
            last_exception: None,
            android_dex: None,
        }
    }

    pub fn set_resources(&mut self, resources: ResourceTable) {
        self.resources = Some(resources);
    }

    /// Sets the VM's special Android dex that will be consulted for lookups that target the Android sentinel dex.
    ///
    /// The provided `dex` becomes the VM's android dex and will be used when operations reference the sentinel android dex index.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // `dex` is a loaded `Dex<'_>` instance.
    /// let dex: Dex = unsafe { std::mem::zeroed() };
    /// let mut vm = Vm::new(&dex);
    /// vm.set_android_dex(dex);
    /// ```
    pub fn set_android_dex(&mut self, dex: Dex<'a>) {
        self.android_dex = Some(dex);
    }

    /// Adds an additional Dex file to this VM's list of extra dexes.
    ///
    /// This makes the provided `dex` available for subsequent class and method lookups
    /// via `get_dex` and `find_class_in_dexes`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Construct VM with a primary Dex, then attach another Dex for lookups.
    /// let primary_dex = /* create or load a Dex<'_'> */ unimplemented!();
    /// let mut vm = Vm::new(&primary_dex);
    /// let extra = /* create or load another Dex<'_'> */ unimplemented!();
    /// vm.add_extra_dex(extra);
    /// assert_eq!(vm.extra_dexes.len(), 1);
    /// ```
    pub fn add_extra_dex(&mut self, dex: Dex<'a>) {
        self.extra_dexes.push(dex);
    }

    /// Selects the Dex corresponding to the provided index.
    ///
    /// `idx` of `0` refers to the VM's primary `dex`; any `idx > 0` selects the
    /// (idx - 1)th entry from `extra_dexes`.
    ///
    /// # Parameters
    ///
    /// - `idx`: index of the desired Dex (0 = primary, >0 = extra dex slot).
    ///
    /// # Returns
    ///
    /// A reference to the chosen `Dex<'a>` (primary when `idx == 0`, otherwise
    /// `&self.extra_dexes[idx - 1]`).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Given a Vm `vm` with at least one extra dex:
    /// let primary = vm.get_dex(0);
    /// let first_extra = vm.get_dex(1);
    /// ```
    pub fn get_dex(&self, idx: usize) -> &Dex<'a> {
        if idx == 0 {
            self.dex
        } else {
            &self.extra_dexes[idx - 1]
        }
    }

    /// Locate a class by its type descriptor in the VM's primary dex and extra dexes.
    ///
    /// Searches the primary dex first (returned with dex index `0`), then each `extra_dexes` entry
    /// in order (returned with dex index `i + 1` for the i-th extra dex). Returns `None` if the
    /// class is not found in any dex.
    ///
    /// # Returns
    ///
    /// `Some((dex_index, class_def_idx))` with the dex index and the class definition index when found,
    /// `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a VM `vm` with dex files loaded:
    /// if let Some((dex_idx, class_idx)) = vm.find_class_in_dexes("Lcom/example/MyClass;") {
    ///     println!("Found in dex {} at class index {}", dex_idx, class_idx);
    /// } else {
    ///     println!("Class not found");
    /// }
    /// ```
    pub fn find_class_in_dexes(&self, name: &str) -> Option<(usize, u32)> {
        if let Some(idx) = self.dex.find_class(name).ok().flatten() {
            return Some((0, idx));
        }
        for (i, d) in self.extra_dexes.iter().enumerate() {
            if let Some(idx) = d.find_class(name).ok().flatten() {
                return Some((i + 1, idx));
            }
        }
        None
    }

    /// Resolve a method by name starting from `class_desc` and walking up the superclass chain across available dex files.
    ///
    /// The function searches the VM's primary dex and any added extra dexes for `class_desc`; if found, it scans that class's virtual and direct methods
    /// for a method whose decoded name equals `method_name`. If not found in the class, the search continues with the superclass and repeats until the root.
    /// If no match is found in the primary/extra dexes, the same search is performed against the optional `android_dex` (matches there are reported with the
    /// sentinel dex index `0xFFFFFFFE`).
    ///
    /// # Returns
    ///
    /// `Some((dex_idx, class_def_idx, method_idx))` when a matching method is found, where `dex_idx` identifies the dex containing the resolved class
    /// (or `0xFFFFFFFE` for `android_dex`), otherwise `None`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Find a static method "main" on class descriptor "Ljava/lang/String;" (example only)
    /// # use vm::Vm;
    /// # fn example(vm: &Vm) {
    /// if let Some((dex_idx, class_def_idx, method_idx)) = vm.resolve_method_by_name("Lcom/example/MyClass;", "doWork") {
    ///     println!("Found method in dex {}, class {}, method {}", dex_idx, class_def_idx, method_idx);
    /// } else {
    ///     println!("Method not found");
    /// }
    /// # }
    /// ```
    pub fn resolve_method_by_name(&self, class_desc: &str, method_name: &str) -> Option<(usize, u32, u32)> {
        let mut current_class_name = class_desc.to_string();
        loop {
            if let Some((d_idx, c_def)) = self.find_class_in_dexes(&current_class_name) {
                let active_dex = self.get_dex(d_idx);
                if let Ok(class_data) = active_dex.get_class_data(c_def) {
                    for m in class_data.virtual_methods.iter().chain(class_data.direct_methods.iter()) {
                        if let Ok(m_name) = active_dex.get_method_name(m.method_idx) {
                            if m_name == method_name {
                                return Some((d_idx, c_def, m.method_idx));
                            }
                        }
                    }
                }
                
                let off = active_dex.header.class_defs_off as usize + (c_def as usize * 32);
                if let Ok(class_def) = active_dex.data.pread_with::<crate::dex::ClassDef>(off, LE) {
                    if class_def.superclass_idx == 0xFFFFFFFF { break; }
                    if let Ok(super_class_name) = active_dex.get_type(class_def.superclass_idx) {
                        current_class_name = super_class_name;
                        continue;
                    }
                }
            }
            break;
        }
        
        if let Some(ref ad) = self.android_dex {
            let mut current_class_name = class_desc.to_string();
            loop {
                if let Some(c_def) = ad.find_class(&current_class_name).ok().flatten() {
                    if let Ok(class_data) = ad.get_class_data(c_def) {
                        for m in class_data.virtual_methods.iter().chain(class_data.direct_methods.iter()) {
                            if let Ok(m_name) = ad.get_method_name(m.method_idx) {
                                if m_name == method_name {
                                    return Some((0xFFFFFFFE, c_def, m.method_idx));
                                }
                            }
                        }
                    }
                    
                    let off = ad.header.class_defs_off as usize + (c_def as usize * 32);
                    if let Ok(class_def) = ad.data.pread_with::<crate::dex::ClassDef>(off, LE) {
                        if class_def.superclass_idx == 0xFFFFFFFF { break; }
                        if let Ok(super_class_name) = ad.get_type(class_def.superclass_idx) {
                            current_class_name = super_class_name;
                            continue;
                        }
                    }
                }
                break;
            }
        }
        
        None
    }

    /// Determines whether a type named by `class_type_name` is considered an instance of `target_type_name`.
    ///
    /// The check follows class inheritance and implemented interfaces across the VM's available dex files.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if `class_type_name` is equal to or a subtype/implementor of `target_type_name`, `Ok(false)` otherwise. Errors from dex parsing or I/O are returned as `Err`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Assuming `vm` is an initialized `Vm`:
    /// let is_sub = vm.is_instance_of("Lcom/example/MyClass;", "Ljava/lang/Object;");
    /// assert!(is_sub.unwrap()); // MyClass is an instance of java.lang.Object
    /// ```
    pub fn is_instance_of(&self, class_type_name: &str, target_type_name: &str) -> DexResult<bool> {
        let res = self.is_instance_of_internal(class_type_name, target_type_name);
        res
    }

    /// Determines whether a type named by `class_type_name` is an instance of `target_type_name` according to the VM's class and interface hierarchy.
    ///
    /// This checks equality, the implicit `Ljava/lang/Object;` root, implemented interfaces, and superclass chains across the primary dex, any added extra dexes, and the optional `android_dex`. If a class definition cannot be found for `class_type_name` in any available dex, this function conservatively returns `true`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Constructing a full Vm and Dex is out of scope for this example.
    /// // The call below demonstrates the intended usage.
    /// // let vm = Vm::new(&dex);
    /// // let res = vm.is_instance_of_internal("Lcom/example/MyClass;", "Ljava/lang/Object;").unwrap();
    /// // assert!(res);
    /// ```
    ///
    /// @returns `true` if `class_type_name` is considered an instance of `target_type_name`, `false` otherwise.
    fn is_instance_of_internal(&self, class_type_name: &str, target_type_name: &str) -> DexResult<bool> {
        if class_type_name == target_type_name { return Ok(true); }
        if target_type_name == "Ljava/lang/Object;" { return Ok(true); }
        if class_type_name == "Ljava/lang/Object;" { return Ok(true); }

        let mut current_name = class_type_name.to_string();
        loop {
            let mut found = false;
            let mut superclass_name = None;
            let mut interfaces = Vec::new();

            if let Some(c_idx) = self.dex.find_class(&current_name)? {
                found = true;
                let off = self.dex.header.class_defs_off as usize + (c_idx as usize * 32);
                let class_def: crate::dex::ClassDef = self.dex.data.pread_with(off, LE)?;
                
                if class_def.superclass_idx != 0xFFFFFFFF {
                    superclass_name = Some(self.dex.get_type(class_def.superclass_idx)?);
                }
                
                if class_def.interfaces_off != 0 {
                    let mut i_off = class_def.interfaces_off as usize;
                    let size: u32 = self.dex.data.pread_with(i_off, LE)?; i_off += 4;
                    for _ in 0..size {
                        let itype_idx: u16 = self.dex.data.pread_with(i_off, LE)?; i_off += 2;
                        interfaces.push(self.dex.get_type(itype_idx as u32)?);
                    }
                }
            } else if let Some((extra_idx, c_idx)) = self.extra_dexes.iter().enumerate().find_map(|(i, d)| d.find_class(&current_name).ok().flatten().map(|idx| (i, idx))) {
                found = true;
                let ad = &self.extra_dexes[extra_idx];
                let off = ad.header.class_defs_off as usize + (c_idx as usize * 32);
                let class_def: crate::dex::ClassDef = ad.data.pread_with(off, LE)?;
                if class_def.superclass_idx != 0xFFFFFFFF {
                    superclass_name = Some(ad.get_type(class_def.superclass_idx)?);
                }
                if class_def.interfaces_off != 0 {
                    let mut i_off = class_def.interfaces_off as usize;
                    let size: u32 = ad.data.pread_with(i_off, LE)?; i_off += 4;
                    for _ in 0..size {
                        let itype_idx: u16 = ad.data.pread_with(i_off, LE)?; i_off += 2;
                        interfaces.push(ad.get_type(itype_idx as u32)?);
                    }
                }
            } else if let Some(ref ad) = self.android_dex {
                if let Some(c_idx) = ad.find_class(&current_name)? {
                    found = true;
                    let off = ad.header.class_defs_off as usize + (c_idx as usize * 32);
                    let class_def: crate::dex::ClassDef = ad.data.pread_with(off, LE)?;
                    
                    if class_def.superclass_idx != 0xFFFFFFFF {
                        superclass_name = Some(ad.get_type(class_def.superclass_idx)?);
                    }
                    
                    if class_def.interfaces_off != 0 {
                        let mut i_off = class_def.interfaces_off as usize;
                        let size: u32 = ad.data.pread_with(i_off, LE)?; i_off += 4;
                        for _ in 0..size {
                            let itype_idx: u16 = ad.data.pread_with(i_off, LE)?; i_off += 2;
                            interfaces.push(ad.get_type(itype_idx as u32)?);
                        }
                    }
                }
            }

            if !found {
                return Ok(true);
            }

            for itf in interfaces {
                if itf == target_type_name { return Ok(true); }
                if self.is_instance_of(&itf, target_type_name)? { return Ok(true); }
            }

            if let Some(super_name) = superclass_name {
                if super_name == target_type_name { return Ok(true); }
                current_name = super_name;
            } else {
                break;
            }
        }
        Ok(false)
    }

    pub fn get_resource_string(&self, res_id: u32) -> Option<String> {
        if let Some(ref res) = self.resources {
            if let Some(crate::resources::ResourceValue::String(s)) = res.get(res_id, &self.current_config) {
                return Some(s);
            }
        }
        None
    }

    pub fn alloc(&mut self, obj: Object) -> u32 {
        let mut s = self.state.lock().unwrap();
        if let Some(id) = s.free_list.pop() {
            s.heap[id as usize] = obj;
            return id;
        }
        s.heap.push(obj);
        (s.heap.len() - 1) as u32
    }

    /// Performs a conservative mark-and-sweep garbage collection using the provided
    /// VM registers as root references.
    ///
    /// The collector:
    /// - always treats heap index 0 as live,
    /// - marks any reachable objects reachable from `active_registers` (and any objects
    ///   transitively referenced from their instance fields or array elements),
    /// - replaces unreachable non-null heap entries with `Object::Null` and adds their
    ///   indices to the `free_list`,
    /// - prints a brief summary when any objects are swept.
    ///
    /// # Examples
    ///
    /// ```
    /// // Mark roots in registers 1 and 2 and run a GC.
    /// // `vm` is a mutable VM instance; this shows the intended call site.
    /// vm.conservative_gc(&[1, 2]);
    /// ```
    pub fn conservative_gc(&mut self, active_registers: &[u32]) {
        let mut s = self.state.lock().unwrap();
        let mut marked = vec![false; s.heap.len()];
        marked[0] = true;
        
        let mut worklist = Vec::new();
        for &reg in active_registers {
            let id = reg as usize;
            if id < s.heap.len() && !marked[id] {
                if !matches!(s.heap[id], Object::Null) {
                    marked[id] = true;
                    worklist.push(id);
                }
            }
        }

        while let Some(id) = worklist.pop() {
            let refs = match &s.heap[id] {
                Object::Instance { fields, .. } => fields.values().cloned().collect::<Vec<_>>(),
                Object::Array { data, .. } => data.clone(),
                _ => vec![],
            };

            for r in refs {
                let rid = r as usize;
                if rid < s.heap.len() && !marked[rid] {
                    if !matches!(s.heap[rid], Object::Null) {
                        marked[rid] = true;
                        worklist.push(rid);
                    }
                }
            }
        }

        s.free_list.clear();
        let mut swept = 0;
        for i in 1..s.heap.len() {
            if !marked[i] {
                if !matches!(s.heap[i], Object::Null) {
                    s.heap[i] = Object::Null;
                    s.free_list.push(i as u32);
                    swept += 1;
                }
            }
        }
        if swept > 0 {
            println!("[GC] Swept {} dead objects. Heap size: {}, Free: {}", swept, s.heap.len(), s.free_list.len());
        }
    }

    /// Initializes a class's static fields in the VM and runs its class initializer (`<clinit>`) if present.
    ///
    /// This resolves the class by the given `dex_idx` and `class_idx`, populates dex-scoped static field storage with
    /// any encoded static values (allocating string objects on the heap when needed), marks the class as initialized,
    /// and invokes the class's `<clinit>` method if one exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Assume `dex` is a loaded `Dex` and `vm` is a `Vm` created for that dex.
    /// // This example is illustrative; adjust construction to your test harness.
    /// let mut vm = Vm::new(&dex);
    /// // Initialize class at index 42 in the primary dex (dex_idx = 0).
    /// vm.initialize_class(0, 42).unwrap();
    /// ```
    pub fn initialize_class(&mut self, dex_idx: usize, class_idx: u32) -> DexResult<()> {
        let class_name = {
            let active_dex = if dex_idx == 0xFFFFFFFE {
                self.android_dex.as_ref().ok_or_else(|| DexError::Parse("android.dex not loaded".into()))?
            } else {
                self.get_dex(dex_idx)
            };
            active_dex.get_type(class_idx)?
        };
        {
            let s = self.state.lock().unwrap();
            if s.initialized_classes.contains(&class_name) { return Ok(()); }
        }
        
        enum ResolvedStaticVal {
            Value(u64),
            String(String),
        }

        let (def_idx, static_fields, resolved_values) = {
            let active_dex = if dex_idx == 0xFFFFFFFE {
                self.android_dex.as_ref().ok_or_else(|| DexError::Parse("android.dex not loaded".into()))?
            } else {
                self.get_dex(dex_idx)
            };
            if let Some(def_idx) = active_dex.find_class_def(class_idx)? {
                let class_data = active_dex.get_class_data(def_idx)?;
                let static_values = active_dex.get_static_values(def_idx)?;
                let mut resolved = Vec::new();
                for ev in &static_values {
                    let rv = match ev {
                        EncodedValue::Byte(b) => ResolvedStaticVal::Value(*b as i64 as u64),
                        EncodedValue::Short(s) => ResolvedStaticVal::Value(*s as i64 as u64),
                        EncodedValue::Char(c) => ResolvedStaticVal::Value(*c as u64),
                        EncodedValue::Int(i) => ResolvedStaticVal::Value(*i as i64 as u64),
                        EncodedValue::Long(l) => ResolvedStaticVal::Value(*l as u64),
                        EncodedValue::Float(f) => ResolvedStaticVal::Value(f.to_bits() as u64),
                        EncodedValue::Double(d) => ResolvedStaticVal::Value(d.to_bits()),
                        EncodedValue::String(idx) => {
                            let s = active_dex.get_string(*idx)?;
                            ResolvedStaticVal::String(s)
                        }
                        EncodedValue::Boolean(b) => ResolvedStaticVal::Value(if *b { 1 } else { 0 }),
                        EncodedValue::Null => ResolvedStaticVal::Value(0),
                        _ => ResolvedStaticVal::Value(0),
                    };
                    resolved.push(rv);
                }
                let fields = class_data.static_fields.iter().map(|f| f.field_idx).collect::<Vec<_>>();
                (Some(def_idx), fields, resolved)
            } else {
                (None, Vec::new(), Vec::new())
            }
        };

        if let Some(def_idx) = def_idx {
            let mut fields = HashMap::new();
            for (i, &field_idx) in static_fields.iter().enumerate() {
                let val = if let Some(rv) = resolved_values.get(i) {
                    match rv {
                        ResolvedStaticVal::Value(v) => *v,
                        ResolvedStaticVal::String(s) => {
                            self.alloc(Object::String(s.clone())) as u64
                        }
                    }
                } else {
                    0
                };
                fields.insert(field_idx, val);
            }
            
            {
                let mut s = self.state.lock().unwrap();
                s.static_fields.insert((dex_idx, class_idx), fields);
                s.initialized_classes.insert(class_name);
            }
            
            let clinit_idx = {
                let active_dex = if dex_idx == 0xFFFFFFFE {
                    self.android_dex.as_ref().ok_or_else(|| DexError::Parse("android.dex not loaded".into()))?
                } else {
                    self.get_dex(dex_idx)
                };
                active_dex.find_method_in_class(def_idx, "<clinit>")?
            };
            if let Some(clinit_idx) = clinit_idx {
                self.execute_method(dex_idx, def_idx, clinit_idx, &[])?;
            }
        }
        Ok(())
    }

    /// Execute a method identified by dex, class, and method indexes with the given VM arguments.
    ///
    /// Initializes the declaring class if needed, dispatches to native/JNI handlers when appropriate,
    /// invokes JIT-compiled code when available, or interprets the method body otherwise.
    ///
    /// # Parameters
    ///
    /// - `dex_idx`: Index of the dex to resolve the method from. Use `0xFFFFFFFE` to target the special `android_dex`.
    /// - `def_idx`: Class definition index containing the method, or `0xFFFFFFFF` for external/native methods.
    /// - `method_idx`: Method index within the dex's method table.
    /// - `args`: VM register values passed as the method arguments (heap ids for object references).
    ///
    /// # Returns
    ///
    /// `Ok(Some(u32))` when the method returns a value (object/array/string returns are returned as heap ids; primitive returns are represented as their raw `u32` value), `Ok(None)` for void returns, or an error `Err(DexError::...)` on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assuming `vm` is an initialized `Vm` and indexes are valid:
    /// // let res = vm.execute_method(dex_idx, def_idx, method_idx, &[])?;
    /// // match res {
    /// //     Some(val) => println!("returned value/id: {}", val),
    /// //     None => println!("void return"),
    /// // }
    /// ```
    pub fn execute_method(&mut self, dex_idx: usize, def_idx: u32, method_idx: u32, args: &[u32]) -> DexResult<Option<u32>> {
        let (full_sig, type_idx) = {
            let active_dex = if dex_idx == 0xFFFFFFFE {
                self.android_dex.as_ref().ok_or_else(|| DexError::Parse("android.dex not loaded".into()))?
            } else {
                self.get_dex(dex_idx)
            };
            let full_sig = active_dex.get_method_full_signature(method_idx)?;
            let type_idx = if def_idx != 0xFFFFFFFF {
                active_dex.get_class_type_idx(def_idx)?
            } else {
                0xFFFFFFFF
            };
            (full_sig, type_idx)
        };

        if dex_idx == 0xFFFFFFFE || def_idx == 0xFFFFFFFF {
            if let Some(native) = self.native_methods.get(&full_sig) {
                return native(self, args);
            }
            if let Some(ret_type) = full_sig.find(')').map(|idx| &full_sig[idx + 1..]) {
                if ret_type.starts_with('L') {
                    if ret_type == "Ljava/lang/String;" {
                        let mock_str = self.alloc(Object::String("".into()));
                        println!("[VM] Mocking unimplemented method return: {} -> String reference ({})", full_sig, mock_str);
                        return Ok(Some(mock_str));
                    } else {
                        let mock_obj = self.alloc(Object::Instance {
                            class_desc: ret_type.to_string(),
                            fields: std::collections::HashMap::new(),
                        });
                        println!("[VM] Mocking unimplemented method return: {} -> Mock object of type {} ({})", full_sig, ret_type, mock_obj);
                        return Ok(Some(mock_obj));
                    }
                } else if ret_type.starts_with('[') {
                    let mock_arr = self.alloc(Object::Array {
                        element_type: ret_type.to_string(),
                        data: Vec::new(),
                    });
                    println!("[VM] Mocking unimplemented method return: {} -> Mock array of type {} ({})", full_sig, ret_type, mock_arr);
                    return Ok(Some(mock_arr));
                }
            }
            println!("[VM] Unimplemented method return void/primitive: {} -> 0", full_sig);
            return Ok(None);
        }

        self.initialize_class(dex_idx, type_idx)?;
        
        let (is_native, access_flags, code_off, m_proto_idx) = {
            let active_dex = self.get_dex(dex_idx);
            let class_data = active_dex.get_class_data(def_idx)?;
            let method = class_data.direct_methods.iter().chain(class_data.virtual_methods.iter())
                .find(|m| m.method_idx == method_idx)
                .ok_or_else(|| DexError::Parse(format!("Method not found: {}", full_sig)))?;
            let is_native = (method.access_flags & 0x0100) != 0;
            let is_static = (method.access_flags & 0x0008) != 0;
            
            let proto_idx = if method.code_off == 0 && is_native {
                let off = active_dex.header.method_ids_off as usize + (method_idx as usize * 8);
                let m_id: crate::dex::MethodId = active_dex.data.pread_with(off, LE)?;
                Some((m_id.proto_idx as u32, is_static))
            } else {
                None
            };
            
            (is_native, method.access_flags, method.code_off, proto_idx)
        };

        if code_off == 0 {
            if let Some(native) = self.native_methods.get(&full_sig) {
                return native(self, args);
            }
            if is_native {
                let registered = crate::jni::get_registered_natives();
                if let Some(&fn_ptr) = registered.get(&full_sig) {
                    let (proto_idx, is_static) = m_proto_idx.unwrap();
                    let mut jni_args = Vec::new();
                    
                    let second_arg = if is_static {
                        let class_desc = {
                            let active_dex = self.get_dex(dex_idx);
                            active_dex.get_type(type_idx)?
                        };
                        crate::jni::get_or_create_class_handle(&class_desc)
                    } else {
                        if args.is_empty() {
                            return Err(DexError::Parse("Instance JNI method called with no arguments".into()));
                        }
                        self.vm_to_jni_handle(args[0])
                    };

                    let p_types = self.get_method_parameter_types(dex_idx, proto_idx)?;
                    
                    let start_idx = if is_static { 0 } else { 1 };
                    for (i, &arg_val) in args.iter().enumerate().skip(start_idx) {
                        let p_type = p_types.get(i - start_idx);
                        let is_obj = p_type.map(|t| t.starts_with('L') || t.starts_with('[')).unwrap_or(false);
                        if is_obj {
                            jni_args.push(self.vm_to_jni_handle(arg_val) as u32);
                        } else {
                            jni_args.push(arg_val);
                        }
                    }

                    let env_ptr = crate::jni::get_env_ptr();
                    let raw_res = unsafe {
                        crate::jni::invoke_jni_method(fn_ptr, env_ptr, second_arg, &jni_args)
                    };

                    if let Some(ex_handle) = crate::jni::get_and_clear_pending_exception() {
                        let vm_ex_id = self.jni_to_vm_handle(ex_handle);
                        return Err(DexError::Exception(vm_ex_id));
                    }

                    if full_sig.ends_with(")V") {
                        return Ok(None);
                    } else {
                        let return_type = self.get_method_return_type(dex_idx, proto_idx)?;
                        let returns_obj = return_type.starts_with('L') || return_type.starts_with('[');
                        if returns_obj {
                            if let Some(h) = raw_res {
                                return Ok(Some(self.jni_to_vm_handle(h as usize)));
                            }
                        }
                        return Ok(raw_res);
                    }
                }

                if full_sig.contains("isEduMode") || full_sig.contains("isBrazeEnabled") {
                    return Ok(Some(0));
                }
                println!("[VM] Native method called (not implemented): {}", full_sig);
            }
            return Ok(None);
        }

        let code = {
            let active_dex = self.get_dex(dex_idx);
            active_dex.get_code_item(code_off)?
        };
        let mut registers = vec![0; code.header.registers_size as usize];
        if !args.is_empty() {
            let arg_start = registers.len().saturating_sub(args.len());
            for (i, &arg) in args.iter().enumerate() {
                if arg_start + i < registers.len() {
                    registers[arg_start + i] = arg;
                }
            }
        }

        if self.jit.get_compiled(&full_sig).is_none() {
            self.jit.compile(&full_sig, &code);
        }

        if let Some(jit_func) = self.jit.get_compiled(&full_sig) {
            let res = jit_func(registers.as_mut_ptr());
            return Ok(Some(res));
        }

        let mut pc = 0;
        self.run_loop(dex_idx, &code, &mut pc, &mut registers)
    }

    /// Executes the interpreter loop for a CodeItem until it returns, throws an uncaught exception, or reaches the end of the instruction stream.
    ///
    /// This advances and updates `pc` and `registers` as instructions execute, performs periodic conservative GC when the heap grows past the VM's threshold, and maps thrown exceptions to try-catch handlers in the provided `code` when available.
    ///
    /// # Parameters
    ///
    /// - `dex_idx`: index of the active Dex to use for decoding metadata.
    /// - `code`: the CodeItem whose `insns`, `tries`, and `handlers` drive execution.
    /// - `pc`: mutable program counter (bytecode index) updated as instructions execute; may be set to a handler target on caught exceptions.
    /// - `registers`: mutable register array used for execution and GC root scanning.
    ///
    /// # Returns
    ///
    /// `Ok(Some(u32))` containing a result value heap id when a return value is produced; `Ok(None)` when execution reaches the end without a return; `Err(DexError::Exception(obj_id))` for an uncaught VM exception; or other `DexError` variants for interpreter errors.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assuming `vm`, `dex_idx`, `code`, `pc`, and `registers` are available and properly initialized:
    /// // let mut vm = Vm::new(&dex);
    /// // let mut pc = 0usize;
    /// // let mut registers = vec![0u32; code.header.registers_size as usize];
    /// // let res = vm.run_loop(dex_idx, &code, &mut pc, &mut registers);
    /// ```
    fn run_loop(&mut self, dex_idx: usize, code: &crate::dex::CodeItem, pc: &mut usize, registers: &mut [u32]) -> DexResult<Option<u32>> {
        while *pc < code.insns.len() {
            let insn = code.insns[*pc];
            let opcode = (insn & 0xFF) as u8;
            let old_pc = *pc;
            let active_dex = self.get_dex(dex_idx);
            let m_info = if opcode == 0x6e || opcode == 0x72 || opcode == 0x74 || opcode == 0x78 || opcode == 0x6f || opcode == 0x70 || opcode == 0x71 || opcode == 0x75 || opcode == 0x76 || opcode == 0x77 {
                let m_idx = if *pc + 1 < code.insns.len() { code.insns[*pc + 1] } else { 0 };
                active_dex.get_method_full_signature(m_idx as u32).unwrap_or_default()
            } else {
                "".to_string()
            };
            let res = interpreter::execute_instruction(self, dex_idx, opcode, insn, pc, registers, code);
            if let Err(ref e) = res {
                // println!("[VM TRACE ERROR] pc={} opcode=0x{:02x} insn=0x{:04x} error={:?}", old_pc, opcode, insn, e);
            }
            
            {
                let s = self.state.lock().unwrap();
                if s.heap.len() - s.free_list.len() > self.gc_threshold {
                    drop(s); // Unlock before GC
                    self.conservative_gc(registers);
                    self.gc_threshold += 1024;
                }
            }

            if let Err(e) = res {
                match e {
                    DexError::Return(val) => return Ok(val),
                    DexError::Exception(obj_id) => {
                        self.last_exception = Some(obj_id);
                        let addr = *pc as u32;
                        let mut handled = false;
                        for tri in &code.tries {
                            if addr >= tri.start_addr && addr < (tri.start_addr + tri.insn_count as u32) {
                                if let Some(handler) = code.handlers.get(&tri.handler_off) {
                                    let target_pc = handler.catch_all.or_else(|| handler.handlers.first().map(|(_, a)| *a)).unwrap_or(0);
                                    *pc = target_pc as usize;
                                    handled = true;
                                    break;
                                }
                            }
                        }
                        if !handled { return Err(DexError::Exception(obj_id)); }
                    }
                    _ => {
                        println!("[VM ERROR] Error during instruction interpretation: {:?}, opcode=0x{:02x}, insn=0x{:04x}, pc={}, dex_idx={}", e, opcode, insn, pc, dex_idx);
                        return Err(e);
                    }
                }
            }
        }
        Ok(None)
    }

    /// Resolve the concrete method implementation for an object receiver across the VM's loaded dex files.
    ///
    /// Attempts to determine the actual method implementation to invoke for the given receiver object by:
    /// reading the receiver's class descriptor from the heap, obtaining the method name from the specified dex, and searching for a matching method declaration across the primary dex, any extra dexes, and the optional android dex.
    ///
    /// # Returns
    ///
    /// A tuple `(resolved_dex_idx, class_def_idx, method_idx)` identifying where the implementation was found. Returns the sentinel `(0xFFFFFFFE, 0xFFFFFFFF, method_idx)` when the receiver is not an instance type (e.g., string/array/native) or when resolution fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given a `Vm` instance `vm`, resolve the concrete method for object id 1 in dex 0:
    /// let resolved = vm.resolve_method(0, 1, 5).unwrap();
    /// println!("Resolved method location: {:?}", resolved);
    /// ```
    pub fn resolve_method(&self, dex_idx: usize, obj_id: u32, method_idx: u32) -> DexResult<(usize, u32, u32)> {
        let class_desc = {
            let s = self.state.lock().unwrap();
            match s.heap.get(obj_id as usize) {
                Some(Object::Instance { class_desc, .. }) => class_desc.clone(),
                _ => return Ok((0xFFFFFFFE, 0xFFFFFFFF, method_idx)), // Для String/Array/Native
            }
        };

        let method_name = self.get_dex(dex_idx).get_method_name(method_idx)?;
        if let Some((resolved_dex_idx, resolved_class_def_idx, resolved_method_idx)) = self.resolve_method_by_name(&class_desc, &method_name) {
            Ok((resolved_dex_idx, resolved_class_def_idx, resolved_method_idx))
        } else {
            Ok((0xFFFFFFFE, 0xFFFFFFFF, method_idx))
        }
    }

    /// Retrieve the value of an instance field from the heap.
    ///
    /// Returns the 32-bit value stored in the instance's field identified by `field_idx`.
    /// If the field has not been set, `0` is returned.
    ///
    /// # Parameters
    ///
    /// - `obj_id`: Heap object id of the instance. `0` is treated as `null` and causes an exception.
    /// - `field_idx`: Index of the field within the instance to read.
    ///
    /// # Returns
    ///
    /// `u32` value of the requested field; `0` if the field is absent or unset.
    ///
    /// # Errors
    ///
    /// Returns `DexError::Exception(0)` when `obj_id` is `0`. Returns `DexError::Parse` if the
    /// object id is out of heap bounds or the referenced heap slot is not an instance.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a VM `vm` with an instance at heap id 1 that has field 2 set to 42:
    /// // let v = vm.get_field(1, 2).unwrap();
    /// // assert_eq!(v, 42);
    /// let v = vm.get_field(1, 2)?;
    /// assert_eq!(v, 0); // if the field was not set, the result is 0
    /// ```
    pub fn get_field(&self, obj_id: u32, field_idx: u32) -> DexResult<u32> {
        if obj_id == 0 {
            return Err(DexError::Exception(0));
        }
        let s = self.state.lock().unwrap();
        match s.heap.get(obj_id as usize) {
            Some(Object::Instance { fields, .. }) => Ok(*fields.get(&field_idx).unwrap_or(&0)),
            Some(other) => Err(DexError::Parse(format!("Not an instance: obj_id={}, object={:?}", obj_id, other))),
            None => Err(DexError::Parse(format!("Not an instance: obj_id={} is out of heap bounds", obj_id))),
        }
    }

    /// Sets the value of an instance field for the heap object identified by `obj_id`.
    ///
    /// On success the field is updated and `Ok(())` is returned.
    ///
    /// # Errors
    ///
    /// - Returns `Err(DexError::Exception(0))` if `obj_id == 0` (null reference).
    /// - Returns `Err(DexError::Parse(_))` if the heap entry is not an instance or if `obj_id` is out of heap bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a mutable `vm: Vm` and an existing instance at heap id 1 with a field at index 0:
    /// vm.set_field(1, 0, 42).unwrap();
    /// ```
    pub fn set_field(&mut self, obj_id: u32, field_idx: u32, val: u32) -> DexResult<()> {
        if obj_id == 0 {
            return Err(DexError::Exception(0));
        }
        let mut s = self.state.lock().unwrap();
        match s.heap.get_mut(obj_id as usize) {
            Some(Object::Instance { fields, .. }) => { fields.insert(field_idx, val); Ok(()) },
            Some(other) => Err(DexError::Parse(format!("Not an instance: obj_id={}, object={:?}", obj_id, other))),
            None => Err(DexError::Parse(format!("Not an instance: obj_id={} is out of heap bounds", obj_id))),
        }
    }

    /// Retrieve the stored static field value for a class in a specific dex.
    ///
    /// Looks up the static field map keyed by `(dex_idx, class_idx)` and returns the
    /// stored `u64` value for `field_idx`, or `0` when no value is present.
    ///
    /// # Parameters
    ///
    /// - `dex_idx`: index of the dex (0 = primary dex; >0 refers to entries in `extra_dexes`) containing the class.
    /// - `class_idx`: class definition index within the specified dex.
    /// - `field_idx`: index of the static field within the class.
    ///
    /// # Returns
    ///
    /// The `u64` value of the requested static field, or `0` if the field or class has no stored value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use crate::vm::Vm;
    /// # fn example(vm: &Vm) {
    /// let val = vm.get_static_field(0, 1, 2).unwrap();
    /// assert_eq!(val, 0);
    /// # }
    /// ```
    pub fn get_static_field(&self, dex_idx: usize, class_idx: u32, field_idx: u32) -> DexResult<u64> {
        let s = self.state.lock().unwrap();
        if let Some(fields) = s.static_fields.get(&(dex_idx, class_idx)) {
            Ok(*fields.get(&field_idx).unwrap_or(&0))
        } else {
            Ok(0)
        }
    }

    /// Sets the value of a static field for a class in a specific dex.
    ///
    /// This records `val` into the VM's static fields map under the key `(dex_idx, class_idx)`
    /// and associates it with `field_idx`, creating the class-entry map if necessary.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assume `vm` is a mutable `Vm` instance.
    /// // Store the value `42` for field index `3` of class `10` in dex `0`.
    /// vm.set_static_field(0, 10, 3, 42).unwrap();
    /// ```
    pub fn set_static_field(&mut self, dex_idx: usize, class_idx: u32, field_idx: u32, val: u64) -> DexResult<()> {
        let mut s = self.state.lock().unwrap();
        s.static_fields.entry((dex_idx, class_idx)).or_insert_with(HashMap::new).insert(field_idx, val);
        Ok(())
    }

    pub fn get_array_element(&self, obj_id: u32, idx: usize) -> DexResult<u32> {
        let s = self.state.lock().unwrap();
        match &s.heap[obj_id as usize] {
            Object::Array { data, .. } => Ok(data.get(idx).cloned().unwrap_or(0)),
            _ => Err(DexError::Parse("Not an array".into())),
        }
    }

    pub fn set_array_element(&mut self, obj_id: u32, idx: usize, val: u32) -> DexResult<()> {
        let mut s = self.state.lock().unwrap();
        match &mut s.heap[obj_id as usize] {
            Object::Array { data, .. } => { if idx < data.len() { data[idx] = val; } Ok(()) },
            _ => Err(DexError::Parse("Not an array".into())),
        }
    }

    pub fn get_array_length(&self, obj_id: u32) -> DexResult<usize> {
        let s = self.state.lock().unwrap();
        match &s.heap[obj_id as usize] {
            Object::Array { data, .. } => Ok(data.len()),
            _ => Err(DexError::Parse("Not an array".into())),
        }
    }

    pub fn fill_array_data(&mut self, obj_id: u32, data: &[u32]) -> DexResult<()> {
        let mut s = self.state.lock().unwrap();
        match &mut s.heap[obj_id as usize] {
            Object::Array { data: target, .. } => {
                for (i, &val) in data.iter().enumerate() {
                    if i < target.len() { target[i] = val; }
                }
                Ok(())
            }
            _ => Err(DexError::Parse("Not an array".into())),
        }
    }

    /// Retrieve the string contents of a heap object by its object id.
    ///
    /// Returns `Some(String)` containing the object's string value if the heap object at `obj_id` is a string, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Assuming `vm` is a `Vm` and object id 1 refers to a string on the heap:
    /// let s = vm.get_string_val(1);
    /// assert_eq!(s, Some("hello".to_string()));
    /// ```
    pub fn get_string_val(&self, obj_id: u32) -> Option<String> {
        let s = self.state.lock().unwrap();
        match s.heap.get(obj_id as usize) {
            Some(Object::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Get the class descriptor for an instance object from the heap.
    ///
    /// Returns `Some(String)` containing the object's class descriptor when the heap
    /// entry at `obj_id` is an `Object::Instance`, or `None` for null, arrays,
    /// strings, or out-of-bounds indices.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assume `vm` is a `Vm` with an object at heap id 1 whose class descriptor
    /// // is "Ljava/lang/String;".
    /// let desc = vm.get_object_class_desc(1);
    /// assert_eq!(desc, Some("Ljava/lang/String;".to_string()));
    /// ```
    pub fn get_object_class_desc(&self, obj_id: u32) -> Option<String> {
        let s = self.state.lock().unwrap();
        match s.heap.get(obj_id as usize) {
            Some(Object::Instance { class_desc, .. }) => Some(class_desc.clone()),
            _ => None,
        }
    }

    /// Calls a static method identified by class descriptor and method name, searching the VM's primary and extra dex files.
    ///
    /// Attempts to locate `class_desc` across the VM's dex pool; if the class and the named static method are found, invokes the method with no arguments and returns its result.
    ///
    /// # Returns
    ///
    /// `Ok(Some(heap_id))` when the called method returns an object/heap id, `Ok(None)` when the method has no return value or the class/method was not found. Returns `Err(_)` if method execution fails.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assuming `dex` is a loaded Dex and `vm` is created from it:
    /// // let mut vm = Vm::new(&dex);
    /// let res = vm.call_static_by_name("Lcom/example/MyClass;", "main").unwrap();
    /// match res {
    ///     Some(id) => println!("method returned object id {}", id),
    ///     None => println!("no return value or method not found"),
    /// }
    /// ```
    pub fn call_static_by_name(&mut self, class_desc: &str, method_name: &str) -> DexResult<Option<u32>> {
        if let Some((dex_idx, class_idx)) = self.find_class_in_dexes(class_desc) {
            let active_dex = self.get_dex(dex_idx);
            if let Some(method_idx) = active_dex.find_method_in_class(class_idx, method_name)? {
                println!("[JNI] >>> Executing Java method: {}->{}()", class_desc, method_name);
                return self.execute_method(dex_idx, class_idx, method_idx, &[]);
            }
        }
        Ok(None)
    }

    pub fn monitor_enter(&mut self, obj_id: u32) {
        let monitor = {
            let mut s = self.state.lock().unwrap();
            s.monitors.entry(obj_id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = monitor.lock().unwrap();
    }

    pub fn monitor_exit(&mut self, _obj_id: u32) {
        // Simple stub
    }

    /// Spawns a new OS thread that constructs a child VM sharing this VM's shared state and invokes the specified static method.
    ///
    /// The child VM receives a new thread id and a cloned view of this VM's dex tables and shared state, and then calls the static method identified by `class_desc` and `method_name`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Given a `dex: Dex` already loaded:
    /// let mut vm = Vm::new(&dex);
    /// vm.spawn_thread("Lcom/example/MyClass;", "main");
    /// // The call runs asynchronously on a new thread.
    /// ```
    pub fn spawn_thread(&mut self, class_desc: &str, method_name: &str) {
        let cd = class_desc.to_string();
        let mn = method_name.to_string();
        let dex_ptr_val = self.dex as *const Dex<'a> as usize;
        let state = self.state.clone();
        let thread_id = self.thread_id + 1;
        let android_dex_ptr = match self.android_dex {
            Some(ref ad) => ad as *const Dex<'a> as usize,
            None => 0,
        };
        let extra_dexes_static: Vec<crate::dex::Dex<'static>> = unsafe {
            std::mem::transmute(self.extra_dexes.clone())
        };
        
        std::thread::spawn(move || {
            let dex: &Dex = unsafe { &*(dex_ptr_val as *const Dex) };
            let mut child_vm = Vm::new_thread(dex, state, thread_id);
            child_vm.extra_dexes = unsafe { std::mem::transmute(extra_dexes_static) };
            if android_dex_ptr != 0 {
                let ad: &Dex = unsafe { &*(android_dex_ptr as *const Dex) };
                child_vm.android_dex = Some(*ad);
            }
            println!("[THREAD-{}] Started!", thread_id);
            let _ = child_vm.call_static_by_name(&cd, &mn);
            println!("[THREAD-{}] Finished!", thread_id);
        });
    }

    /// Prints a human-readable representation of the heap object identified by `obj_id` to stdout.
    ///
    /// Behavior:
    /// - If the id refers to the VM null object, prints `[STDOUT]: null`.
    /// - If it refers to a string object, prints `[STDOUT]: <string>`.
    /// - If it refers to an instance object, prints `[STDOUT]: (Object of type <class_desc>)`.
    /// - For other object kinds, prints `[STDOUT]: (Object ID <obj_id>)`.
    /// - If `obj_id` is out of the heap bounds, prints the numeric id as a signed 32-bit integer.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `vm` is a properly initialized `Vm`
    /// vm.native_println(0);
    /// ```
    pub fn native_println(&self, obj_id: u32) {
        let s = self.state.lock().unwrap();
        if let Some(obj) = s.heap.get(obj_id as usize) {
            match obj {
                Object::Null => println!("[STDOUT]: null"),
                Object::String(s) => println!("[STDOUT]: {}", s),
                Object::Instance { class_desc, .. } => {
                    println!("[STDOUT]: (Object of type {})", class_desc);
                }
                _ => println!("[STDOUT]: (Object ID {})", obj_id),
            }
        } else {
            println!("[STDOUT]: {}", obj_id as i32);
        }
    }

    /// Retrieves the list of parameter type descriptors for the given method prototype in the specified dex.
    ///
    /// Returns an empty vector when the prototype declares no parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// // assuming `vm` is an initialized `Vm` and the dex contains a valid proto at index 1
    /// let types = vm.get_method_parameter_types(0, 1).unwrap();
    /// // `types` will be a Vec of type descriptors like "Ljava/lang/String;" or "[I"
    /// assert!(types.is_empty() || types.iter().all(|s| !s.is_empty()));
    /// ```
    pub fn get_method_parameter_types(&self, dex_idx: usize, proto_idx: u32) -> DexResult<Vec<String>> {
        let active_dex = self.get_dex(dex_idx);
        let off = active_dex.header.proto_ids_off as usize + (proto_idx as usize * 12);
        let proto: crate::dex::ProtoId = active_dex.data.pread_with(off, LE)?;
        let mut types = Vec::new();
        if proto.parameters_off != 0 {
            let mut p_off = proto.parameters_off as usize;
            let size: u32 = active_dex.data.pread_with(p_off, LE)?; p_off += 4;
            for _ in 0..size {
                let type_idx: u16 = active_dex.data.pread_with(p_off, LE)?; p_off += 2;
                types.push(active_dex.get_type(type_idx as u32)?);
            }
        }
        Ok(types)
    }

    /// Get the method prototype's return type descriptor from the specified dex.
    ///
    /// Looks up the `ProtoId` at `proto_idx` in the dex identified by `dex_idx` and
    /// returns its return type descriptor as a string (for example, `Ljava/lang/String;`).
    ///
    /// # Errors
    ///
    /// Returns an error if reading the proto entry or resolving the type descriptor fails.
    pub fn get_method_return_type(&self, dex_idx: usize, proto_idx: u32) -> DexResult<String> {
        let active_dex = self.get_dex(dex_idx);
        let off = active_dex.header.proto_ids_off as usize + (proto_idx as usize * 12);
        let proto: crate::dex::ProtoId = active_dex.data.pread_with(off, LE)?;
        active_dex.get_type(proto.return_type_idx)
    }

    /// Convert a VM heap object id into a JNI handle suitable for use with the JNI layer.
    ///
    /// Returns `0` for the VM null reference (heap id `0`). For VM string objects this
    /// returns a JNI string handle; for all other heap objects this returns a VM object
    /// JNI handle created or looked up by the JNI helper.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Given a `vm: Vm` instance and a heap id:
    /// let null_handle = vm.vm_to_jni_handle(0);
    /// assert_eq!(null_handle, 0);
    ///
    /// // For a string object (assumes `str_obj_id` is a heap id pointing to an
    /// // Object::String in the VM heap):
    /// let str_handle = vm.vm_to_jni_handle(str_obj_id);
    /// // `str_handle` is a JNI string handle (non-zero).
    ///
    /// // For other VM objects:
    /// let obj_handle = vm.vm_to_jni_handle(obj_id);
    /// // `obj_handle` is a JNI VM object handle (non-zero).
    /// ```
    fn vm_to_jni_handle(&self, heap_id: u32) -> usize {
        if heap_id == 0 { return 0; }
        let s = self.state.lock().unwrap();
        match s.heap.get(heap_id as usize) {
            Some(Object::String(str_val)) => {
                crate::jni::get_or_create_string_handle(str_val.clone())
            }
            _ => {
                crate::jni::get_or_create_vm_object_handle(heap_id)
            }
        }
    }

    /// Convert a JNI handle into a VM object and return its heap id.
    ///
    /// Maps JNI handle values produced by the JNI layer into VM representations:
    /// - `Null` -> `0`
    /// - `VmObject(hid)` -> returns the existing heap id `hid`
    /// - `String(s)` -> allocates a VM `Object::String` and returns its new heap id
    /// - `Class(desc)` -> allocates an empty `Object::Instance` with `class_desc` and returns its heap id
    ///
    /// # Returns
    ///
    /// The heap id (`u32`) of the VM object corresponding to `jni_handle`. `0` represents the VM null reference.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Example (non-running): convert a JNI handle into a VM heap id.
    /// // `jni_handle` is obtained from the JNI layer; `vm` is an existing Vm instance.
    /// // let heap_id = vm.jni_to_vm_handle(jni_handle);
    /// ```
    fn jni_to_vm_handle(&mut self, jni_handle: usize) -> u32 {
        match crate::jni::resolve_jni_handle(jni_handle) {
            crate::jni::JniToVmMapResult::Null => 0,
            crate::jni::JniToVmMapResult::VmObject(hid) => hid,
            crate::jni::JniToVmMapResult::String(s) => {
                self.alloc(Object::String(s))
            }
            crate::jni::JniToVmMapResult::Class(desc) => {
                self.alloc(Object::Instance {
                    class_desc: desc,
                    fields: HashMap::new(),
                })
            }
        }
    }
}
