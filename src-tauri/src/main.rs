// Windows 发布构建不弹控制台窗口（debug 构建保留，便于看启动自检输出）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    orbis_lib::run();
}
