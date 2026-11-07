//! Start with Windows: one value under HKCU\...\Run pointing at this exe. 🚀
use crate::sys::wide;
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    },
};

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "NoDesk";

/// The Run value that starts this exe: its quoted path, NUL-terminated.
fn command() -> Result<Vec<u16>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(wide(&format!("\"{}\"", exe.display())))
}

/// True when the Run value starts this very exe; after the exe moves it reads false,
/// so ticking the menu item again repairs the entry.
pub fn is_on() -> bool {
    let (k, n) = (wide(KEY), wide(NAME));
    let mut buf = [0u16; 1024];
    let mut bytes = (buf.len() * 2) as u32;
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            k.as_ptr(),
            n.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    code == ERROR_SUCCESS && command().is_ok_and(|c| buf[..bytes as usize / 2] == c[..])
}

/// Adds or removes the Run value; Err holds the reason for the error box.
pub fn set(on: bool) -> Result<(), String> {
    let (k, n) = (wide(KEY), wide(NAME));
    let code = if on {
        let cmd = command()?;
        let bytes = (cmd.len() * 2) as u32;
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                k.as_ptr(),
                n.as_ptr(),
                REG_SZ,
                cmd.as_ptr().cast(),
                bytes,
            )
        }
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, k.as_ptr(), n.as_ptr()) }
    };
    match code {
        ERROR_SUCCESS => Ok(()),
        c => Err(std::io::Error::from_raw_os_error(c as i32).to_string()),
    }
}
