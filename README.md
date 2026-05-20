# Linux Runtime for Android (LRT)

A lightweight Dalvik Virtual Machine implementation in Rust for executing Android applications (.apk and .dex files) directly on Linux.

## Features
- **Multi-DEX Support:** Automatically parses and loads classes from secondary DEX files.
- **Auto-Mocking Architecture:** Intelligently bypasses complex Android system dependencies by dynamically mocking objects and their fields when uninitialized, preventing `NullPointerExceptions`.
- **Flexible Type Checking:** Robustly evaluates `check-cast` and `instance-of` even on unknown or mocked class hierarchies.
- **Integrated JNI Mocking:** Intercepts unimplemented native system calls and smoothly replaces them with sensible defaults to keep the VM executing.
- **Dalvik Opcode Support:** Implements full core Dalvik interpreter instruction set for method dispatch, virtual calls, fields access, and calculations.

## Building and Usage
```bash
cargo build --release
cargo install --path .
```

To run an APK:
```bash
linux-android-runtime /path/to/app.apk
```

## Architecture
The application is structured into the following key components:
- `dex/` - DEX file parser and structures.
- `vm/` - Virtual machine runtime, GC, class resolution, and the execution engine.
- `vm/interpreter.rs` - The Dalvik byte-code interpreter.
- `vm/native.rs` - JNI bridge and implementations of fundamental system methods.

## Current State
The interpreter is able to successfully parse complex Android Applications, perform Multi-DEX loading, and dive deep into `Activity.onCreate` execution paths while auto-mocking Android SDK frameworks on the fly.
