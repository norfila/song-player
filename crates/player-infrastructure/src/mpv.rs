//! A small dynamically loaded libmpv client. The native SDK is not required at
//! build time. ABI definitions follow mpv/include/mpv/client.h (client API 2).
use crate::InfraError;
use libloading::Library;
use serde::{Deserialize, Serialize};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    path::{Path, PathBuf},
    ptr,
};

const MPV_FORMAT_NODE: c_int = 6;
const MPV_FORMAT_NODE_ARRAY: c_int = 7;
const MPV_FORMAT_NODE_MAP: c_int = 8;
const MPV_FORMAT_STRING: c_int = 1;
const MPV_EVENT_NONE: c_int = 0;
const MPV_EVENT_END_FILE: c_int = 7;
const MPV_EVENT_FILE_LOADED: c_int = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    TrackChanged {
        index: usize,
    },
    /// Every file produces an END_FILE, even before another playlist entry.
    Ended {
        reason: String,
        error: Option<String>,
    },
}

#[repr(C)]
struct MpvEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct MpvEventEndFile {
    reason: c_int,
    error: c_int,
    playlist_entry_id: i64,
    playlist_insert_id: i64,
    playlist_insert_num_entries: c_int,
}

#[repr(C)]
union MpvNodeValue {
    string: *mut c_char,
    flag: c_int,
    int64: i64,
    double: f64,
    list: *mut MpvNodeList,
    byte_array: *mut c_void,
}

#[repr(C)]
struct MpvNode {
    value: MpvNodeValue,
    format: c_int,
}

#[repr(C)]
struct MpvNodeList {
    num: c_int,
    values: *mut MpvNode,
    keys: *mut *mut c_char,
}

type Create = unsafe extern "C" fn() -> *mut c_void;
type Initialize = unsafe extern "C" fn(*mut c_void) -> c_int;
type Destroy = unsafe extern "C" fn(*mut c_void);
type SetString = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type GetString = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;
type GetProperty = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type Command = unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int;
type WaitEvent = unsafe extern "C" fn(*mut c_void, f64) -> *const MpvEvent;
type ErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type Free = unsafe extern "C" fn(*mut c_void);
type FreeNode = unsafe extern "C" fn(*mut MpvNode);
type ApiVersion = unsafe extern "C" fn() -> std::ffi::c_ulong;

struct Api {
    create: Create,
    initialize: Initialize,
    destroy: Destroy,
    set_option_string: SetString,
    set_property_string: SetString,
    get_property_string: GetString,
    get_property: GetProperty,
    command: Command,
    wait_event: WaitEvent,
    error_string: ErrorString,
    free: Free,
    free_node_contents: FreeNode,
    client_api_version: ApiVersion,
    // Function pointers remain valid until the handle has been destroyed.
    _library: Library,
}

impl Api {
    unsafe fn load(path: &Path) -> Result<Self, InfraError> {
        // SAFETY: Loading native code is limited to the bundled library, an
        // explicit caller-selected path, or a system library search name.
        let library = unsafe { Library::new(path) }
            .map_err(|error| InfraError::new("engine_load", error.to_string()))?;
        macro_rules! symbol {
            ($name:literal, $signature:ty) => {
                *unsafe { library.get::<$signature>(concat!($name, "\0").as_bytes()) }
                    .map_err(|error| InfraError::new("engine_api", error.to_string()))?
            };
        }
        Ok(Self {
            create: symbol!("mpv_create", Create),
            initialize: symbol!("mpv_initialize", Initialize),
            destroy: symbol!("mpv_terminate_destroy", Destroy),
            set_option_string: symbol!("mpv_set_option_string", SetString),
            set_property_string: symbol!("mpv_set_property_string", SetString),
            get_property_string: symbol!("mpv_get_property_string", GetString),
            get_property: symbol!("mpv_get_property", GetProperty),
            command: symbol!("mpv_command", Command),
            wait_event: symbol!("mpv_wait_event", WaitEvent),
            error_string: symbol!("mpv_error_string", ErrorString),
            free: symbol!("mpv_free", Free),
            free_node_contents: symbol!("mpv_free_node_contents", FreeNode),
            client_api_version: symbol!("mpv_client_api_version", ApiVersion),
            _library: library,
        })
    }

