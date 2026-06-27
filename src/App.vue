<script setup lang="ts">
import { ref } from "vue";
import { darkTheme, NConfigProvider, NMessageProvider, NDialogProvider, GlobalThemeOverrides } from "naive-ui";
import AppContent from "./AppContent.vue";

const isDark = ref(false);

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#6366f1",
    primaryColorHover: "#818cf8",
    primaryColorPressed: "#4f46e5",
    bodyColor: "#f8fafc",
    cardColor: "#ffffff",
    modalColor: "#ffffff",
  },
};

const darkOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#818cf8",
    primaryColorHover: "#a5b4fc",
    primaryColorPressed: "#6366f1",
    bodyColor: "#0f172a",
    cardColor: "#1e293b",
    modalColor: "#1e293b",
    borderColor: "#334155",
    dividerColor: "#334155",
    textColor1: "#f1f5f9",
    textColor2: "#cbd5e1",
    textColor3: "#64748b",
    placeholderColor: "#475569",
    popoverColor: "#1e293b",
    hoverColor: "#334155",
  },
};
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