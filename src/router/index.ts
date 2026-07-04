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
  {
    path: "/routing-rules",
    name: "routing-rules",
    component: () => import("../views/RoutingRulesView.vue"),
    meta: { title: "路由" },
  },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
