// 防止 release 构建时弹出多余的控制台窗口（Windows）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shiyi_lib::run()
}
