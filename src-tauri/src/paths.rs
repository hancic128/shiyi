use std::path::PathBuf;

/// 环境变量名。设置它可以把应用数据目录指到别处。
///
/// 正常运行时用不上，主要给「跑一份隔离实例」的场景用（比如临时改
/// 一下数据目录做实验）。测试构建下这个变量不生效——测试固定用临时目录。
#[cfg_attr(test, allow(dead_code))]
pub const DATA_DIR_ENV: &str = "SHIYI_DATA_DIR";

/// 应用数据目录。存放 `settings.json`、`data.json` 与 `books/`。
///
/// # 测试隔离
///
/// `cargo test` 会跑一部分用到全局单例（书库 / 收藏 / 设置）的用例。
/// 这些单例走的是 `global()`，默认往真实用户目录里写文件——本地会覆盖
/// 用户自己的收藏和设置，CI 上会污染 runner。所以测试构建下统一改用
/// 「进程专属的临时目录」：
///
/// - 编译期判断（`cfg(test)`），不依赖环境变量的设置时机
/// - 目录名带上进程号，同一次 `cargo test` 里所有用例共享一份数据，
///   但跟用户真实数据、跟别的测试进程都完全隔开
///
/// 非测试构建才走环境变量 → 平台默认目录。
pub fn app_data_dir() -> PathBuf {
    #[cfg(test)]
    {
        return std::env::temp_dir().join(format!("shiyi-test-data-{}", std::process::id()));
    }

    #[cfg(not(test))]
    {
        // 环境变量优先，方便临时指向别的目录
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join("Library/Application Support")
                    .join(crate::APP_IDENTIFIER);
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata).join(crate::APP_IDENTIFIER);
            }
        }
        std::env::temp_dir().join(crate::APP_IDENTIFIER)
    }
}
