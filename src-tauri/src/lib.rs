//! 拾遗 · 经典语录菜单栏小工具
//!
//! 后端职责：
//! - 内嵌 265 条语料（编译期 `include_str!`，运行期零文件 IO）
//! - 收藏 / 历史 / 设置持久化
//! - 定时调度：在设定时间窗内随机时刻弹出卡片
//! - 托盘菜单与全局快捷键

mod corpus;
mod paths;
mod quotes;
mod scheduler;
mod settings;
mod store;

#[cfg(target_os = "macos")]
mod macos;

use crate::corpus::{BookInfo, BookOpResult};
use crate::quotes::Quote;
use crate::scheduler::Scheduler;
use crate::settings::{Settings, SettingsStore};
use crate::store::{DataStore, HistoryEntry};
use chrono::Local;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// 导出一本书的结果。`content` 带着 JSON 文本，前端写盘。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub ok: bool,
    pub message: String,
    pub content: Option<String>,
}

/// 与 `tauri.conf.json` 中的 `identifier` 保持一致。
pub const APP_IDENTIFIER: &str = "site.hancic.shiyi";

/// 卡片窗口 label
const CARD_WINDOW: &str = "card";
/// 设置窗口 label
const SETTINGS_WINDOW: &str = "settings";
/// 收藏夹窗口 label
const FAVORITES_WINDOW: &str = "favorites";
/// 历史窗口 label
const HISTORY_WINDOW: &str = "history";
/// 书库窗口 label
const LIBRARY_WINDOW: &str = "library";

/// 卡片窗口宽度（逻辑像素）。与前端 `.card` 的 372px + 边框一致。
const CARD_WIDTH: f64 = 380.0;
/// 卡片窗口最小高度，避免极短语录时窗口塌缩。
const MIN_CARD_HEIGHT: f64 = 150.0;
/// 卡片距屏幕右边缘的间距（逻辑像素）。
const CARD_MARGIN_RIGHT: f64 = 20.0;
/// 卡片顶边距屏幕可见区域顶部的间距（逻辑像素）。
const CARD_MARGIN_TOP: f64 = 10.0;

/// 调度线程是否已启动，避免重复启动。
static SCHEDULER_STARTED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------- 命令

#[tauri::command]
fn get_quotes() -> Vec<Quote> {
    quotes::all()
}

#[tauri::command]
fn get_random_quote() -> Option<Quote> {
    let mut all = quotes::all();
    all.shuffle(&mut rand::thread_rng());
    all.into_iter().next()
}

#[tauri::command]
fn get_quote_by_id(id: String) -> Option<Quote> {
    corpus::Corpus::global().by_id(&id)
}

#[tauri::command]
fn get_favorites() -> Vec<String> {
    let favs = DataStore::global().favorites();
    refresh_tray_menu();
    favs
}

#[tauri::command]
fn is_favorite(id: String) -> bool {
    DataStore::global().is_favorite(&id)
}

#[tauri::command]
fn toggle_favorite(app: AppHandle, id: String) -> bool {
    let state = DataStore::global().toggle_favorite(&id);
    refresh_tray_menu();
    emit_data_changed(&app);
    state
}

#[tauri::command]
fn get_history() -> Vec<HistoryEntry> {
    DataStore::global().history()
}

#[tauri::command]
fn clear_history(app: AppHandle) {
    DataStore::global().clear_history();
    emit_data_changed(&app);
    refresh_tray_menu();
}

/// 广播「收藏 / 历史变了」。
///
/// 收藏夹、历史面板的窗口是**常驻隐藏窗口**（关掉只是隐藏，窗口和里面的
/// Vue 应用都不销毁），所以它们的 `onMounted` 只在应用启动时跑一次。
/// 数据之后发生变化时前端不会自动知道，面板就会一直显示启动那一刻的旧内容
/// （例如卡片已经弹出好几条，历史面板却还是「0 条」）。
///
/// 所以数据一改就发这个事件，前端收到后重新拉一次。
fn emit_data_changed(app: &AppHandle) {
    let _ = app.emit("data-changed", ());
}

