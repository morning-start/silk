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

/** 形状类令牌：与 style.css 的 :root 对应 */
const baseOverrides = {
  fontFamily: FONT_SANS,
  fontSize: "13.5px",
  fontSizeSmall: "12.5px",
  heightSmall: "26px",
  heightMedium: "32px",
  heightLarge: "38px",
};

/* ----------------------------------------------------------------
   以下色值与 style.css 的 :root / body.dark 一一对应。
   naive-ui 需要字面量色值，无法直接吃 CSS 变量，所以这里是唯一
   允许与令牌重复的地方 —— 改色时必须两边同改，否则原生控件
   （输入框 / 表格 / 弹窗 / 分页）会和自定义 CSS 出现两套色系。
   ---------------------------------------------------------------- */

/** 浅色：--bg --surface --surface-alt --fg --fg-2 --muted --border --border-soft --hover-bg */
const themeOverrides: GlobalThemeOverrides = {
  common: {
    ...baseOverrides,
    primaryColor: "#0e7490",
    primaryColorHover: "#0b6579",
    primaryColorPressed: "#0a5568",
    infoColor: "#4f46e5",
    infoColorHover: "#4338ca",
    infoColorPressed: "#3730a3",
    successColor: "#16a34a",
    warningColor: "#d97706",
    errorColor: "#dc2626",
    borderRadius: "8px",
    borderRadiusSmall: "6px",
    bodyColor: "#fafafa",
    cardColor: "#ffffff",
    modalColor: "#ffffff",
    popoverColor: "#ffffff",
    tableColor: "#ffffff",
    tableHeaderColor: "#f5f5f5",
    borderColor: "#e5e5e5",
    dividerColor: "#ededed",
    textColor1: "#0a0a0a",
    textColor2: "#171717",
    textColor3: "#737373",
    textColorDisabled: "#a3a3a3",
    placeholderColor: "#a3a3a3",
    hoverColor: "#f5f5f5",
  },
};

/** 深色：--bg --surface --surface-alt --fg --fg-2 --muted --border --border-soft --hover-bg */
const darkOverrides: GlobalThemeOverrides = {
  common: {
    ...baseOverrides,
    primaryColor: "#22d3ee",
    primaryColorHover: "#67e8f9",
    primaryColorPressed: "#06b6d4",
    infoColor: "#818cf8",
    infoColorHover: "#a5b4fc",
    infoColorPressed: "#6366f1",
    successColor: "#62d178",
    warningColor: "#fbbf24",
    errorColor: "#ff6166",
    borderRadius: "8px",
    borderRadiusSmall: "6px",
    bodyColor: "#0a0a0a",
    cardColor: "#171717",
    modalColor: "#171717",
    popoverColor: "#171717",
    tableColor: "#171717",
    tableHeaderColor: "#262626",
    borderColor: "#282828",
    dividerColor: "#202020",
    textColor1: "#fafafa",
    textColor2: "#e5e5e5",
    textColor3: "#a1a1a1",
    textColorDisabled: "#525252",
    placeholderColor: "#525252",
    hoverColor: "#262626",
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
