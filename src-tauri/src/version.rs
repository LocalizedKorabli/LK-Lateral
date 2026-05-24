use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

#[link(name = "version")]
extern "system" {
    fn GetFileVersionInfoSizeW(lptstr: *const u16, lpdw_handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(
        lptstr: *const u16,
        dw_handle: u32,
        dw_len: u32,
        lp_data: *mut u8,
    ) -> i32;
    fn VerQueryValueW(
        p_block: *const u8,
        lp_sub_block: *const u16,
        lplp_buffer: *mut *mut u8,
        pu_len: *mut u32,
    ) -> i32;
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn wide_to_string(ptr: *const u8, len: u32) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr as *const u16, (len as usize).saturating_sub(1)) };
    String::from_utf16_lossy(slice)
}

pub fn get_product_version(exe_path: &Path) -> Option<String> {
    let path_str = exe_path.to_string_lossy();
    let wide_path = to_wide(&path_str);

    let mut handle: u32 = 0;
    let info_size = unsafe { GetFileVersionInfoSizeW(wide_path.as_ptr(), &mut handle) };

    if info_size == 0 {
        return None;
    }

    let mut buffer: Vec<u8> = vec![0u8; info_size as usize];
    let result = unsafe {
        GetFileVersionInfoW(
            wide_path.as_ptr(),
            handle,
            info_size,
            buffer.as_mut_ptr(),
        )
    };

    if result == 0 {
        return None;
    }

    let translation_query = to_wide(r"\VarFileInfo\Translation");
    let mut trans_ptr: *mut u8 = std::ptr::null_mut();
    let mut trans_len: u32 = 0;

    let trans_result = unsafe {
        VerQueryValueW(
            buffer.as_ptr(),
            translation_query.as_ptr(),
            &mut trans_ptr,
            &mut trans_len,
        )
    };

    if trans_result == 0 || trans_ptr.is_null() || trans_len < 4 {
        return None;
    }

    let lang_id = unsafe { std::ptr::read_unaligned(trans_ptr as *const u16) };
    let code_page = unsafe { std::ptr::read_unaligned(trans_ptr.add(2) as *const u16) };

    let string_query = format!(
        r"\StringFileInfo\{:04x}{:04x}\ProductVersion",
        lang_id, code_page
    );
    let wide_string_query = to_wide(&string_query);

    let mut value_ptr: *mut u8 = std::ptr::null_mut();
    let mut value_len: u32 = 0;

    let value_result = unsafe {
        VerQueryValueW(
            buffer.as_ptr(),
            wide_string_query.as_ptr(),
            &mut value_ptr,
            &mut value_len,
        )
    };

    if value_result == 0 || value_ptr.is_null() || value_len == 0 {
        return None;
    }

    let version = wide_to_string(value_ptr, value_len);
    let version = version.trim_end_matches('\0').trim().to_string();

    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

pub fn get_exe_version(path: &str) -> Option<String> {
    let path = Path::new(path);
    if path.is_dir() {
        let exe_path = find_main_exe(path)?;
        get_product_version(&exe_path)
    } else if path.is_file() {
        get_product_version(path)
    } else {
        None
    }
}

fn find_main_exe(dir: &Path) -> Option<std::path::PathBuf> {
    let candidates = ["lgc.exe", "Korabli.Most.exe", "Korabli.Client.exe"];
    for name in &candidates {
        let exe = dir.join(name);
        if exe.exists() {
            return Some(exe);
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "exe").unwrap_or(false) {
                return Some(p);
            }
        }
    }
    None
}