#[tauri::command]
fn get_settings() -> Settings {
    SettingsStore::global().get()
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Settings {
    let old = SettingsStore::global().get();
    SettingsStore::global().set(settings.clone());

    // 快捷键变了就重新注册
    if old.hotkey != settings.hotkey {
        let _ = app.global_shortcut().unregister_all();
        register_hotkey(&app, &settings.hotkey);
    }
    refresh_tray_menu();
    settings
}

/// 弹出卡片。可指定语录；不指定则随机取一条。
///
/// `record` 为 false 时不写历史——从收藏夹 / 历史列表里回看某条时，
/// 不应该把它当成一次新的弹出（spec 明确要求「不重复记历史」）。
#[tauri::command]
fn show_card(app: AppHandle, quote: Option<Quote>, record: Option<bool>) {
    let quote = match quote {
        Some(q) => q,
        None => match get_random_quote() {
            Some(q) => q,
            None => return,
        },
    };
    if record.unwrap_or(true) {
        DataStore::global().record_history(&quote);
    }
    show_quote_card(&app, &quote);
}

/// 显示卡片。卡片窗口配置为 `focus: false` / `focusable: false`，
/// 因此弹出时不会抢走当前应用的焦点。
fn show_quote_card(app: &AppHandle, quote: &Quote) {
    if let Some(window) = app.get_webview_window(CARD_WINDOW) {
        // 每次弹出都重新确保一次「浮在全屏应用之上」的标记。
        // macOS 在窗口重建 / space 切换后可能重置 collection behavior。
        #[cfg(target_os = "macos")]
        macos::enable_fullscreen_overlay(app, &window);

        let _ = window.emit("show-quote", quote);
        // 每次弹出都把卡片挪回右上角（用户拖走过的卡片复位）
        position_card_top_right(&window);
        let _ = window.show();
    }
    // 卡片弹出的所有路径（定时器 / 快捷键 / 托盘 / 命令）都经过这里，
    // 在这里统一刷新托盘计数，避免各条路径各写一份而漏掉。
    refresh_tray_menu();
    // 同理，弹出通常伴随一条新的历史记录，通知常驻面板重新拉数据。
    emit_data_changed(app);
}

/// 把卡片窗口放到屏幕右上角。
///
/// 坐标约定（已在 tao / tauri-runtime-wry 源码中核对）：
/// - `Monitor::work_area()` 返回的是**可见区域**（macOS 上即 `NSScreen.visibleFrame`，
///   已扣掉菜单栏和 Dock），且已换算成**左上角原点**的全局坐标。
/// - `Window::set_position` 在 macOS 上同样是左上角原点（tao 内部再翻成
///   AppKit 的左下角原点）。
///
/// 注意 `work_area` / `outer_size` 都是物理像素，要除以缩放系数才能得到逻辑像素。
fn position_card_top_right(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let Ok(window_size) = window.outer_size() else {
        return;
    };

    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();

    let area_x = work_area.position.x as f64 / scale;
    let area_y = work_area.position.y as f64 / scale;
    let area_w = work_area.size.width as f64 / scale;
    let card_w = window_size.width as f64 / scale;

    // 卡片宽高固定，右边缘贴着可见区域右边缘，顶边留一点间距。
    let x = area_x + area_w - card_w - CARD_MARGIN_RIGHT;
    let y = area_y + CARD_MARGIN_TOP;

    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
}

#[tauri::command]
fn hide_card(app: AppHandle) {
    if let Some(window) = app.get_webview_window(CARD_WINDOW) {
        let _ = window.hide();
    }
}

/// 隐藏设置窗口（保存完自动关掉面板时用）。
///
/// 面板窗口关闭时是「隐藏」而不是销毁——Tauri 里被销毁的窗口无法再次显示，
/// 所以这里调 `hide` 而不是 `close`。
#[tauri::command]
fn hide_settings(app: AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        let _ = window.hide();
    }
}

/// 卡片窗口按内容调整尺寸。语录长短不一，固定高度会截断长句。
#[tauri::command]
fn resize_card(app: AppHandle, height: f64) {
    if let Some(window) = app.get_webview_window(CARD_WINDOW) {
        let _ = window.set_size(tauri::LogicalSize::new(CARD_WIDTH, height.max(MIN_CARD_HEIGHT)));
    }
}

/// 打开收藏夹 / 历史 / 设置 / 书库窗口。
#[tauri::command]
fn open_window(app: AppHandle, label: String) {
    let window = match label.as_str() {
        FAVORITES_WINDOW => app.get_webview_window(FAVORITES_WINDOW),
        HISTORY_WINDOW => app.get_webview_window(HISTORY_WINDOW),
        SETTINGS_WINDOW => app.get_webview_window(SETTINGS_WINDOW),
        LIBRARY_WINDOW => app.get_webview_window(LIBRARY_WINDOW),
        _ => None,
    };
    if let Some(w) = window {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.center();
        // 面板是常驻隐藏窗口，打开时再拉一次数据：即使错过了广播
        // （例如面板窗口还没建好时数据就变了），这里也能拿到最新内容。
        emit_data_changed(&app);
    }
}

