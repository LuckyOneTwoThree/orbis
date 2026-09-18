/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 强制 API 实现：'mock' 时即使运行在 Tauri 中也走 mock（对比调试用） */
  readonly VITE_ORBIS_API?: 'mock' | 'tauri';
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
