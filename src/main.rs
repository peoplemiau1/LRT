use std::env;
use std::fs::File;
use std::io::Read;
use linux_android_runtime::dex::Dex;
use linux_android_runtime::vm::Vm;
use linux_android_runtime::axml::{parse_manifest, AxmlElement};
use linux_android_runtime::resources::parse_resources;


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

    if file_path.ends_with(".apk") {
        let mut archive = zip::ZipArchive::new(file).expect("Failed to open APK as ZIP");
        
        
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
        if let Some(m_idx) = dex.find_method_in_class(c_idx, method_name).expect("Method error") {
            vm.execute_method(c_idx, m_idx, &[0]).expect("Execution failed");
        } else {
            println!("Error: Method {} not found in class {}", method_name, class_name);
        }
    } else {
        println!("Error: Class {} not found", class_name);
    }
    
    println!("Execution finished successfully.");
}
