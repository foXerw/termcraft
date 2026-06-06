import React from "react"; // eslint-disable-line @typescript-eslint/no-unused-vars
import { ConfigProvider, theme } from "antd";
import zhCN from 'antd/locale/zh_CN';
import AppLayout from "./components/layout/AppLayout";
import { useAppStore } from "./stores/appStore";

function App() {
  const settings = useAppStore((s) => s.settings);
  const isDark = settings.theme === "dark";

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        algorithm: isDark ? theme.darkAlgorithm : theme.defaultAlgorithm,
        token: {
          colorPrimary: "#0078d4",
          borderRadius: 4,
        },
      }}
    >
      <div data-theme={settings.theme}>
        <AppLayout />
      </div>
    </ConfigProvider>
  );
}

export default App;