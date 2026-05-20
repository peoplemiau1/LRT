# LRT: Linux Android Runtime

LRT (Linux Android Runtime) is a lightweight, high-performance Dalvik Virtual Machine (VM) and JIT compiler designed to run Android Dalvik Executables (`.dex` and `.apk` files) natively on Linux systems, featuring complete JNI bridging, resources resolution, and full classpath class-hierarchy traversal.

---

## Key Features

- **High-Performance Dalvik VM**:
  - Full bytecode execution including math, remainders, type conversions, packed/sparse switches, and array operations.
  - Native exception propagation (`try-catch` exception handling) with robust exception table mappings.
  - Multi-threaded execution support.

- **Dynamic JIT Compiler**:
  - Direct x86_64 machine code generation via dual-pass register-allocated compilation.
  - 100% native register allocation for performance-sensitive loops and methods (e.g., Fibonacci, mathematical loops).

- **Complete JNI Subsystem**:
  - Loading ELF native libraries (`.so`) using standard dynamic linking.
  - Full execution of `JNI_OnLoad` initialization.
  - Robust JNI handle management (`VmObject(u32)`) representing JVM/DEX objects.
  - Java-to-C and C-to-Java call redirection.

- **Classpath Integration & Type Resolution**:
  - Resolves complex class inheritance and recursive interface checks across both the primary DEX file and `android.dex` classpath library.
  - Seamless virtual dispatch resolution for classes that inherit from Android SDK framework classes (e.g., `MainActivity` inheriting from `android.app.Activity`).

- **Resource & Manifest Parsing**:
  - Binary XML parser for `AndroidManifest.xml` to detect the entrypoint component (`MainActivity`).
  - Resource table parser (`resources.arsc`) to map resource IDs (`0x7f010001`) to localized string values.

---

## Architecture Overview

```
               +----------------------------------+
               |         Command Line (CLI)       |
               +----------------------------------+
                                |
                                v
               +----------------------------------+
               |        AXML & ARSC Parser        |
               +----------------------------------+
                                | (Extracts classes.dex, Activity, Resources)
                                v
               +----------------------------------+
               |        Dex Class Loader          | <------+
               +----------------------------------+        |
                                |                          | Loads android.dex
                                v                          | (Classpath SDK)
+--------------------------------------------------------+ |
|                        Dalvik VM                       |-+
+--------------------------------------------------------+
|  +--------------------+        +--------------------+  |
|  |     Interpreter    | <----> |    JIT Compiler    |  |
|  +--------------------+        +--------------------+  |
|                                                        |
|  +--------------------+        +--------------------+  |
|  |   JNI Subsystem    | <----> |   Heap / GC        |  |
|  +--------------------+        +--------------------+  |
+--------------------------------------------------------+
                                |
                                v
               +----------------------------------+
               |       Dynamic Linker (.so)       |
               +----------------------------------+
```

---

## Getting Started

### Prerequisites

To build and run LRT, you need:
- Rust toolchain (Rust 2024 edition)
- Java Runtime Environment (for running `D8`/`R8` compilation if you compile custom SDK resources)
- Linux environment (x86_64 target)

### Building the Project

Compile the LRT binary in release mode:
```bash
cargo build --release
```

### Running Dalvik / APK Executables

To run an APK or DEX file:
```bash
./target/release/linux-android-runtime <path/to/file.apk_or.dex> [class_name] [method_name]
```

Example (defaults to `MainActivity` and `onCreate`):
```bash
./target/release/linux-android-runtime test_build/test.apk
```

---

## Development & Verification

Execute the built-in testing suite to verify VM operations, native JNI loading, and typechecks:
```bash
cargo run -- test_build/test.apk
```