    fn error(&self, code: c_int) -> InfraError {
        // SAFETY: mpv_error_string returns static, zero-terminated storage.
        let message = unsafe { CStr::from_ptr((self.error_string)(code)) }.to_string_lossy();
        InfraError::new("engine_command", format!("{message} ({code})"))
    }
}

pub struct MpvEngine {
    handle: *mut c_void,
    api: Api,
}

// SAFETY: libmpv client API permits calls from arbitrary threads. The client
// handle has one owner and methods that consume events require &mut self.
// Application access is serialized by its playback coordinator/Mutex.
unsafe impl Send for MpvEngine {}

impl MpvEngine {
    /// Search paths may identify individual libraries or containing folders.
    /// The desktop shell passes its resource directory and development path.
    pub fn new(search_paths: &[PathBuf]) -> Result<Self, InfraError> {
        let names: &[&str] = if cfg!(target_os = "windows") {
            &["libmpv-2.dll", "mpv-2.dll", "mpv.dll"]
        } else if cfg!(target_os = "macos") {
            &["libmpv.2.dylib", "libmpv.dylib"]
        } else {
            &["libmpv.so.2", "libmpv.so"]
        };
        let mut candidates = Vec::new();
        for search_path in search_paths {
            if search_path.is_dir() {
                for name in names {
                    candidates.push(search_path.join(name));
                    candidates.push(search_path.join("mpv").join(name));
                    candidates.push(search_path.join("resources").join("mpv").join(name));
                }
            } else {
                candidates.push(search_path.clone());
            }
        }
        if let Some(explicit) = std::env::var_os("SONG_PLAYER_MPV_PATH") {
            let explicit = PathBuf::from(explicit);
            if explicit.is_dir() {
                for name in names {
                    candidates.push(explicit.join(name));
                }
            } else {
                candidates.push(explicit);
            }
        }
        if let Ok(executable) = std::env::current_exe() {
            if let Some(parent) = executable.parent() {
                for name in names {
                    candidates.push(parent.join(name));
                    candidates.push(parent.join("resources/mpv").join(name));
                }
            }
        }
        // Convenience for development; production resources are passed above.
        let development =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/mpv");
        for name in names {
            candidates.push(development.join(name));
        }
        candidates.extend(names.iter().map(PathBuf::from));
        let mut diagnostics = Vec::new();
        for candidate in candidates {
            // SAFETY: The library is held for the full client handle lifetime.
            match unsafe { Api::load(&candidate) } {
                Ok(api) => return Self::initialize(api),
                Err(error) => {
                    diagnostics.push(format!("{}: {}", candidate.display(), error.detail))
                }
            }
        }
        Err(InfraError::new("engine_missing", diagnostics.join("\n")))
    }

    fn initialize(api: Api) -> Result<Self, InfraError> {
        // SAFETY: Function signatures match the libmpv client API header.
        let version = unsafe { (api.client_api_version)() };
        if version >> 16 != 2 {
            return Err(InfraError::new(
                "engine_api",
                format!(
                    "Expected libmpv client API 2, found {}.{}",
                    version >> 16,
                    version & 0xffff
                ),
            ));
        }
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(InfraError::new(
                "engine_initialize",
                "mpv_create returned null",
            ));
        }
        let engine = Self { handle, api };
        for (name, value) in [
            ("config", "no"),
            ("terminal", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("osc", "no"),
            ("vid", "no"),
            ("vo", "null"),
            ("idle", "yes"),
            ("keep-open", "no"),
            ("ytdl", "no"),
            ("gapless-audio", "weak"),
        ] {
            let name = cstring(name)?;
            let value = cstring(value)?;
            let status = unsafe {
                (engine.api.set_option_string)(engine.handle, name.as_ptr(), value.as_ptr())
            };
            if status < 0 {
                return Err(engine.api.error(status));
            }
        }
        let status = unsafe { (engine.api.initialize)(engine.handle) };
        if status < 0 {
            return Err(engine.api.error(status));
        }
        Ok(engine)
    }

