//! 修正在縮放比例不同的兩個螢幕之間拖動視窗時，視窗大小與位置來回亂跳。
//!
//! winit 在縮放改變時會依新比例重設視窗大小；視窗變大或變小後，大半面積可能又回到
//! 原本的螢幕，Windows 再改回原本的縮放，就這樣來回切換。這裡在拖動途中縮放改變時，
//! 讓 winit 照常更新縮放，再把視窗放回改變前的實體位置與大小，畫面內容依新縮放重排。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetWindowRect, SetWindowLongPtrW, SetWindowPos, GWLP_WNDPROC, SWP_NOACTIVATE,
    SWP_NOZORDER, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WNDPROC,
};

static OLD_PROC: AtomicIsize = AtomicIsize::new(0);
static DRAGGING: AtomicBool = AtomicBool::new(false);

pub fn install(cc: &eframe::CreationContext<'_>) {
    let Ok(handle) = cc.window_handle() else { return };
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return };
    let hwnd = h.hwnd.get() as HWND;
    unsafe {
        let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT as usize as isize);
        OLD_PROC.store(old, Ordering::SeqCst);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let old: WNDPROC = std::mem::transmute(OLD_PROC.load(Ordering::SeqCst));
    match msg {
        WM_ENTERSIZEMOVE => DRAGGING.store(true, Ordering::SeqCst),
        WM_EXITSIZEMOVE => DRAGGING.store(false, Ordering::SeqCst),
        WM_DPICHANGED if DRAGGING.load(Ordering::SeqCst) => {
            let mut before: RECT = std::mem::zeroed();
            let ok = GetWindowRect(hwnd, &mut before) != 0;
            let r = CallWindowProcW(old, hwnd, msg, wparam, lparam);
            if ok {
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    before.left,
                    before.top,
                    before.right - before.left,
                    before.bottom - before.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            return r;
        }
        _ => {}
    }
    CallWindowProcW(old, hwnd, msg, wparam, lparam)
}
