/**
 * Orbis 应用根：视图路由 + 全局初始化 + 全局提示 + 模态
 *
 * 初始化顺序：订阅事件 → 拉游戏目录与安装列表 → 拉工具与设置。
 * 所有数据访问经 store，store 经 src/api（00 §7.9）。
 */
import React, { useEffect } from 'react';
import { useAppStore } from './store/useAppStore';
import { useGamesStore } from './store/useGamesStore';
import { useSettingsStore } from './store/useSettingsStore';
import { useToolsStore } from './store/useToolsStore';
import { Header } from './components/layout/Header';
import { NoticeBar } from './components/common/NoticeBar';
import { OnboardingView } from './views/OnboardingView';
import { HomeView } from './views/HomeView';
import { WuwaDashboardView } from './views/WuwaDashboardView';
import { GenshinDashboardView } from './views/GenshinDashboardView';
import { GenericDashboardView } from './views/GenericDashboardView';
import { ConfigHistoryView } from './views/ConfigHistoryView';
import { SettingsView } from './views/SettingsView';
import { L3ConsentModal } from './components/modals/L3ConsentModal';
import { ExecutionFlowModal } from './components/modals/ExecutionFlowModal';
import { TerminateConfirmModal } from './components/modals/TerminateConfirmModal';

export const App: React.FC = () => {
  const activeView = useAppStore((s) => s.activeView);

  useEffect(() => {
    // 事件订阅与首屏数据（StrictMode 下由 store 内部幂等保护）
    void useGamesStore.getState().initialize();
    void useToolsStore.getState().initialize();
    void useSettingsStore.getState().load();
  }, []);

  return (
    <div className="min-h-screen bg-bg-base text-on-surface font-body-md relative overflow-x-hidden selection:bg-primary-container selection:text-bg-base">
      {activeView !== 'onboarding' && <Header />}
      <NoticeBar />

      <div className="w-full">
        {activeView === 'onboarding' && <OnboardingView />}
        {activeView === 'home' && <HomeView />}
        {activeView === 'wuthering-waves' && <WuwaDashboardView />}
        {activeView === 'genshin-impact' && <GenshinDashboardView />}
        {activeView === 'generic' && <GenericDashboardView />}
        {activeView === 'config-history' && <ConfigHistoryView />}
        {activeView === 'settings' && <SettingsView />}
      </div>

      <L3ConsentModal />
      <ExecutionFlowModal />
      <TerminateConfirmModal />
    </div>
  );
};

export default App;
