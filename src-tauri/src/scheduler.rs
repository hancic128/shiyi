use crate::settings::Settings;
use chrono::{DateTime, Datelike, Local, LocalResult, NaiveDate, NaiveTime, TimeZone};
use rand::Rng;

/// 调度器：把时间窗 `[start_hour, end_hour)` 均分为 N 段（N = 每天弹出次数），
/// 每段内随机取一个时刻触发。已触发过的段跳过；过了时间窗则顺延到次日。
///
/// 触发记录由调用方持久化，因此重启应用不会重复弹出当天已弹过的内容。
pub struct Scheduler;

impl Scheduler {
    /// 计算下一次触发时间（毫秒时间戳）。
    ///
    /// - `now` 当前时间
    /// - `settings` 当前设置
    /// - `fired_today` 今天已触发过的时间戳（毫秒）
    pub fn next_fire_date(
        now: DateTime<Local>,
        settings: &Settings,
        fired_today: &[i64],
    ) -> Option<i64> {
        let start_hour = settings.start_hour.min(23);
        let end_hour = settings.end_hour.min(24);
        if end_hour <= start_hour {
            // 时间窗无效（起止相同，或结束早于开始）
            return None;
        }
        let count = settings.popups_per_day.max(1);

        if settings.skip_weekend && is_weekend(now.date_naive()) {
            return None;
        }

        let today = now.date_naive();
        let (window_start, window_end) = window_of(today, start_hour, end_hour, now)?;
        let window = window_end - window_start;
        if window <= 0 {
            return None;
        }
        let segment = window / count as i64;
        let now_ms = now.timestamp_millis();

        // 找第一个「尚未触发、且还没结束」的段
        for i in 0..count {
            let seg_start = window_start + segment * i as i64;
            let seg_end = if i == count - 1 {
                window_end
            } else {
                seg_start + segment
            };
            if seg_end <= now_ms {
                // 这一段的时间已经过去
                continue;
            }
            let fired_in_segment = fired_today
                .iter()
                .any(|t| *t >= seg_start && *t < seg_end);
            if fired_in_segment {
                continue;
            }
            // 落在当前段内，且不早于现在
            let lower = seg_start.max(now_ms);
            if seg_end <= lower {
                continue;
            }
            return Some(Self::random_between(lower, seg_end));
        }

        // 今天没有可用的段了 → 规划明天
        Self::plan_next_day(today, start_hour, end_hour, count, settings, now)
    }

    /// 规划下一天第一段的随机时刻。跳过周末时直接跳到周一。
    fn plan_next_day(
        today: NaiveDate,
        start_hour: u32,
        end_hour: u32,
        count: u32,
        settings: &Settings,
        now: DateTime<Local>,
    ) -> Option<i64> {
        let mut next = today + chrono::Days::new(1);
        if settings.skip_weekend {
            // 周末不排程：直接跳到下周一
            while is_weekend(next) {
                next = next + chrono::Days::new(1);
            }
        }
        let (window_start, window_end) = window_of(next, start_hour, end_hour, now)?;
        let window = window_end - window_start;
        if window <= 0 {
            return None;
        }
        let segment = window / count as i64;
        Some(Self::random_between(window_start, window_start + segment))
    }

    /// 在 [lower, upper) 内取随机时刻。
    fn random_between(lower: i64, upper: i64) -> i64 {
        if upper <= lower {
            return lower;
        }
        let mut rng = rand::thread_rng();
        lower + rng.gen_range(0..(upper - lower))
    }
}

/// 是否周末（周六或周日）。
fn is_weekend(date: NaiveDate) -> bool {
    date.weekday().num_days_from_monday() >= 5
}

/// 某一天的时间窗 [起, 止) 对应的毫秒时间戳。
///
/// `end_hour == 24` 表示午夜，落到第二天 00:00。
fn window_of(
    date: NaiveDate,
    start_hour: u32,
    end_hour: u32,
    now: DateTime<Local>,
) -> Option<(i64, i64)> {
    let start = local_ts(date, NaiveTime::from_hms_opt(start_hour, 0, 0)?, now)?;
    let end = if end_hour == 24 {
        local_ts(
            date + chrono::Days::new(1),
            NaiveTime::MIN,
            now,
        )?
    } else {
        local_ts(date, NaiveTime::from_hms_opt(end_hour, 0, 0)?, now)?
    };
    Some((start, end))
}

/// 本地日期 + 本地时刻 → 毫秒时间戳。
///
/// 夏令时切换的边界上，本地时刻可能不存在或是二义的；前者用当前偏移量近似，
/// 后者取较早的那个，保证窗口长度稳定。
fn local_ts(date: NaiveDate, time: NaiveTime, now: DateTime<Local>) -> Option<i64> {
    let naive = date.and_time(time);
    Some(match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.timestamp_millis(),
        LocalResult::Ambiguous(first, _) => first.timestamp_millis(),
        LocalResult::None => {
            let offset_secs = now.offset().local_minus_utc() as i64;
            naive.and_utc().timestamp_millis() - offset_secs * 1000
        }
    })
}

