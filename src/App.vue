<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { darkTheme, NConfigProvider, NMessageProvider, NDialogProvider, GlobalThemeOverrides } from "naive-ui";
import AppContent from "./AppContent.vue";
import SplashScreen from "./components/SplashScreen.vue";
import { initBackendEvents } from "./composables/useBackendEvents";

const THEME_STORAGE_KEY = "silk-theme";
const isDark = ref(false);
const showSplash = ref(true);

const FONT_SANS =
  "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', Roboto, sans-serif";

/** 基础令牌：与 style.css 的 :root 保持一致，形状类由 CSS 变量驱动 */
const baseOverrides = {
  fontFamily: FONT_SANS,
  fontSize: "13px",
  fontSizeSmall: "12px",
  heightSmall: "26px",
  heightMedium: "32px",
  heightLarge: "38px",
  successColor: "#0f9b6c",
  warningColor: "#d97706",
  errorColor: "#dc2626",
};

const themeOverrides: GlobalThemeOverrides = {
  common: {
    ...baseOverrides,
    primaryColor: "#0e7490",
    primaryColorHover: "#0b6579",
    primaryColorPressed: "#0a5568",
    infoColor: "#4f46e5",
    infoColorHover: "#4338ca",
    infoColorPressed: "#3730a3",
    borderRadius: "8px",
    borderRadiusSmall: "6px",
    bodyColor: "#f7f8fa",
    cardColor: "#ffffff",
    modalColor: "#ffffff",
    popoverColor: "#ffffff",
    tableColor: "#ffffff",
    tableHeaderColor: "#f2f4f7",
    borderColor: "#e2e6eb",
    dividerColor: "#eceff2",
    textColor1: "#10141a",
    textColor2: "#3f4652",
    textColor3: "#6b7480",
    textColorDisabled: "#a3abb6",
    placeholderColor: "#9aa3b0",
    hoverColor: "#f3f5f8",
  },
};

const darkOverrides: GlobalThemeOverrides = {
  common: {
    ...baseOverrides,
    successColor: "#34d399",
    warningColor: "#fbbf24",
    errorColor: "#f87171",
    primaryColor: "#22d3ee",
    primaryColorHover: "#67e8f9",
    primaryColorPressed: "#06b6d4",
    infoColor: "#818cf8",
    infoColorHover: "#a5b4fc",
    infoColorPressed: "#6366f1",
    borderRadius: "8px",
    borderRadiusSmall: "6px",
    bodyColor: "#0b0e14",
    cardColor: "#121722",
    modalColor: "#121722",
    popoverColor: "#161d29",
    tableColor: "#121722",
    tableHeaderColor: "#171d29",
    borderColor: "#232c3b",
    dividerColor: "#1a2130",
    textColor1: "#e8edf4",
    textColor2: "#b7c1cf",
    textColor3: "#7f8b9c",
    textColorDisabled: "#5b6675",
    placeholderColor: "#5b6675",
    hoverColor: "#171d29",
  },
};

function applyThemeClass(enabled: boolean) {
  document.body.classList.toggle("dark", enabled);
}

onMounted(() => {
  const savedTheme = localStorage.getItem(THEME_STORAGE_KEY);
  isDark.value = savedTheme === "dark";
  applyThemeClass(isDark.value);
  // 订阅后端 data-changed 事件，桥接为前端跨 Store 失效信号
  initBackendEvents();
});

watch(isDark, (enabled) => {
  applyThemeClass(enabled);
  localStorage.setItem(THEME_STORAGE_KEY, enabled ? "dark" : "light");
});
</script>

<template>
  <SplashScreen :visible="showSplash" @complete="showSplash = false" />

  <NConfigProvider
    :theme="isDark ? darkTheme : null"
    :theme-overrides="isDark ? darkOverrides : themeOverrides"
    :inline-theme-disabled="false"
  >
    <NMessageProvider>
      <NDialogProvider>
        <AppContent :is-dark="isDark" @toggle-theme="isDark = !isDark" />
      </NDialogProvider>
    </NMessageProvider>
  </NConfigProvider>
</template>

<style>
html, body, #app {
  height: 100%;
  width: 100%;
  overflow: hidden;
}
</style>
