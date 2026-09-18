/**
 * Orbis 入口。
 *
 * 字体在 src/styles/fonts.css 里以 @font-face 自托管声明（woff2-only，只取 latin 子集），
 * 不使用 index.html 的远程 link：
 *  - 离线可用是硬要求（pm/02 §5）：远程字体在断网 / 内网环境下会让 Material Symbols
 *    的字形加载失败，图标退化成图标名文字，界面直接不可用
 *  - 中文由系统字体兜底（PingFang SC / MiSans / Microsoft YaHei，见 tailwind.config.js）
 *  - 也正因如此，Tauri 的 CSP 可以保持严格（无任何外部来源）
 */
import './styles/fonts.css';

import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
