/// Android JNI bridge — static storage and exported native functions.
///
/// ## Startup flow
///
/// `init()` must be called once from `run_mobile()` after `Arc::new(app_state)`.
/// `NativeBridge.registerShizukuDispatcher()` is called from `MainActivity.onCreate` after
/// `super.onCreate()` to store the [`JavaVM`] and `ShizukuDispatcher` class reference.
///
/// ## Dispatch flow
///
/// ```text
/// Kotlin BlePlugin.onTapBytes()
///   → NativeBridge.processTapBytes(address, bytes)    [JNI call, any thread]
///     → tap_event_tx.blocking_send(TapEventMsg)       [feeds existing pump]
///       → run_android_pump resolves actions
///         → dispatch_via_shizuku(actionsJson)         [JNI call from pump]
///           → ShizukuDispatcher.dispatch(actionsJson) [Kotlin object, non-external]
///             → IInputService.injectKey / injectMotion (shell uid via Shizuku)
///               → InputManager.injectInputEvent()
/// ```
///
/// All items are Android-only (`cfg(target_os = "android")`).
#[cfg(target_os = "android")]
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "android")]
use jni::{
    EnvUnowned, Outcome,
    objects::{Global, JByteArray, JClass, JString},
    sys::jstring,
    JavaVM,
};

#[cfg(target_os = "android")]
use crate::state::AppState;

// ── Module-level statics ──────────────────────────────────────────────────────

#[cfg(target_os = "android")]
static APP_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

#[cfg(target_os = "android")]
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

/// [`JavaVM`] stored by `registerShizukuDispatcher`, used by the pump to attach a thread
/// and call `ShizukuDispatcher.dispatch(actionsJson)`.
#[cfg(target_os = "android")]
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

/// Global reference to the `ShizukuDispatcher` Kotlin object class.
///
/// Stored by `registerShizukuDispatcher`. Used by the pump to call
/// `ShizukuDispatcher.dispatch(actionsJson)` as a JNI static method, routing
/// resolved actions to `InputUserService` running as shell uid.
#[cfg(target_os = "android")]
static SHIZUKU_DISPATCHER_CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();

// ── Init (called from lib.rs) ─────────────────────────────────────────────────

/// Populate the module-level statics with the shared app state and handle.
///
/// Call once from `run_mobile()` immediately after `Arc::new(app_state)`.
/// Subsequent calls are silently ignored (the `OnceLock` is already set).
#[cfg(target_os = "android")]
pub fn init(state: Arc<AppState>, handle: tauri::AppHandle) {
    let _ = APP_STATE.set(state);
    let _ = APP_HANDLE.set(handle);
}

// ── Accessors (used by android_pump) ─────────────────────────────────────────

/// Returns the stored [`AppState`], or `None` if [`init`] has not been called.
#[cfg(target_os = "android")]
pub fn app_state() -> Option<&'static Arc<AppState>> {
    APP_STATE.get()
}

/// Returns the stored [`tauri::AppHandle`], or `None` if [`init`] has not been called.
#[cfg(target_os = "android")]
pub fn app_handle() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
}

/// Returns the stored [`JavaVM`], or `None` if `registerShizukuDispatcher` has not been called.
#[cfg(target_os = "android")]
pub fn java_vm() -> Option<&'static JavaVM> {
    JAVA_VM.get()
}

/// Returns the stored global ref to the `ShizukuDispatcher` class,
/// or `None` if `registerShizukuDispatcher` has not been called.
#[cfg(target_os = "android")]
pub fn shizuku_dispatcher_class() -> Option<&'static Global<JClass<'static>>> {
    SHIZUKU_DISPATCHER_CLASS.get()
}

// ── JNI-exported functions ────────────────────────────────────────────────────