/// 下一次弹出的时间戳（毫秒）。`null` 表示今天不再弹出。
#[tauri::command]
fn get_next_fire_date() -> Option<i64> {
    let settings = SettingsStore::global().get();
    let fired = DataStore::global().fired_today();
    Scheduler::next_fire_date(Local::now(), &settings, &fired)
}

/// 设置开机自启。返回设置后的实际状态。
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => enabled,
        Err(err) => {
            eprintln!("设置开机自启失败: {err}");
            !enabled
        }
    }
}

#[tauri::command]
fn is_autostart_enabled(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// 应用界面外观：auto（跟随系统）/ light / dark。
///
/// 除了切换原生窗口的标题栏配色，还要通知前端改写 CSS 变量，
/// 否则浅色设置下系统深色标题栏配深色文字会看不清。
#[tauri::command]
fn set_appearance(app: AppHandle, mode: String) {
    apply_appearance(&app, &mode);
    let _ = app.emit("appearance-changed", mode);
}

/// 把外观模式应用到所有窗口。
///
/// 只作用于收藏夹 / 历史 / 设置这三个**带原生标题栏**的面板窗口：
/// 标题栏配色要跟着界面样式走，否则浅色设置下深色标题栏配深色文字会看不清。
///
/// 卡片窗口不在此列——它透明、无边框，五套卡片主题各有自己的配色。
/// 给它设原生主题会让 macOS 重建窗口的透明图层，webview 内容不再合成，
/// 结果是卡片整个消失（历史照记，托盘计数照涨，就是看不见）。
pub fn apply_appearance(app: &AppHandle, mode: &str) {
    let theme = match mode {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    for (label, window) in app.webview_windows() {
        if label == CARD_WINDOW {
            continue;
        }
        let _ = window.set_theme(theme);
    }
}

// ---------------------------------------------------------------- 书库

/// 书库里的书籍列表。
#[tauri::command]
fn get_books() -> Vec<BookInfo> {
    corpus::Corpus::global().books()
}

/// 导入一本书。从 `path` 读 JSON 文件。
///
/// 返回 `{ ok, message }`，`message` 是给用户看的中文提示（成功或失败原因）。
#[tauri::command]
fn import_book(app: AppHandle, path: String) -> BookOpResult {
    let result = match read_book_file(&path) {
        Ok((name, quotes)) => corpus::Corpus::global().import(&name, quotes),
        Err(err) => err,
    };
    if result.ok {
        refresh_tray_menu();
        emit_data_changed(&app);
    }
    result
}

/// 导出一本书。返回 `{ ok, message, content }`。
///
/// `content` 是 JSON 文本，由前端交给文件对话框写盘——后端不直接弹
/// 保存对话框，免得两个模态框叠在一起。
#[tauri::command]
fn export_book(name: String) -> ExportResult {
    match corpus::Corpus::global().export(&name) {
        Ok(content) => ExportResult {
            ok: true,
            message: format!("已导出《{name}》"),
            content: Some(content),
        },
        Err(message) => ExportResult {
            ok: false,
            message,
            content: None,
        },
    }
}

/// 移除一本书。
#[tauri::command]
fn remove_book(app: AppHandle, name: String) -> BookOpResult {
    let result = corpus::Corpus::global().remove(&name);
    if result.ok {
        refresh_tray_menu();
        emit_data_changed(&app);
    }
    result
}

/// 读一个 JSON 语料文件，推断书名。
///
/// 书名取自语录自身的 `book` 字段；文件里没有就用文件名（去掉扩展名）。
fn read_book_file(path: &str) -> Result<(String, Vec<Quote>), BookOpResult> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| BookOpResult::err(format!("读取文件失败：{e}")))?;
    let quotes: Vec<Quote> = serde_json::from_str(&raw)
        .map_err(|e| BookOpResult::err(format!("不是有效的语料 JSON：{e}")))?;
    if quotes.is_empty() {
        return Err(BookOpResult::err("这个文件里没有语录"));
    }
    // 书名优先用语录里的 book 字段；没有就退回文件名
    let name = quotes
        .first()
        .map(|q| q.book.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "未命名".to_string())
        });
    Ok((name, quotes))
}