    pub fn command(&mut self, args: &[&str]) -> Result<(), InfraError> {
        if args.is_empty() {
            return Err(InfraError::new("engine_command", "Empty command"));
        }
        let args: Vec<CString> = args
            .iter()
            .map(|arg| cstring(arg))
            .collect::<Result<_, _>>()?;
        let mut pointers: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(ptr::null());
        // SAFETY: Every argument lives until the synchronous call returns; the
        // argument array has the null terminator required by mpv_command.
        let status = unsafe { (self.api.command)(self.handle, pointers.as_ptr()) };
        if status < 0 {
            Err(self.api.error(status))
        } else {
            Ok(())
        }
    }

    pub fn set_property(&mut self, name: &str, value: &str) -> Result<(), InfraError> {
        let name = cstring(name)?;
        let value = cstring(value)?;
        // SAFETY: Both C strings live across the call and handle is valid.
        let status =
            unsafe { (self.api.set_property_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        if status < 0 {
            Err(self.api.error(status))
        } else {
            Ok(())
        }
    }

    pub fn get_property(&mut self, name: &str) -> Option<String> {
        self.property_string(name)
    }

    fn property_string(&self, name: &str) -> Option<String> {
        let name = cstring(name).ok()?;
        // SAFETY: libmpv allocates the returned C string; mpv_free is used on
        // that same pointer after copying, and unavailable properties are null.
        let value = unsafe { (self.api.get_property_string)(self.handle, name.as_ptr()) };
        if value.is_null() {
            return None;
        }
        let owned = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.api.free)(value.cast()) };
        Some(owned)
    }

    pub fn version(&self) -> String {
        self.property_string("mpv-version")
            .unwrap_or_else(|| "libmpv".into())
    }

    /// Nonblocking; irrelevant events are consumed so the queue cannot fill.
    pub fn poll_event(&mut self) -> Option<EngineEvent> {
        for _ in 0..128 {
            // SAFETY: An event is valid until the next mpv_wait_event call. Any
            // strings/data used below are copied before this loop advances.
            let event = unsafe { (self.api.wait_event)(self.handle, 0.0).as_ref() }?;
            match event.event_id {
                MPV_EVENT_NONE => return None,
                MPV_EVENT_FILE_LOADED => {
                    if let Some(index) = self
                        .property_string("playlist-pos")
                        .and_then(|value| value.parse::<usize>().ok())
                    {
                        return Some(EngineEvent::TrackChanged { index });
                    }
                }
                MPV_EVENT_END_FILE if !event.data.is_null() => {
                    let end = unsafe { &*event.data.cast::<MpvEventEndFile>() };
                    let reason = match end.reason {
                        0 => "eof",
                        2 => "stop",
                        3 => "quit",
                        4 => "error",
                        5 => "redirect",
                        _ => "unknown",
                    };
                    let error = if end.error < 0 {
                        Some(self.api.error(end.error).detail)
                    } else {
                        None
                    };
                    return Some(EngineEvent::Ended {
                        reason: reason.into(),
                        error,
                    });
                }
                _ => {}
            }
        }
        None
    }

    pub fn list_devices(&mut self) -> Vec<AudioDevice> {
        let name = c"audio-device-list";
        let mut node = MpvNode {
            value: MpvNodeValue { int64: 0 },
            format: 0,
        };
        // SAFETY: node is an out parameter initialized by libmpv and released
        // with mpv_free_node_contents after extracting owned device strings.
        let status = unsafe {
            (self.api.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_NODE,
                (&mut node as *mut MpvNode).cast(),
            )
        };
        if status < 0 {
            return Vec::new();
        }
        let mut devices = Vec::new();
        if node.format == MPV_FORMAT_NODE_ARRAY {
            if let Some(list) = unsafe { node.value.list.as_ref() } {
                if list.num > 0 && !list.values.is_null() {
                    for item in
                        unsafe { std::slice::from_raw_parts(list.values, list.num as usize) }
                    {
                        let id = unsafe { node_map_string(item, "name") };
                        let description = unsafe { node_map_string(item, "description") };
                        if let Some(id) = id {
                            devices.push(AudioDevice {
                                name: description.unwrap_or_else(|| id.clone()),
                                id,
                            });
                        }
                    }
                }
            }
        }
        unsafe { (self.api.free_node_contents)(&mut node) };
        devices
    }
}

