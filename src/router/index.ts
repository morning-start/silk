import { createRouter, createWebHashHistory } from "vue-router";
import DashboardView from "../views/DashboardView.vue";
import ProvidersView from "../views/ProvidersView.vue";
import GroupsView from "../views/GroupsView.vue";
import ModelSquareView from "../views/ModelSquareView.vue";
import LogsView from "../views/LogsView.vue";
import MonitoringView from "../views/MonitoringView.vue";
import AnalyticsView from "../views/AnalyticsView.vue";
import SettingsView from "../views/SettingsView.vue";
import DebuggerView from "../views/DebuggerView.vue";

const routes = [
  { path: "/", redirect: "/dashboard" },
  { path: "/dashboard", name: "dashboard", component: DashboardView, meta: { title: "仪表盘" } },
  { path: "/providers", name: "providers", component: ProvidersView, meta: { title: "渠道管理" } },
  { path: "/groups", name: "groups", component: GroupsView, meta: { title: "负载均衡分组" } },
  { path: "/model-square", name: "model-square", component: ModelSquareView, meta: { title: "模型池" } },
  { path: "/logs", name: "logs", component: LogsView, meta: { title: "请求日志" } },
  { path: "/monitoring", name: "monitoring", component: MonitoringView, meta: { title: "实时监控" } },
  { path: "/analytics", name: "analytics", component: AnalyticsView, meta: { title: "用量分析" } },
  { path: "/settings", name: "settings", component: SettingsView, meta: { title: "系统设置" } },
  { path: "/debugger", name: "debugger", component: DebuggerView, meta: { title: "API 调试" } },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