/// Called by `NativeBridge.processTapBytes()` on each BLE characteristic notification.
///
/// Forwards the raw bytes into the Android pump via `blocking_send`, bypassing the
/// WebView. Resolved actions are dispatched by the pump to `ShizukuDispatcher` via
/// `NativeBridge.registerShizukuDispatcher()`.
///
/// Returns the string `"ok"` on success or `"err"` on failure (channel closed,
/// state not initialised). Errors are logged to logcat via stderr.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mapxr_app_NativeBridge_processTapBytes<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    address: JString<'local>,
    bytes: JByteArray<'local>,
) -> jstring {
    // Step 1: extract address and bytes from JNI objects.
    let extracted = match env
        .with_env(|env| -> jni::errors::Result<(String, Vec<u8>)> {
            let addr: String = address.mutf8_chars(env)?.into();
            let raw_bytes: Vec<u8> = env.convert_byte_array(&bytes)?;
            Ok((addr, raw_bytes))
        })
        .into_outcome()
    {
        Outcome::Ok(v) => Some(v),
        Outcome::Err(e) => {
            eprintln!("mapxr/jni: processTapBytes JNI extraction error: {e}");
            None
        }
        Outcome::Panic(_) => {
            eprintln!("mapxr/jni: processTapBytes panic during JNI extraction");
            None
        }
    };

    // Step 2: forward to the Rust pump (no JNI needed).
    let status = match extracted {
        Some((addr, raw_bytes)) => match APP_STATE.get() {
            Some(state) => {
                match state
                    .tap_event_tx
                    .blocking_send(crate::android_pump::TapEventMsg {
                        address: addr,
                        bytes: raw_bytes,
                    }) {
                    Ok(()) => "ok",
                    Err(_) => {
                        eprintln!("mapxr/jni: tap_event_tx channel closed");
                        "err"
                    }
                }
            }
            None => {
                eprintln!("mapxr/jni: AppState not initialised — init() not yet called");
                "err"
            }
        },
        None => "err",
    };

    // Step 3: wrap the status string as a JNI string for the return value.
    // Use into_outcome() to avoid propagating Java exceptions back to the Kotlin caller;
    // a null return is safe — Kotlin handles it gracefully.
    match env
        .with_env(|env| -> jni::errors::Result<JString<'local>> { env.new_string(status) })
        .into_outcome()
    {
        Outcome::Ok(j_ret) => j_ret.into_raw(),
        Outcome::Err(e) => {
            eprintln!("mapxr/jni: processTapBytes could not create return string: {e}");
            std::ptr::null_mut()
        }
        Outcome::Panic(_) => {
            eprintln!("mapxr/jni: processTapBytes panic creating return string");
            std::ptr::null_mut()
        }
    }
}

/// Called by `NativeBridge.registerShizukuDispatcher()` from `MainActivity.onCreate` after
/// `super.onCreate()`.
///
/// Stores the current [`JavaVM`] and a global reference to the `ShizukuDispatcher` Kotlin
/// object so the pump can call `ShizukuDispatcher.dispatch(actionsJson)` from any thread
/// without going through the WebView.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_mapxr_app_NativeBridge_registerShizukuDispatcher<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    // Use into_outcome() so that any JNI failure is logged and silently swallowed
    // rather than propagated as a Java exception back to MainActivity.onCreate().
    // A crash there would prevent the WebView from loading.
    match env
        .with_env(|env| -> jni::errors::Result<()> {
            let vm = env.get_java_vm()?;
            let dispatcher_class =
                env.find_class(jni::jni_str!("com/mapxr/app/ShizukuDispatcher"))?;
            let dispatcher_global = env.new_global_ref(dispatcher_class)?;
            let _ = JAVA_VM.set(vm);
            let _ = SHIZUKU_DISPATCHER_CLASS.set(dispatcher_global);
            eprintln!("mapxr/jni: Shizuku dispatch initialised");
            Ok(())
        })
        .into_outcome()
    {
        Outcome::Ok(()) => {}
        Outcome::Err(e) => {
            eprintln!("mapxr/jni: registerShizukuDispatcher error: {e}");
        }
        Outcome::Panic(_) => {
            eprintln!("mapxr/jni: registerShizukuDispatcher panic");
        }
    }
}