/// 解析快捷键字符串，如 "CmdOrCtrl+Shift+L"、"Ctrl+Alt+K"。
pub fn parse_shortcut(spec: &str) -> Option<tauri_plugin_global_shortcut::Shortcut> {
    let mut modifiers: Vec<&str> = Vec::new();
    let mut key: Option<String> = None;

    for part in spec.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "cmdorcontrol" => modifiers.push("COMMAND"),
            "ctrl" | "control" => modifiers.push("CONTROL"),
            "alt" | "option" => modifiers.push("ALT"),
            "shift" => modifiers.push("SHIFT"),
            "super" | "cmd" | "command" => modifiers.push("COMMAND"),
            other => key = Some(other.to_string()),
        }
    }

    let key = key?;
    // 组合键必须至少带一个修饰键，否则会抢占普通按键
    if modifiers.is_empty() {
        return None;
    }

    let shortcut_str = format!("{}+{}", modifiers.join("+"), key.to_uppercase());
    shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(date: NaiveDate, hour: u32, minute: u32) -> DateTime<Local> {
        let time = NaiveTime::from_hms_opt(hour, minute, 0).unwrap();
        Local.from_local_datetime(&date.and_time(time)).unwrap()
    }

    /// 2026-10-26 是星期一
    fn monday(hour: u32, minute: u32) -> DateTime<Local> {
        at(NaiveDate::from_ymd_opt(2026, 10, 26).unwrap(), hour, minute)
    }

    fn default_settings() -> Settings {
        Settings {
            start_hour: 9,
            end_hour: 21,
            popups_per_day: 2,
            skip_weekend: false,
            ..Default::default()
        }
    }

    #[test]
    fn fires_within_window_before_any_fired() {
        let s = default_settings();
        let next = Scheduler::next_fire_date(monday(9, 0), &s, &[]).unwrap();
        let ts = DateTime::from_timestamp_millis(next).unwrap();
        assert!(ts >= monday(9, 0), "不应早于窗口开始");
        assert!(ts < monday(21, 0), "不应晚于窗口结束");
    }

    #[test]
    fn next_fire_is_after_now_when_called_mid_window() {
        let s = default_settings();
        let now = monday(14, 30);
        let next = Scheduler::next_fire_date(now, &s, &[]).unwrap();
        assert!(next > now.timestamp_millis(), "触发时间应在当前之后");
    }

    #[test]
    fn rolls_to_next_day_when_window_finished() {
        let s = default_settings();
        let now = monday(22, 0);
        let next = Scheduler::next_fire_date(now, &s, &[]).unwrap();
        let ts = DateTime::from_timestamp_millis(next).unwrap();
        assert!(ts >= monday(9, 0) + chrono::Duration::days(1), "应规划到次日");
        assert!(ts < monday(21, 0) + chrono::Duration::days(1), "应在次日窗口内");
    }

    #[test]
    fn skips_weekend_when_configured() {
        let mut s = default_settings();
        s.skip_weekend = true;
        // 2026-10-31 是星期六
        let sat = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        let now = at(sat, 10, 0);
        assert!(
            Scheduler::next_fire_date(now, &s, &[]).is_none(),
            "周末应不排程"
        );
    }

    #[test]
    fn friday_evening_rolls_over_the_weekend_to_monday() {
        let mut s = default_settings();
        s.skip_weekend = true;
        // 2026-10-30 是星期五
        let fri = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let now = at(fri, 22, 0);
        let next = Scheduler::next_fire_date(now, &s, &[]).unwrap();
        let ts = DateTime::from_timestamp_millis(next).unwrap();
        // 下周一 = 2026-11-02
        let monday_next = at(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(), 9, 0);
        assert!(ts >= monday_next, "周五夜里应顺延到下周一");
        assert!(
            ts < monday_next + chrono::Duration::days(1),
            "应落在下周一的时间窗内"
        );
    }

    #[test]
    fn invalid_window_returns_none() {
        let mut s = default_settings();
        s.start_hour = 21;
        s.end_hour = 9;
        assert!(Scheduler::next_fire_date(monday(10, 0), &s, &[]).is_none());
    }

    #[test]
    fn after_firing_all_slots_rolls_forward() {
        let s = default_settings();
        let now = monday(10, 0);
        let fired = vec![now.timestamp_millis()];
        let next = Scheduler::next_fire_date(now, &s, &fired).unwrap();
        assert!(next > now.timestamp_millis());
    }

    #[test]
    fn fired_segment_is_skipped() {
        let s = default_settings();
        let now = monday(10, 0);
        // 第一段是 [09:00, 15:00)
        let fired = vec![now.timestamp_millis()];
        let next = Scheduler::next_fire_date(now, &s, &fired).unwrap();
        let ts = DateTime::from_timestamp_millis(next).unwrap();
        assert!(
            ts >= monday(15, 0),
            "已触发过的段不应再排程，应落到下一段"
        );
    }

    #[test]
    fn fired_today_from_other_days_does_not_count() {
        let s = default_settings();
        // 昨天的时间戳不应影响今天的计数
        let yesterday = monday(10, 0).timestamp_millis() - 86_400_000;
        let next = Scheduler::next_fire_date(monday(9, 0), &s, &[yesterday]).unwrap();
        let ts = DateTime::from_timestamp_millis(next).unwrap();
        assert!(ts >= monday(9, 0) && ts < monday(21, 0));
    }

    #[test]
    fn window_respects_local_timezone() {
        // 时间窗按本地时间计算：9 点本地 = 本地时间戳的 9 点，而不是 UTC 的 9 点
        let s = default_settings();
        let now = monday(8, 0);
        let next = Scheduler::next_fire_date(now, &s, &[]).unwrap();
        let ts = DateTime::from_timestamp_millis(next).unwrap();
        assert!(ts >= monday(9, 0), "不应早于本地 9 点");
    }

    #[test]
    fn parses_common_shortcut_formats() {
        assert!(parse_shortcut("CmdOrCtrl+Shift+L").is_some());
        assert!(parse_shortcut("Ctrl+Alt+K").is_some());
    }

    #[test]
    fn rejects_shortcut_without_modifier() {
        assert!(parse_shortcut("L").is_none(), "无修饰键的组合键应被拒绝");
    }
}
