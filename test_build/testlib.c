#include <stdio.h>
#include <string.h>

// Minimal JNI types
typedef void* jobject;
typedef jobject jclass;
typedef jobject jstring;
typedef void* jmethodID;
typedef void* jfieldID;
typedef int jint;
typedef unsigned char jboolean;

struct JNINativeInterface_ { void* funcs[230]; };
typedef const struct JNINativeInterface_* JNIEnv;

struct JNIInvokeInterface_ { void* funcs[8]; };
typedef const struct JNIInvokeInterface_* JavaVM;

// Function pointer types
typedef jclass (*FindClass_t)(JNIEnv*, const char*);
typedef jmethodID (*GetStaticMethodID_t)(JNIEnv*, jclass, const char*, const char*);
typedef void (*CallStaticVoidMethod_t)(JNIEnv*, jclass, jmethodID, ...);
typedef jint (*GetEnv_t)(JavaVM*, void**, jint);
typedef jstring (*NewStringUTF_t)(JNIEnv*, const char*);
typedef const char* (*GetStringUTFChars_t)(JNIEnv*, jstring, jboolean*);
typedef void (*ReleaseStringUTFChars_t)(JNIEnv*, jstring, const char*);
typedef jint (*GetStringUTFLength_t)(JNIEnv*, jstring);
typedef jboolean (*ExceptionCheck_t)(JNIEnv*);
typedef void (*ExceptionClear_t)(JNIEnv*);

// JNINativeMethod struct (must match Android ABI)
typedef struct {
    const char* name;
    const char* signature;
    void*       fnPtr;
} JNINativeMethod;

typedef jint (*RegisterNatives_t)(JNIEnv*, jclass, const JNINativeMethod*, jint);

// A native function that will be registered and callable from Java
void native_greet(JNIEnv* env, jclass clazz) {
    printf("[C/C++] native_greet() called! This was registered via RegisterNatives!\n");
}

jint JNI_OnLoad(JavaVM* vm, void* reserved) {
    printf("[C/C++] ==========================================\n");
    printf("[C/C++] JNI_OnLoad: Initializing native library...\n");

    // 1. Get JNIEnv
    JNIEnv* env = NULL;
    GetEnv_t getEnv = (GetEnv_t)((*vm)->funcs[6]);
    jint result = getEnv(vm, (void**)&env, 0x00010006);
    if (result != 0 || env == NULL) {
        printf("[C/C++] FATAL: Failed to get JNIEnv!\n");
        return -1;
    }
    printf("[C/C++] Got JNIEnv OK!\n");

    // 2. Test String operations
    NewStringUTF_t newStr = (NewStringUTF_t)((*env)->funcs[167]);
    GetStringUTFChars_t getChars = (GetStringUTFChars_t)((*env)->funcs[169]);
    GetStringUTFLength_t getLen = (GetStringUTFLength_t)((*env)->funcs[168]);
    ReleaseStringUTFChars_t relChars = (ReleaseStringUTFChars_t)((*env)->funcs[170]);

    jstring jstr = newStr(env, "Hello from C++ to Java!");
    jint len = getLen(env, jstr);
    const char* chars = getChars(env, jstr, NULL);
    printf("[C/C++] Created JNI String: \"%s\" (length=%d)\n", chars, len);
    relChars(env, jstr, chars);

    // 3. Test Exception checking
    ExceptionCheck_t exCheck = (ExceptionCheck_t)((*env)->funcs[228]);
    jboolean hasEx = exCheck(env);
    printf("[C/C++] Exception pending: %s\n", hasEx ? "YES" : "NO");

    // 4. Find class and call Java method
    FindClass_t findClass = (FindClass_t)((*env)->funcs[6]);
    jclass cls = findClass(env, "com/example/tinyart/GrandTest");

    GetStaticMethodID_t getMethod = (GetStaticMethodID_t)((*env)->funcs[113]);
    jmethodID mid = getMethod(env, cls, "helloFromC", "()V");

    printf("[C/C++] >>> Calling Java from C! <<<\n");
    CallStaticVoidMethod_t callMethod = (CallStaticVoidMethod_t)((*env)->funcs[141]);
    callMethod(env, cls, mid);

    // 5. RegisterNatives test
    JNINativeMethod nativeMethods[] = {
        {"nativeGreet", "()V", (void*)native_greet}
    };
    RegisterNatives_t regNatives = (RegisterNatives_t)((*env)->funcs[215]);
    jint regResult = regNatives(env, cls, nativeMethods, 1);
    printf("[C/C++] RegisterNatives result: %d (0=OK)\n", regResult);

    printf("[C/C++] Native init complete!\n");
    printf("[C/C++] ==========================================\n");
    return 0x00010006;
}