// ---------------------------------------------------------------- 调度

/// 调度线程：循环计算下一次触发时间并等待。
///
/// 用独立线程而非 tokio 定时器，避免为 Tauri 之外再引入一份 tokio 依赖。
fn spawn_scheduler(app: AppHandle) {
    if SCHEDULER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || loop {
        let settings = SettingsStore::global().get();
        let fired = DataStore::global().fired_today();

        match Scheduler::next_fire_date(Local::now(), &settings, &fired) {
            Some(ts) => {
                let _ = app.emit("next-fire-date", ts);
                // 分段睡眠：每 30 秒醒一次重算，设置变更后能及时生效
                loop {
                    let now = Local::now().timestamp_millis();
                    if now >= ts {
                        break;
                    }
                    let remaining = (ts - now) as u64;
                    std::thread::sleep(Duration::from_millis(remaining.min(30_000)));
                }
                on_timer_fired(&app);
            }
            None => {
                // 今天不弹（周末或无有效计划）：每分钟检查一次
                let _ = app.emit("next-fire-date", Option::<i64>::None);
                std::thread::sleep(Duration::from_millis(60_000));
            }
        }
    });
}

/// 定时器到点：弹出卡片并记录触发时间。
fn on_timer_fired(app: &AppHandle) {
    let now = Local::now();
    DataStore::global().record_fired(now.timestamp_millis());

    if let Some(q) = get_random_quote() {
        DataStore::global().record_history(&q);
        show_quote_card(app, &q);
    }
    refresh_tray_menu();
}

// ---------------------------------------------------------------- 快捷键

fn register_hotkey(app: &AppHandle, spec: &str) {
    let shortcut = match scheduler::parse_shortcut(spec) {
        Some(s) => s,
        None => {
            eprintln!("快捷键格式无效: {spec}");
            return;
        }
    };
    let handle = app.clone();
    if let Err(err) = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _sc, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(q) = get_random_quote() {
                    DataStore::global().record_history(&q);
                    show_quote_card(&handle, &q);
                }
            }
        })
    {
        eprintln!("注册全局快捷键失败: {err}");
    }
}

// ---------------------------------------------------------------- 托盘

/// 托盘菜单里需要动态刷新的条目。
///
/// 「今日已弹」是一个子菜单，条目随当天弹出的语录增减，所以只留子菜单句柄，
/// 每次刷新时重建它的子项。
struct TrayMenuItems {
    favorites: MenuItem<tauri::Wry>,
    today: Submenu<tauri::Wry>,
    next: MenuItem<tauri::Wry>,
    library: MenuItem<tauri::Wry>,
}

/// 托盘菜单各项的句柄，供刷新计数用。
static TRAY_ITEMS: OnceLock<Mutex<Option<TrayMenuItems>>> = OnceLock::new();

fn tray_items() -> &'static Mutex<Option<TrayMenuItems>> {
    TRAY_ITEMS.get_or_init(|| Mutex::new(None))
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_now = MenuItem::with_id(app, "show_now", "随机来一条", true, None::<&str>)?;
    // 「今日已弹」做成子菜单：点开后列出当天弹过的语录，可以直接点回看。
    // spec 明确要求这些条目必须可点，所以用 MenuItem 而不是灰字 Text。
    let today = Submenu::with_id(app, "today", "今日暂无", true)?;
    let next = MenuItem::with_id(app, "next", "下次弹出: —", false, None::<&str>)?;
    let favorites = MenuItem::with_id(app, "favorites", "收藏夹 (0)", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "历史记录", true, None::<&str>)?;
    let library = MenuItem::with_id(app, "library", "书库 (0)", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_now,
            &sep,
            &today,
            &next,
            &sep,
            &favorites,
            &history,
            &library,
            &settings,
            &sep,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("拾遗 · 经典语录")
        .menu(&menu)
        // 左键直接弹菜单，与 macOS 菜单栏行为一致
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "show_now" => {
                    if let Some(q) = get_random_quote() {
                        DataStore::global().record_history(&q);
                        show_quote_card(app, &q);
                    }
                    refresh_tray_menu();
                }
                // 回看当天弹过的某条：不再记一次历史
                id if id.starts_with("today_entry:") => {
                    let quote_id = id.trim_start_matches("today_entry:");
                    if let Some(q) = quotes::all().into_iter().find(|q| q.id == quote_id) {
                        show_quote_card(app, &q);
                    }
                }
                "favorites" => open_window(app.clone(), FAVORITES_WINDOW.into()),
                "history" => open_window(app.clone(), HISTORY_WINDOW.into()),
                "library" => open_window(app.clone(), LIBRARY_WINDOW.into()),
                "settings" => open_window(app.clone(), SETTINGS_WINDOW.into()),
                "quit" => app.exit(0),
                _ => {}
            }
        });

    // 菜单栏图标用单色 template image，系统按菜单栏深浅自动反色
    match tray_icon() {
        Some(icon) => {
            builder = builder.icon(icon).icon_as_template(true);
        }
        None => {
            // 兜底：用打包进二进制的默认图标
            if let Some(icon) = app.default_window_icon().cloned() {
                builder = builder.icon(icon);
            }
        }
    }

    builder.build(app)?;

    *tray_items().lock().expect("tray mutex poisoned") = Some(TrayMenuItems {
        favorites,
        today,
        next,
        library,
    });
    refresh_tray_menu();
    Ok(())
}

