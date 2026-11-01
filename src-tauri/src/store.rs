use crate::quotes::Quote;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

/// 历史记录条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub quote_id: String,
    pub timestamp: i64,
    pub text: String,
    pub source: String,
}

/// 收藏与历史的持久化存储。数据落在应用数据目录：
/// - macOS: `~/Library/Application Support/site.hancic.shiyi/`
/// - Windows: `%APPDATA%\site.hancic.shiyi\`
pub struct DataStore {
    dir: PathBuf,
    inner: Mutex<Data>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Data {
    favorites: HashSet<String>,
    history: Vec<HistoryEntry>,
    /// 已触发的时间戳（毫秒）。保留最近几天，避免重启后重复弹出。
    fired: Vec<i64>,
}

const MAX_HISTORY: usize = 200;
/// 触发记录保留天数
const FIRED_RETENTION_DAYS: i64 = 7;
const DAY_MS: i64 = 86_400_000;

impl DataStore {
    /// 用指定目录构造（测试用）。
    pub fn with_dir(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let data = Self::load(&dir).unwrap_or_default();
        Self {
            dir,
            inner: Mutex::new(data),
        }
    }

    fn load(dir: &PathBuf) -> Option<Data> {
        let path = dir.join("data.json");
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn global() -> &'static DataStore {
        static STORE: OnceLock<DataStore> = OnceLock::new();
        STORE.get_or_init(|| {
            let dir = app_data_dir();
            Self::with_dir(dir)
        })
    }

    fn persist(&self, data: &Data) {
        let path = self.dir.join("data.json");
        if let Ok(raw) = serde_json::to_string_pretty(data) {
            let _ = fs::write(path, raw);
        }
    }

    pub fn favorites(&self) -> Vec<String> {
        let data = self.inner.lock().expect("store mutex poisoned");
        let mut v: Vec<String> = data.favorites.iter().cloned().collect();
        v.sort();
        v
    }

    pub fn is_favorite(&self, quote_id: &str) -> bool {
        let data = self.inner.lock().expect("store mutex poisoned");
        data.favorites.contains(quote_id)
    }

    /// 切换收藏状态，返回切换后的状态。
    pub fn toggle_favorite(&self, quote_id: &str) -> bool {
        let mut data = self.inner.lock().expect("store mutex poisoned");
        let now_favorite = if data.favorites.contains(quote_id) {
            data.favorites.remove(quote_id);
            false
        } else {
            data.favorites.insert(quote_id.to_string());
            true
        };
        self.persist(&data);
        now_favorite
    }

    /// 批量取消收藏。`should_remove` 对每个收藏 id 调一次，返回 true 的被移除。
    ///
    /// 移除书籍时用：书没了，它下面的收藏就成了打不开的条目，要一起清掉。
    /// 返回被清掉的条数。
    pub fn remove_favorites<F: Fn(&str) -> bool>(&self, should_remove: F) -> usize {
        let mut data = self.inner.lock().expect("store mutex poisoned");
        let before = data.favorites.len();
        data.favorites.retain(|id| !should_remove(id));
        let removed = before - data.favorites.len();
        if removed > 0 {
            self.persist(&data);
        }
        removed
    }

    pub fn history(&self) -> Vec<HistoryEntry> {
        let data = self.inner.lock().expect("store mutex poisoned");
        data.history.clone()
    }

