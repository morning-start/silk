<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { darkTheme, NConfigProvider, NMessageProvider, NDialogProvider, GlobalThemeOverrides } from "naive-ui";
import AppContent from "./AppContent.vue";

const THEME_STORAGE_KEY = "silk-theme";
const isDark = ref(false);

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#0891b2",
    primaryColorHover: "#0e7490",
    primaryColorPressed: "#0c6a83",
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
});

watch(isDark, (enabled) => {
  applyThemeClass(enabled);
  localStorage.setItem(THEME_STORAGE_KEY, enabled ? "dark" : "light");
});
</script>

<template>
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
