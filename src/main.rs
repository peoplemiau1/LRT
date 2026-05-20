use std::env;
use std::fs::File;
use std::io::Read;
use linux_android_runtime::dex::Dex;
use linux_android_runtime::vm::Vm;
use linux_android_runtime::axml::{parse_manifest, AxmlElement};
use linux_android_runtime::resources::parse_resources;
use scroll::{Pread, LE};


/// Finds the Android main activity type descriptor from an AXML element tree.
///
/// Searches recursively for an `<activity>` element that contains an `<intent-filter>` with
/// an `<action>` whose `android:name` equals `"android.intent.action.MAIN"`. When found,
/// returns the activity class name normalized to a DEX-style type descriptor:
/// - If the attribute value starts with `.`, the leading `.` is removed.
/// - If the value does not start with `L`, `.` are replaced with `/`, the result is prefixed with `L` and suffixed with `;`.
///
/// # Returns
///
/// `Some(String)` containing the normalized DEX type descriptor of the main activity when found, `None` otherwise.
///
/// # Examples
///
/// ```
/// // Construct a minimal element representing:
/// // <activity android:name="com.example.Main">
/// //   <intent-filter>
/// //     <action android:name="android.intent.action.MAIN"/>
/// //   </intent-filter>
/// // </activity>
/// let element = AxmlElement {
///     name: "activity".into(),
///     attributes: vec![AxmlAttribute { name: "name".into(), id: 0, value: "com.example.Main".into() }],
///     children: vec![AxmlElement {
///         name: "intent-filter".into(),
///         attributes: vec![],
///         children: vec![AxmlElement {
///             name: "action".into(),
///             attributes: vec![AxmlAttribute { name: "name".into(), id: 0, value: "android.intent.action.MAIN".into() }],
///             children: vec![],
///         }],
///     }],
/// };
///
/// let found = find_main_activity(&element);
/// assert_eq!(found, Some("Lcom/example/Main;".to_string()));
/// ```
fn find_main_activity(element: &AxmlElement) -> Option<String> {
    if element.name == "activity" {
        let mut is_main = false;
        for child in &element.children {
            if child.name == "intent-filter" {
                for filter_child in &child.children {
                    if filter_child.name == "action" {
                        if filter_child.attributes.iter().any(|a| a.value == "android.intent.action.MAIN") {
                            is_main = true;
                        }
                    }
                }
            }
        }
        if is_main {
            return element.attributes.iter()
                .find(|a| a.name == "name" || a.id == 0x01010003)
                .map(|a| {
                    let mut name = a.value.clone();
                    if name.starts_with('.') { name = name.replacen('.', "", 1); }
                    if !name.starts_with('L') { name = format!("L{};", name.replace('.', "/")); }
                    name
                });
        }
    }
    for child in &element.children {
        if let Some(name) = find_main_activity(child) { return Some(name); }
    }
    None
}

fn print_element(element: &AxmlElement, depth: usize) {
    let indent = "  ".repeat(depth);
    print!("{}<{}", indent, element.name);
    for attr in &element.attributes {
        print!(" {}=\"{}\" (id: 0x{:08x})", attr.name, attr.value, attr.id);
    }
    if element.children.is_empty() {
        println!("/>");
    } else {
        println!(">");
        for child in &element.children {
            print_element(child, depth + 1);
        }
        println!("{}</{}>", indent, element.name);
    }
}

