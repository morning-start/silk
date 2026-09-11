<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { darkTheme, NConfigProvider, NMessageProvider, NDialogProvider, GlobalThemeOverrides } from "naive-ui";
import AppContent from "./AppContent.vue";
import SplashScreen from "./components/SplashScreen.vue";
import { initBackendEvents } from "./composables/useBackendEvents";

const THEME_STORAGE_KEY = "silk-theme";
const isDark = ref(false);
const showSplash = ref(true);

const themeOverrides: GlobalThemeOverrides = {
  common: {
    // 渐变主视觉：主按钮/选中态用青，次要强调用靛（互补双色）
    primaryColor: "#0891b2",
    primaryColorHover: "#0e7490",
    primaryColorPressed: "#0c6a83",
    infoColor: "#6366f1",
    infoColorHover: "#4f46e5",
    infoColorPressed: "#4338ca",
    borderRadius: "10px",
    bodyColor: "#f4f7fb",
    cardColor: "rgba(255,255,255,0.72)",
    modalColor: "rgba(255,255,255,0.86)",
    popoverColor: "rgba(255,255,255,0.86)",
  },
};

const darkOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#06b6d4",
    primaryColorHover: "#22d3ee",
    primaryColorPressed: "#0891b2",
    infoColor: "#818cf8",
    infoColorHover: "#a5b4fc",
    infoColorPressed: "#6366f1",
    borderRadius: "10px",
    bodyColor: "#070b14",
    cardColor: "rgba(16,24,38,0.66)",
    modalColor: "rgba(16,24,38,0.82)",
    popoverColor: "rgba(16,24,38,0.82)",
    borderColor: "#26334a",
    dividerColor: "#26334a",
    textColor1: "#eef4fb",
    textColor2: "#c6d2e4",
    textColor3: "#8b9bb4",
    placeholderColor: "#475569",
    hoverColor: "#1b2639",
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