impl Drop for MpvEngine {
    fn drop(&mut self) {
        // SAFETY: This is the sole owner of the handle; api/library are alive.
        unsafe { (self.api.destroy)(self.handle) };
        self.handle = ptr::null_mut();
    }
}

fn cstring(value: &str) -> Result<CString, InfraError> {
    CString::new(value).map_err(|_| InfraError::new("invalid_argument", "Embedded NUL character"))
}

unsafe fn node_map_string(node: &MpvNode, key: &str) -> Option<String> {
    if node.format != MPV_FORMAT_NODE_MAP {
        return None;
    }
    let list = unsafe { node.value.list.as_ref() }?;
    if list.num <= 0 || list.values.is_null() || list.keys.is_null() {
        return None;
    }
    for index in 0..list.num as usize {
        let name = unsafe { *list.keys.add(index) };
        if name.is_null() {
            continue;
        }
        if unsafe { CStr::from_ptr(name) }.to_bytes() == key.as_bytes() {
            let value = unsafe { &*list.values.add(index) };
            if value.format == MPV_FORMAT_STRING {
                let value = unsafe { value.value.string };
                if !value.is_null() {
                    return Some(
                        unsafe { CStr::from_ptr(value) }
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    #[ignore = "requires the native libmpv runtime; uses null output, not speakers"]
    fn native_engine_loads_unicode_audio_seeks_and_reports_playlist_transitions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Über 🎵.wav");
        let pcm_length = 8_000_u32 * 2 * 2;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + pcm_length).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16_u32.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(8_000_u32.to_le_bytes());
        wav.extend(16_000_u32.to_le_bytes());
        wav.extend(2_u16.to_le_bytes());
        wav.extend(16_u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(pcm_length.to_le_bytes());
        wav.resize(44 + pcm_length as usize, 0);
        std::fs::write(&path, wav).unwrap();
        let path = path.to_string_lossy().into_owned();
        let mut engine = MpvEngine::new(&[]).unwrap();
        eprintln!("Native engine: {}", engine.version());
        let devices = engine.list_devices();
        assert!(devices.iter().any(|device| device.id == "auto"));
        engine.set_property("ao", "null").unwrap();
        engine.set_property("pause", "yes").unwrap();
        engine.command(&["loadfile", &path, "replace"]).unwrap();
        engine.command(&["loadfile", &path, "append"]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut loaded = false;
        while Instant::now() < deadline {
            if let Some(EngineEvent::TrackChanged { index: 0 }) = engine.poll_event() {
                loaded = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(loaded, "First playlist entry did not load");
        assert!(
            engine
                .get_property("duration")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                > 1.9
        );
        engine.command(&["seek", "0.75", "absolute+exact"]).unwrap();
        engine.set_property("pause", "no").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut second = false;
        while Instant::now() < deadline {
            if let Some(event) = engine.poll_event() {
                match event {
                    EngineEvent::TrackChanged { index: 1 } => {
                        second = true;
                        break;
                    }
                    EngineEvent::Ended {
                        error: Some(error), ..
                    } => panic!("Native playback failed: {error}"),
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(second, "Second playlist entry did not start automatically");
        engine.command(&["stop"]).unwrap();
        assert!(engine.get_property("does-not-exist").is_none());
        assert!(engine.command(&["loadfile", "invalid\0name"]).is_err());
        let missing = directory
            .path()
            .join("missing.wav")
            .to_string_lossy()
            .into_owned();
        engine.command(&["loadfile", &missing, "replace"]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut reported_error = false;
        while Instant::now() < deadline {
            if let Some(EngineEvent::Ended {
                reason,
                error: Some(_),
            }) = engine.poll_event()
            {
                if reason == "error" {
                    reported_error = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(reported_error, "Missing file should produce an error event");
        engine.command(&["loadfile", &path, "replace"]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut recovered = false;
        while Instant::now() < deadline {
            if let Some(EngineEvent::TrackChanged { index: 0 }) = engine.poll_event() {
                recovered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(recovered, "Engine should recover after an invalid file");
        engine.command(&["stop"]).unwrap();
    }
}
