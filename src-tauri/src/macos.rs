//! macOS 专用的窗口层级处理。
//!
//! 卡片要求「始终在最前端」，包括别的应用处于全屏（macOS 的 native
//! fullscreen，会独占一个 Space）的时候。
//!
//! `alwaysOnTop` 只把窗口抬到 `NSFloatingWindowLevel`，在全屏应用之上
//! 并不生效——全屏窗口在自己的 Space 里，普通窗口根本不在那个 Space。
//! 要让窗口跟着全屏 space 一起显示，需要给 NSWindow 加上
//! `NSWindowCollectionBehaviorFullScreenAuxiliary`。
//!
//! tao 的 `set_visible_on_all_workspaces` 只设了 `CanJoinAllSpaces`，
//! Tauri 也没有透出这个 collection behavior，所以这里直接操作 NSWindow。
//!
//! AppKit 要求这些调用在主线程，所以统一走 `run_on_main_thread`。

use objc2::rc::Retained;
use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
use tauri::{AppHandle, Manager, WebviewWindow};

/// 给卡片窗口打上「可以出现在全屏应用之上」的标记。
///
/// 幂等：重复调用不会叠加出奇怪的组合。
#[cfg(target_os = "macos")]
pub fn enable_fullscreen_overlay(app: &AppHandle, window: &WebviewWindow) {
    let handle = app.clone();
    // `run_on_main_thread` 要求闭包是 `'static`，所以把窗口克隆一份带进去。
    let window = window.clone();
    let _ = handle.run_on_main_thread(move || {
        let Ok(ptr) = window.ns_window() else {
            return;
        };
        // SAFETY: `ns_window` 返回的是 tao 持有的 NSWindow 的裸指针，
        // 由 app 持有的 WebviewWindow 保证它在本回调执行期间有效。
        // `retain` 一下，免得回调返回后窗口被销毁而我们还在用。
        let Some(ns_window) = (unsafe { Retained::<NSWindow>::retain(ptr.cast()) }) else {
            return;
        };

        let mut behavior = ns_window.collectionBehavior();
        // FullScreenAuxiliary 与 FullScreenPrimary / FullScreenNone 互斥，
        // 三个只能选一个，所以先把另两个清掉再设。
        behavior.remove(NSWindowCollectionBehavior::FullScreenPrimary);
        behavior.remove(NSWindowCollectionBehavior::FullScreenNone);
        behavior.insert(NSWindowCollectionBehavior::FullScreenAuxiliary);
        ns_window.setCollectionBehavior(behavior);
    });
}

/// 非 macOS 平台是空实现（Windows 上 `alwaysOnTop` 已经够用）。
#[cfg(not(target_os = "macos"))]
pub fn enable_fullscreen_overlay(_app: &AppHandle, _window: &WebviewWindow) {}

/// 应用启动时给卡片窗口打上标记。
///
/// 面板窗口不需要浮在全屏之上，所以只处理卡片。
pub fn prepare_windows(app: &AppHandle) {
    if let Some(card) = app.get_webview_window(crate::CARD_WINDOW) {
        enable_fullscreen_overlay(app, &card);
    }
}
