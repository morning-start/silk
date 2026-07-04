import { createRouter, createWebHashHistory } from "vue-router";
import DashboardView from "../views/DashboardView.vue";
import ProvidersView from "../views/ProvidersView.vue";
import ModelSquareView from "../views/ModelSquareView.vue";
import LogsView from "../views/LogsView.vue";
import SettingsView from "../views/SettingsView.vue";
import RoutingRulesView from "../views/RoutingRulesView.vue";

const routes = [
  { path: "/", redirect: "/dashboard" },
  { path: "/dashboard", name: "dashboard", component: DashboardView, meta: { title: "仪表盘" } },
  { path: "/providers", name: "providers", component: ProvidersView, meta: { title: "渠道" } },
  { path: "/model-square", name: "model-square", component: ModelSquareView, meta: { title: "模型" } },
  { path: "/logs", name: "logs", component: LogsView, meta: { title: "请求日志" } },
  { path: "/settings", name: "settings", component: SettingsView, meta: { title: "设置" } },
  { path: "/routing-rules", name: "routing-rules", component: RoutingRulesView, meta: { title: "路由" } },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