/// 刷新托盘菜单里的计数与时间。
///
/// 「今日已弹」按本地自然日统计历史条数，并把当天弹过的语录列成可点的子项；
/// 「下次弹出」用调度器算出的时间。
///
/// 菜单项的增删改必须在主线程做（AppKit 要求），而后台线程调用
/// `run_on_main_thread` 是投递到主线程事件循环里异步执行的——如果这里同步等待
/// 主线程，主线程此刻又在处理菜单事件回调本函数，就会互相等待。所以整个刷新
/// 作为一个任务投给主线程，不阻塞调用方。
///
/// 顺带的好处：所有刷新都在主线程上串行执行，不会出现两个线程同时重建子菜单。
pub fn refresh_tray_menu() {
    let (favorites, today, next, library) = {
        let guard = tray_items().lock().expect("tray mutex poisoned");
        match guard.as_ref() {
            Some(items) => (
                items.favorites.clone(),
                items.today.clone(),
                items.next.clone(),
                items.library.clone(),
            ),
            None => return,
        }
    };

    // 先在调用线程里把要用的数据算好，主线程上只做纯粹的菜单改动。
    let today_entries = DataStore::global().today_history();
    let fav_count = DataStore::global().favorites().len();
    let book_count = corpus::Corpus::global().books().len();
    let settings = SettingsStore::global().get();
    let fired = DataStore::global().fired_today();
    let next_text = match Scheduler::next_fire_date(Local::now(), &settings, &fired) {
        Some(ts) => {
            let dt = chrono::DateTime::from_timestamp_millis(ts)
                .map(|d| d.with_timezone(&Local))
                .unwrap_or_else(Local::now);
            format!("下次弹出: {}", dt.format("%m-%d %H:%M"))
        }
        None => "下次弹出: 今日不弹".to_string(),
    };

    let app = today.app_handle().clone();
    let _ = app.run_on_main_thread(move || {
        let today_count = today_entries.len();
        let today_text = if today_count == 0 {
            "今日暂无".to_string()
        } else {
            format!("今日已弹 ({today_count})")
        };
        let _ = today.set_text(today_text);
        // 没有内容时置灰，避免点开是一个空子菜单。
        let _ = today.set_enabled(today_count > 0);

        // 子菜单的条目是动态的：先清空再按当天历史重建。
        rebuild_today_submenu(&today, &today_entries);

        let _ = favorites.set_text(format!("收藏夹 ({fav_count})"));
        let _ = library.set_text(format!("书库 ({book_count})"));
        let _ = next.set_text(next_text);
    });
}

/// 重建「今日已弹」子菜单的条目。
///
/// 每条语录一个可点的菜单项，id 形如 `today_entry:<语录 id>`，
/// 点击后由 `on_menu_event` 回看该条（不重复记历史）。
fn rebuild_today_submenu(submenu: &Submenu<tauri::Wry>, entries: &[HistoryEntry]) {
    let app = submenu.app_handle().clone();

    // 清空旧条目。remove_at 按位置删，所以从后往前删。
    let existing = submenu.items().unwrap_or_default();
    for i in (0..existing.len()).rev() {
        let _ = submenu.remove_at(i);
    }

    if entries.is_empty() {
        return;
    }

    for entry in entries {
        let label = format!("{} · {}", truncate(&entry.text, 20), entry.source);
        if let Ok(item) = MenuItem::with_id(
            &app,
            format!("today_entry:{}", entry.quote_id),
            label,
            true,
            None::<&str>,
        ) {
            let _ = submenu.append(&item);
        }
    }
}

