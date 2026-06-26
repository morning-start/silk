import { createRouter, createWebHistory } from "vue-router";
import ProvidersView from "../views/ProvidersView.vue";
import GroupsView from "../views/GroupsView.vue";
import RoutingRulesView from "../views/RoutingRulesView.vue";
import LogsView from "../views/LogsView.vue";
import SettingsView from "../views/SettingsView.vue";

const routes = [
  { path: "/", redirect: "/providers" },
  { path: "/providers", name: "providers", component: ProvidersView },
  { path: "/groups", name: "groups", component: GroupsView },
  { path: "/routing-rules", name: "routing-rules", component: RoutingRulesView },
  { path: "/logs", name: "logs", component: LogsView },
  { path: "/settings", name: "settings", component: SettingsView },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;
