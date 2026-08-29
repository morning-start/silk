import { invoke } from "@tauri-apps/api/core";
import type {
  DashboardStats,
  ProviderStats,
  HourlyStats,
  RequestLog,
} from "./types";

export const statsApi = {
  dashboard: (): Promise<DashboardStats> => invoke<DashboardStats>("dashboard_stats"),

  recentRequests: (limit = 20): Promise<RequestLog[]> =>
    invoke<RequestLog[]>("recent_requests", { limit }),

  byProvider: (limit = 10): Promise<ProviderStats[]> =>
    invoke<ProviderStats[]>("stats_by_provider", { limit }),

  hourly: (hours = 24): Promise<HourlyStats[]> =>
    invoke<HourlyStats[]>("hourly_stats", { hours }),
};