/// Program entry point that loads a DEX or APK, prepares a VM (including secondary DEX and resources),
/// and locates and executes a specified class method (defaulting to the app's main activity and `onCreate`).
///
/// This function:
/// - Accepts a path to a `.dex` or `.apk` file as the first CLI argument.
/// - Optionally accepts a DEX type descriptor class name and a method name as second and third arguments.
/// - When given an APK, extracts `classes.dex`, any subsequent `classesN.dex` files, `resources.arsc`, and `AndroidManifest.xml`.
/// - Constructs a VM, registers extra DEX files and optional `android.dex` classpath, locates the target method by walking superclasses,
///   builds mock arguments for reference/array parameters, and invokes the method.
/// - Prints available methods when the target method is not found.
///
/// # Examples
///
/// ```no_run
/// // Run the program against an APK, specifying a class and method:
/// // linux-android-runtime path/to/app.apk "Lcom/example/tinyart/MainActivity;" "onCreate"
/// ```
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: linux-android-runtime <dex_or_apk_file> [class_name] [method_name]");
        return;
    }

    let file_path = &args[1];
    let mut file = File::open(file_path).expect("Failed to open file");
    let mut buffer = Vec::new();
    let mut resources = None;
    let mut main_activity = None;

    let mut extra_dex_buffers = Vec::new();

    if file_path.ends_with(".apk") {
        let mut archive = zip::ZipArchive::new(file).expect("Failed to open APK as ZIP");
        
        let mut i = 2;
        loop {
            let dex_name = format!("classes{}.dex", i);
            if let Ok(mut extra_file) = archive.by_name(&dex_name) {
                let mut buf = Vec::new();
                extra_file.read_to_end(&mut buf).ok();
                extra_dex_buffers.push(buf);
                i += 1;
            } else {
                break;
            }
        }
        
        
        if let Ok(mut res_file) = archive.by_name("resources.arsc") {
            let mut res_buf = Vec::new();
            res_file.read_to_end(&mut res_buf).ok();
            if let Ok(table) = parse_resources(&res_buf) {
                resources = Some(table);
                println!("[VM] Loaded resources: {} strings", resources.as_ref().unwrap().strings.len());
            }
        }

        
        if let Ok(mut manifest_file) = archive.by_name("AndroidManifest.xml") {
            let mut manifest_buf = Vec::new();
            manifest_file.read_to_end(&mut manifest_buf).ok();
            match parse_manifest(&manifest_buf) {
                Ok(root) => {
                    main_activity = find_main_activity(&root);
                    if main_activity.is_none() {
                        println!("[VM DEBUG] find_main_activity returned None. Manifest structure:");
                        print_element(&root, 0);
                    }
                }
                Err(e) => {
                    println!("[VM DEBUG] parse_manifest failed: {:?}", e);
                }
            }
        }

        let mut dex_file = archive.by_name("classes.dex").expect("classes.dex not found in APK");
        dex_file.read_to_end(&mut buffer).expect("Failed to read classes.dex from APK");
    } else {
        file.read_to_end(&mut buffer).expect("Failed to read file");
    }

    let dex = Dex::new(&buffer).expect("Failed to parse DEX");
    
    let class_name = if args.len() > 2 { 
        args[2].clone() 
    } else if let Some(ref ma) = main_activity {
        ma.clone()
    } else { 
        "Lcom/example/tinyart/MainActivity;".to_string() 
    };
    
    let method_name = if args.len() > 3 { &args[3] } else { "onCreate" };

    println!("Launching Activity: {}", class_name);
    
    let mut vm = Vm::new(&dex);
    if let Some(res) = resources { vm.set_resources(res); }

    for buf in extra_dex_buffers {
        let leaked_buf: &'static [u8] = Box::leak(buf.into_boxed_slice());
        if let Ok(ad) = Dex::new(leaked_buf) {
            println!("[VM] Loaded secondary DEX ({} classes)", ad.header.class_defs_size);
            vm.add_extra_dex(ad);
        }
    }

    let android_dex_paths = ["android.dex", "/home/asadula/android.dex"];
    let mut android_dex_data = None;
    for path in &android_dex_paths {
        if let Ok(mut f) = File::open(path) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() {
                android_dex_data = Some(buf);
                break;
            }
        }
    }

    if let Some(buf) = android_dex_data {
        let leaked_buf: &'static [u8] = Box::leak(buf.into_boxed_slice());
        if let Ok(ad) = Dex::new(leaked_buf) {
            println!("[VM] Loaded classpath android.dex ({} classes)", ad.header.class_defs_size);
            vm.set_android_dex(ad);
        }
    }


    if let Some(c_idx) = dex.find_class(&class_name).expect("Class not found") {
        let mut found_method = None;
        let mut curr_class_idx = c_idx;
        loop {
            if let Some(m_idx) = dex.find_method_in_class(curr_class_idx, method_name).expect("Method error") {
                found_method = Some((curr_class_idx, m_idx));
                break;
            }
            
            let off = dex.header.class_defs_off as usize + (curr_class_idx as usize * 32);
            if let Ok(class_def) = dex.data.pread_with::<linux_android_runtime::dex::ClassDef>(off, LE) {
                if class_def.superclass_idx == 0xFFFFFFFF { break; }
                if let Ok(super_class_name) = dex.get_type(class_def.superclass_idx) {
                    if let Some(super_idx) = dex.find_class(&super_class_name).unwrap_or(None) {
                        curr_class_idx = super_idx;
                        continue;
                    }
                }
            }
            break;
        }

        if let Some((target_class_idx, m_idx)) = found_method {
            let class_data = dex.get_class_data(target_class_idx).expect("Failed to get class data");
            let method = class_data.direct_methods.iter().chain(class_data.virtual_methods.iter())
                .find(|m| m.method_idx == m_idx)
                .expect("Method not found");
            let is_static = (method.access_flags & 0x0008) != 0;

            let off = dex.header.method_ids_off as usize + (m_idx as usize * 8);
            let m_id: linux_android_runtime::dex::MethodId = dex.data.pread_with(off, scroll::LE).expect("Failed to read MethodId");
            let proto_idx = m_id.proto_idx as u32;
            let param_types = vm.get_method_parameter_types(0, proto_idx).unwrap_or_default();

            let mut run_args = Vec::new();
            if !is_static {
                let this_ref = vm.alloc(linux_android_runtime::vm::Object::Instance {
                    class_desc: class_name.clone(),
                    fields: std::collections::HashMap::new(),
                });
                run_args.push(this_ref);
            }

            for p_type in param_types {
                if p_type.starts_with('L') {
                    let mock_val = vm.alloc(linux_android_runtime::vm::Object::Instance {
                        class_desc: p_type.clone(),
                        fields: std::collections::HashMap::new(),
                    });
                    run_args.push(mock_val);
                } else if p_type.starts_with('[') {
                    let mock_val = vm.alloc(linux_android_runtime::vm::Object::Array {
                        element_type: p_type.clone(),
                        data: Vec::new(),
                    });
                    run_args.push(mock_val);
                } else {
                    run_args.push(0);
                }
            }
            
            vm.execute_method(0, target_class_idx, m_idx, &run_args).expect("Execution failed");
        } else {
            println!("Error: Method {} not found in class {} or its superclasses in primary DEX", method_name, class_name);
            if let Ok(class_data) = dex.get_class_data(c_idx) {
                println!("Available methods in class {}:", class_name);
                for m in class_data.direct_methods.iter().chain(class_data.virtual_methods.iter()) {
                    if let Ok(m_name) = dex.get_method_name(m.method_idx) {
                        println!("  - {}", m_name);
                    }
                }
            }
        }
    } else {
        println!("Error: Class {} not found", class_name);
    }
    
    println!("Execution finished successfully.");
}
