import { ref } from "vue";

/**
 * 异步操作的状态包装器（loading、error、run）
 * 消除 store 中重复的 loading/error/try/catch 模板代码
 */
export function useAsyncOperation() {
  const loading = ref(false);
  const error = ref<string | null>(null);

  async function run<T>(
    fn: () => Promise<T>,
    errorMessage?: string
  ): Promise<T | undefined> {
    loading.value = true;
    error.value = null;
    try {
      return await fn();
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : errorMessage || "操作失败";
      return undefined;
    } finally {
      loading.value = false;
    }
  }

  /** run() 的变体：失败时继续抛出异常（用于需要调用方处理错误的情况） */
  async function runOrThrow<T>(
    fn: () => Promise<T>,
    errorMessage?: string
  ): Promise<T> {
    loading.value = true;
    error.value = null;
    try {
      return await fn();
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : errorMessage || "操作失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { loading, error, run, runOrThrow };
}