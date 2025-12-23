use crate::json_reader::{parse_file, parse_text_content};
use crate::tree_model::{build_tree_from_parse_result, TreeNode};
use serde::Serialize;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{self, UnwindSafe};
use std::path::Path;

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum FfiResponse {
    Ok { root: TreeNode },
    Error { message: String },
}

fn cstr_to_str<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    unsafe {
        if ptr.is_null() {
            return Err("Received null pointer".to_string());
        }
        CStr::from_ptr(ptr)
            .to_str()
            .map_err(|_| "Invalid UTF-8 data".to_string())
    }
}

fn stringify(result: Result<TreeNode, String>) -> *mut c_char {
    let response = match result {
        Ok(root) => FfiResponse::Ok { root },
        Err(message) => FfiResponse::Error { message },
    };

    let json = serde_json::to_string(&response).unwrap_or_else(|_| {
        "{\"status\":\"error\",\"message\":\"serialization failed\"}".to_string()
    });

    CString::new(json)
        .unwrap_or_else(|_| {
            CString::new("{\"status\":\"error\",\"message\":\"ffi error\"}").unwrap()
        })
        .into_raw()
}

fn run_ffi_operation<F>(operation: F) -> *mut c_char
where
    F: FnOnce() -> Result<TreeNode, String> + UnwindSafe,
{
    let result = match panic::catch_unwind(operation) {
        Ok(inner) => inner,
        Err(_) => Err("Rust panic while processing request".to_string()),
    };

    stringify(result)
}

fn build_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("File")
        .to_string()
}

fn parse_file_at(path: &Path) -> Result<TreeNode, String> {
    let display_name = build_display_name(path);
    let parsed = parse_file(path).map_err(|err| err.to_string())?;
    Ok(build_tree_from_parse_result(parsed, &display_name))
}

#[no_mangle]
pub extern "C" fn slopjson_parse_file(path_ptr: *const c_char) -> *mut c_char {
    run_ffi_operation(|| {
        let path_str = cstr_to_str(path_ptr)?;
        let path = Path::new(path_str);
        parse_file_at(path)
    })
}

#[no_mangle]
pub extern "C" fn slopjson_parse_text(
    content_ptr: *const c_char,
    name_ptr: *const c_char,
) -> *mut c_char {
    run_ffi_operation(|| {
        let content = cstr_to_str(content_ptr)?.to_string();
        let name = cstr_to_str(name_ptr).unwrap_or("Content");
        let parsed = parse_text_content(&content).map_err(|err| err.to_string())?;
        Ok(build_tree_from_parse_result(parsed, name))
    })
}

#[no_mangle]
pub extern "C" fn slopjson_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    unsafe {
        drop(CString::from_raw(ptr));
    }
}
