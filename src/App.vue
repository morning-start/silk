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
    primaryColor: "#0891b2",
    primaryColorHover: "#0e7490",
    primaryColorPressed: "#0c6a83",
    // 次要强调：靛蓝（信息/链接/选中），与青色主强调互补
    infoColor: "#6366f1",
    infoColorHover: "#4f46e5",
    infoColorPressed: "#4338ca",
    borderRadius: "8px",
    bodyColor: "#f8fafc",
    cardColor: "#ffffff",
    modalColor: "#ffffff",
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
    borderRadius: "8px",
    bodyColor: "#0b0f19",
    cardColor: "#131924",
    modalColor: "#131924",
    borderColor: "#242f41",
    dividerColor: "#242f41",
    textColor1: "#f1f5f9",
    textColor2: "#cbd5e1",
    textColor3: "#94a3b8",
    placeholderColor: "#475569",
    popoverColor: "#131924",
    hoverColor: "#1e293b",
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
