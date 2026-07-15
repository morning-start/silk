import { createRouter, createWebHashHistory } from "vue-router";

const routes = [
  { path: "/", redirect: "/dashboard" },
  {
    path: "/dashboard",
    name: "dashboard",
    component: () => import("../views/DashboardView.vue"),
    meta: { title: "仪表盘" },
  },
  {
    path: "/providers",
    name: "providers",
    component: () => import("../views/ProvidersView.vue"),
    meta: { title: "渠道" },
  },
  {
    path: "/model-square",
    name: "model-square",
    component: () => import("../views/ModelSquareView.vue"),
    meta: { title: "模型" },
  },
  {
    path: "/agent-profiles",
    name: "agent-profiles",
    component: () => import("../views/AgentProfilesView.vue"),
    meta: { title: "预设" },
  },
  {
    path: "/logs",
    name: "logs",
    component: () => import("../views/LogsView.vue"),
    meta: { title: "请求日志" },
  },
  {
    path: "/settings",
    name: "settings",
    component: () => import("../views/SettingsView.vue"),
    meta: { title: "设置" },
  },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
