//! Platform desktop integration. Defaults are queried only; Windows retains
//! control of association changes through its own Default Apps settings page.
use player_application::{AppError, AppResult};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DefaultAppStatus {
    pub supported: bool,
    pub missing_extensions: Vec<String>,
}

pub(super) fn default_app_status() -> AppResult<DefaultAppStatus> {
    #[cfg(windows)]
    return windows::default_app_status();
    #[cfg(not(windows))]
    Ok(DefaultAppStatus {
        supported: false,
        missing_extensions: vec![],
    })
}

pub(super) fn open_default_apps_settings() -> AppResult<()> {
    #[cfg(windows)]
    return windows::open_default_apps_settings();
    #[cfg(not(windows))]
    Err(AppError::new(
        "unsupported_platform",
        "Default Apps settings integration is available on Windows",
    ))
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{ffi::c_void, path::Path, ptr};

    #[link(name = "shlwapi")]
    extern "system" {
        fn AssocQueryStringW(
            flags: u32,
            kind: u32,
            association: *const u16,
            extra: *const u16,
            output: *mut u16,
            size: *mut u32,
        ) -> i32;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            window: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut c_void;
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    fn associated_executable(extension: &str) -> Option<String> {
        const ASSOCSTR_EXECUTABLE: u32 = 2;
        let extension = wide(extension);
        let mut size = 0;
        // SAFETY: Stable NUL-terminated input. Null output requests the exact
        // buffer size, including the terminator, per AssocQueryStringW docs.
        unsafe {
            AssocQueryStringW(
                0,
                ASSOCSTR_EXECUTABLE,
                extension.as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                &mut size,
            );
        }
        if size == 0 || size > 32_768 {
            return None;
        }
        let mut output = vec![0u16; size as usize];
        // SAFETY: Output owns `size` UTF-16 elements and remains live until the
        // synchronous call returns. Flags 0 include current user choices.
        let result = unsafe {
            AssocQueryStringW(
                0,
                ASSOCSTR_EXECUTABLE,
                extension.as_ptr(),
                ptr::null(),
                output.as_mut_ptr(),
                &mut size,
            )
        };
        if result != 0 {
            return None;
        }
        let end = output.iter().position(|c| *c == 0).unwrap_or(output.len());
        String::from_utf16(&output[..end]).ok()
    }

    pub(super) fn default_app_status() -> AppResult<DefaultAppStatus> {
        // Keep the capabilities/reminder aligned with the shipped associations.
        // Parsing embedded config avoids a second format list drifting later.
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .map_err(|error| AppError::new("internal_error", error))?;
        let extensions = config["bundle"]["fileAssociations"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|association| association["ext"].as_array().into_iter().flatten())
            .filter_map(|extension| extension.as_str())
            .map(|extension| format!(".{extension}"));
        let current_exe =
            std::env::current_exe().map_err(|error| AppError::new("internal_error", error))?;
        let app_name = current_exe
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("sonora.exe");
        let missing_extensions = extensions
            .filter(|extension| {
                !associated_executable(extension).is_some_and(|executable| {
                    // Install locations can change between releases or a portable
                    // launch. Match this app's executable name rather than requiring
                    // that the running build is the registered installed copy.
                    Path::new(&executable)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.eq_ignore_ascii_case(app_name))
                })
            })
            .collect();
        Ok(DefaultAppStatus {
            supported: true,
            missing_extensions,
        })
    }

    pub(super) fn open_default_apps_settings() -> AppResult<()> {
        // Per-user installer registers "Sonora" in HKCU RegisteredApplications.
        // Older Windows versions gracefully show the general Default Apps page.
        let uri = wide("ms-settings:defaultapps?registeredAppUser=Sonora");
        let operation = wide("open");
        // SAFETY: Only a fixed Microsoft Settings URI is passed, never user
        // input or a command string. No associations are written here.
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                uri.as_ptr(),
                ptr::null(),
                ptr::null(),
                1,
            )
        } as isize;
        if result <= 32 {
            Err(AppError::new(
                "desktop_error",
                format!("Could not open Windows Default Apps settings ({result})"),
            ))
        } else {
            Ok(())
        }
    }

    #[test]
    fn querying_default_apps_is_read_only_and_includes_declared_formats() {
        let status = default_app_status().unwrap();
        assert!(status.supported);
        assert!(status
            .missing_extensions
            .iter()
            .all(|extension| extension.starts_with('.') && !extension.contains(' ')));
        assert!(status.missing_extensions.len() <= 19);
    }
}
