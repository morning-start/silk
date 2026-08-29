import { invoke } from "@tauri-apps/api/core";
import type { RequestLog } from "./types";

export interface LogsListResult {
  logs: RequestLog[];
  total: number;
  limit: number;
  offset: number;
}

export const logsApi = {
  list: (limit = 50, offset = 0): Promise<LogsListResult> =>
    invoke<LogsListResult>("list_logs", { payload: { limit, offset } }),

  cleanup: (beforeDays: number): Promise<number> =>
    invoke<number>("cleanup_logs", { payload: { beforeDays } }),

  clearAll: (): Promise<number> => invoke<number>("clear_all_logs"),

  exportCsv: (data: {
    provider_id?: string;
    limit?: number;
    file_path?: string;
  }): Promise<{ file_path: string; exported_count: number }> =>
    invoke<{ file_path: string; exported_count: number }>("export_logs_csv", {
      payload: data,
    }),
};