/// 按字符（不是字节）截断字符串，尾部补省略号。
fn truncate(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i == max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// 菜单栏图标。编译期内嵌，不依赖运行时的外部资源文件。
fn tray_icon() -> Option<tauri::image::Image<'static>> {
    static ICON: &[u8] = include_bytes!("../icons/tray-icon.png");
    tauri::image::Image::from_bytes(ICON).ok()
}

// ---------------------------------------------------------------- 入口

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let handle = app.handle().clone();

            // 不占 Dock 图标，纯托盘驻留
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            build_tray(&handle)?;

            // 首次启动把默认设置落盘，`settings.json` 始终存在
            SettingsStore::global().ensure_persisted();

            // 让卡片能浮在全屏应用之上（macOS 专有行为）
            #[cfg(target_os = "macos")]
            macos::prepare_windows(&handle);

            // 面板窗口关闭时改为隐藏：Tauri 里被销毁的窗口无法再次显示
            for label in [
                SETTINGS_WINDOW,
                FAVORITES_WINDOW,
                HISTORY_WINDOW,
                LIBRARY_WINDOW,
            ] {
                if let Some(window) = handle.get_webview_window(label) {
                    let win = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = win.hide();
                        }
                    });
                }
            }

            // 注册全局快捷键
            register_hotkey(&handle, &SettingsStore::global().get().hotkey);

            // 应用上次保存的外观
            let appearance = SettingsStore::global().get().appearance_mode;
            apply_appearance(&handle, &appearance);

            // 启动定时调度
            spawn_scheduler(handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_quotes,
            get_random_quote,
            get_quote_by_id,
            get_favorites,
            is_favorite,
            toggle_favorite,
            get_history,
            clear_history,
            get_settings,
            save_settings,
            show_card,
            hide_card,
            hide_settings,
            resize_card,
            open_window,
            get_next_fire_date,
            set_autostart,
            is_autostart_enabled,
            set_appearance,
            get_books,
            import_book,
            export_book,
            remove_book,
        ])
        .run(tauri::generate_context!())
        .expect("拾遗启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_counts_characters_not_bytes() {
        assert_eq!(truncate("君子不器。", 20), "君子不器。");
        assert_eq!(truncate("君子不器。", 4), "君子不器…");
    }

    #[test]
    fn truncate_handles_multibyte_safely() {
        // 中文 3 字节，截断不能切坏 UTF-8
        let long = "知人者智，自知者明。胜人者有力，自胜者强。";
        let out = truncate(long, 10);
        assert_eq!(out.chars().count(), 11);
        assert!(out.ends_with('…'));
        assert!(long.starts_with(&out[..out.len() - '…'.len_utf8()]));
    }

    #[test]
    fn today_entry_id_prefix_is_stable() {
        // 托盘菜单靠这个前缀区分「回看今天某条」和普通菜单项
        assert!("today_entry:ddj-1".starts_with("today_entry:"));
        assert_eq!(
            "today_entry:ddj-1".trim_start_matches("today_entry:"),
            "ddj-1"
        );
    }

    #[test]
    fn tray_icon_decodes_to_a_visible_shape() {
        // 菜单栏图标必须真的解出像素：之前图标渲染不出来（状态项占位但全黑），
        // 而 `from_bytes` 出错时被 `.ok()` 吞掉了，静默变成「没有图标」。
        let img = tray_icon().expect("tray-icon.png 应该能解码");
        assert_eq!(img.width(), 44);
        assert_eq!(img.height(), 44);

        let rgba = img.rgba();
        assert_eq!(rgba.len(), 44 * 44 * 4);

        // 模板图标只有 alpha 通道有意义：要有不透明像素，也要有透明镂空
        let opaque = (0..rgba.len()).step_by(4).filter(|i| rgba[*i + 3] > 10).count();
        let clear = (0..rgba.len()).step_by(4).filter(|i| rgba[*i + 3] < 10).count();
        assert!(opaque > 200, "实心像素太少: {opaque}");
        assert!(clear > 200, "镂空像素太少: {clear}");

        // 模板图标必须是无色的：RGB 通道全为 0，交给系统按菜单栏深浅反色
        let colored = (0..rgba.len())
            .step_by(4)
            .filter(|i| rgba[*i] > 10 || rgba[*i + 1] > 10 || rgba[*i + 2] > 10)
            .count();
        assert_eq!(colored, 0, "模板图标不该有颜色");
    }
}
