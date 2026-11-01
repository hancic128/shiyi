use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

/// 用户设置。持久化到应用数据目录下的 `settings.json`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 弹出时间窗起止小时（0-23）
    pub start_hour: u32,
    pub end_hour: u32,
    /// 每天弹出次数
    pub popups_per_day: u32,
    /// 周末是否跳过
    pub skip_weekend: bool,
    /// 卡片自动消失秒数
    pub auto_dismiss_seconds: u32,
    /// 主题：xuanzhi / ink / bamboo / night / jade / random
    pub theme: String,
    /// 界面外观：auto / light / dark
    pub appearance_mode: String,
    /// 全局快捷键，形如 "CmdOrCtrl+Shift+L"
    pub hotkey: String,
    /// 开机自启
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_hour: 9,
            end_hour: 21,
            popups_per_day: 2,
            skip_weekend: true,
            auto_dismiss_seconds: 8,
            theme: "xuanzhi".to_string(),
            appearance_mode: "auto".to_string(),
            hotkey: "CmdOrCtrl+Shift+L".to_string(),
            launch_at_login: false,
        }
    }
}

/// 全局设置单例。跨线程共享用 `Mutex` 保护，写入时同步落盘。
pub struct SettingsStore {
    dir: PathBuf,
    inner: Mutex<Settings>,
}

static SETTINGS: OnceLock<SettingsStore> = OnceLock::new();

impl SettingsStore {
    /// 用指定目录构造（测试用）。
    pub fn with_dir(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let settings = Self::load(&dir).unwrap_or_default();
        Self {
            dir,
            inner: Mutex::new(settings),
        }
    }

    fn load(dir: &PathBuf) -> Option<Settings> {
        let raw = fs::read_to_string(dir.join("settings.json")).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn global() -> &'static SettingsStore {
        SETTINGS.get_or_init(|| Self::with_dir(crate::paths::app_data_dir()))
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().expect("settings mutex poisoned").clone()
    }

    /// 写入并落盘。
    pub fn set(&self, settings: Settings) {
        {
            let mut guard = self.inner.lock().expect("settings mutex poisoned");
            *guard = settings.clone();
        }
        self.persist(&settings);
    }

    fn persist(&self, settings: &Settings) {
        if let Ok(raw) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(self.dir.join("settings.json"), raw);
        }
    }

    /// 首次启动时把默认设置落盘。
    ///
    /// 这样 `settings.json` 总是存在，用户可以直接编辑它，
    /// 不用先打开设置窗口点一次「保存」。
    pub fn ensure_persisted(&self) {
        if !self.dir.join("settings.json").exists() {
            let current = self.get();
            self.persist(&current);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("shiyi-settings-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert!(s.start_hour < s.end_hour, "默认时间窗应有效");
        assert!(s.popups_per_day >= 1);
        assert!(s.auto_dismiss_seconds >= 3);
    }

    #[test]
    fn store_roundtrips() {
        let store = SettingsStore::with_dir(temp_dir("roundtrip"));
        let mut s = store.get();
        s.theme = "ink".to_string();
        s.popups_per_day = 5;
        store.set(s.clone());
        assert_eq!(store.get().theme, "ink");
        assert_eq!(store.get().popups_per_day, 5);
    }

    #[test]
    fn update_mutates_in_place() {
        let store = SettingsStore::with_dir(temp_dir("update"));
        let mut s = store.get();
        s.theme = "jade".to_string();
        store.set(s);
        assert_eq!(store.get().theme, "jade");
    }

    #[test]
    fn persists_across_instances() {
        let dir = temp_dir("persist");
        {
            let store = SettingsStore::with_dir(dir.clone());
            let mut s = store.get();
            s.hotkey = "Ctrl+Alt+K".to_string();
            store.set(s);
        }
        let reopened = SettingsStore::with_dir(dir);
        assert_eq!(
            reopened.get().hotkey,
            "Ctrl+Alt+K",
            "重启后设置应保留"
        );
    }

    #[test]
    fn ensure_persisted_writes_defaults_once() {
        let dir = temp_dir("ensure");
        let path = dir.join("settings.json");
        assert!(!path.exists());

        let store = SettingsStore::with_dir(dir.clone());
        store.ensure_persisted();
        assert!(path.exists(), "首次启动应把默认设置落盘");

        // 已经落盘后再调一次不应覆盖用户改过的值
        let mut s = store.get();
        s.theme = "ink".to_string();
        store.set(s);
        store.ensure_persisted();
        assert_eq!(store.get().theme, "ink");
    }

    #[test]
    fn appearance_mode_defaults_to_auto() {
        assert_eq!(Settings::default().appearance_mode, "auto");
    }
}