    /// 今天（按本地时区）弹出的条目，最新的在前。
    pub fn today_history(&self) -> Vec<HistoryEntry> {
        let data = self.inner.lock().expect("store mutex poisoned");
        let today = chrono::Local::now().date_naive();
        data.history
            .iter()
            .filter(|e| {
                chrono::DateTime::from_timestamp_millis(e.timestamp)
                    .map(|dt| dt.with_timezone(&chrono::Local).date_naive() == today)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// 今天（按本地时区）弹出的条数。
    #[cfg(test)]
    pub fn today_history_count(&self) -> usize {
        self.today_history().len()
    }

    /// 记录一次展示。历史上限 200 条，超出丢弃最旧的。
    pub fn record_history(&self, quote: &Quote) {
        let mut data = self.inner.lock().expect("store mutex poisoned");
        let entry = HistoryEntry {
            id: format!("{}-{}", quote.id, chrono::Utc::now().timestamp_millis()),
            quote_id: quote.id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            text: quote.text.clone(),
            source: quote.display_source(),
        };
        data.history.insert(0, entry);
        if data.history.len() > MAX_HISTORY {
            data.history.truncate(MAX_HISTORY);
        }
        self.persist(&data);
    }

    /// 清空历史。
    pub fn clear_history(&self) {
        let mut data = self.inner.lock().expect("store mutex poisoned");
        data.history.clear();
        self.persist(&data);
    }

    /// 今天（按本地时区）已触发过的时间戳。
    pub fn fired_today(&self) -> Vec<i64> {
        let data = self.inner.lock().expect("store mutex poisoned");
        let today = chrono::Local::now().date_naive();
        data.fired
            .iter()
            .copied()
            .filter(|ts| {
                chrono::DateTime::from_timestamp_millis(*ts)
                    .map(|dt| dt.with_timezone(&chrono::Local).date_naive() == today)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// 记录一次触发。同时顺带清理过期的触发记录。
    pub fn record_fired(&self, timestamp: i64) {
        let mut data = self.inner.lock().expect("store mutex poisoned");
        let cutoff = timestamp - FIRED_RETENTION_DAYS * DAY_MS;
        data.fired.retain(|ts| *ts >= cutoff);
        data.fired.push(timestamp);
        self.persist(&data);
    }
}

/// 应用数据目录。与设置文件共用同一个目录。
fn app_data_dir() -> PathBuf {
    crate::paths::app_data_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quotes;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("shiyi-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn toggles_favorite_both_ways() {
        let store = DataStore::with_dir(temp_dir("fav"));
        assert!(!store.is_favorite("ddj-1"));
        assert!(store.toggle_favorite("ddj-1"));
        assert!(store.is_favorite("ddj-1"));
        assert!(!store.toggle_favorite("ddj-1"));
        assert!(!store.is_favorite("ddj-1"));
    }

    #[test]
    fn records_history_newest_first() {
        let store = DataStore::with_dir(temp_dir("hist"));
        let all = quotes::all();
        store.record_history(&all[0]);
        store.record_history(&all[1]);
        let hist = store.history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].quote_id, all[1].id, "最新记录应排在最前");
    }

    #[test]
    fn history_is_capped_at_200() {
        let store = DataStore::with_dir(temp_dir("cap"));
        let all = quotes::all();
        // 同一批语录反复记录，验证上限裁剪
        for i in 0..250 {
            store.record_history(&all[i % all.len()]);
        }
        assert_eq!(store.history().len(), MAX_HISTORY);
    }

    #[test]
    fn persists_across_instances() {
        let dir = temp_dir("persist");
        {
            let store = DataStore::with_dir(dir.clone());
            store.toggle_favorite("ly-1");
        }
        let reopened = DataStore::with_dir(dir);
        assert!(reopened.is_favorite("ly-1"), "重启后收藏应保留");
    }

    #[test]
    fn clear_history_empties_list() {
        let store = DataStore::with_dir(temp_dir("clear"));
        store.record_history(&quotes::all()[0]);
        assert_eq!(store.history().len(), 1);
        store.clear_history();
        assert!(store.history().is_empty());
    }

    #[test]
    fn fired_records_survive_reopen() {
        let dir = temp_dir("fired");
        let now = chrono::Local::now().timestamp_millis();
        {
            let store = DataStore::with_dir(dir.clone());
            store.record_fired(now);
        }
        let reopened = DataStore::with_dir(dir);
        assert_eq!(reopened.fired_today(), vec![now], "重启后不应重复弹出当天已弹过的内容");
    }

    #[test]
    fn fired_today_ignores_old_records() {
        let store = DataStore::with_dir(temp_dir("fired-old"));
        let old = chrono::Local::now().timestamp_millis() - 3 * DAY_MS;
        store.record_fired(old);
        assert!(store.fired_today().is_empty(), "三天前的记录不算今天");
    }

    #[test]
    fn today_history_only_keeps_today() {
        let store = DataStore::with_dir(temp_dir("today"));
        let all = quotes::all();
        store.record_history(&all[0]);
        store.record_history(&all[1]);

        let today = store.today_history();
        assert_eq!(today.len(), 2, "刚记录的两条都算今天");
        assert_eq!(today[0].quote_id, all[1].id, "最新的排在最前");
        assert_eq!(store.today_history_count(), 2);
    }

    #[test]
    fn today_history_ignores_yesterday() {
        let store = DataStore::with_dir(temp_dir("today-old"));
        // 手工塞一条昨天的记录
        let yesterday = chrono::Local::now().timestamp_millis() - DAY_MS;
        let mut data = store.inner.lock().expect("store mutex poisoned");
        data.history.push(HistoryEntry {
            id: "old-1".to_string(),
            quote_id: "ddj-1".to_string(),
            timestamp: yesterday,
            text: "旧条目".to_string(),
            source: "《道德经》".to_string(),
        });
        drop(data);

        assert!(
            store.today_history().is_empty(),
            "昨天的记录不该算进今天"
        );
    }
}
