//! Tauri 构建脚本。
//!
//! 由 `tauri-build` 完成：读取 `tauri.conf.json`、校验 capabilities 权限、
//! 生成权限 schema 到 `gen/schemas/`、并把配置注入编译期常量。
//!
//! 注意：`gen/` 是构建产物，已在 .gitignore 中排除。

fn main() {
    tauri_build::build()
}
