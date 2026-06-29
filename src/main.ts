import { createApp } from "vue";
import { createPinia } from "pinia";
import router from "./router";
import App from "./App.vue";
import "./style.css";

const app = createApp(App);
const pinia = createPinia();

// 全局错误边界：防止组件渲染异常导致白屏
app.config.errorHandler = (err, _instance, info) => {
  console.error("[Silk] 全局错误:", err, info);
  // 不抛出异常，避免整个应用崩溃
};

app.use(pinia);
app.use(router);
app.mount("#app");
