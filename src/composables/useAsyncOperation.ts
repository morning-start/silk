import { ref, type Ref } from "vue";

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
    } catch (e: any) {
      error.value = e?.message || errorMessage || "操作失败";
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
    } catch (e: any) {
      error.value = e?.message || errorMessage || "操作失败";
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return { loading, error, run, runOrThrow };
}

/**
 * 基于 ref 数据数组的异步 store 辅助函数
 * 适用于典型的 CRUD store 模式
 */
export function useAsyncDataList<T extends { id: string }>(
  fetchFn: () => Promise<T[]>
) {
  const { loading, error, run } = useAsyncOperation();
  const data = ref<T[]>([]) as Ref<T[]>;

  async function fetchAll(): Promise<void> {
    const result = await run(fetchFn, "获取数据失败");
    if (result) data.value = result;
  }

  async function create(
    createFn: () => Promise<T>
  ): Promise<T | undefined> {
    const result = await run(createFn, "创建失败");
    if (result) {
      data.value.unshift(result);
    }
    return result;
  }

  async function update(
    id: string,
    updateFn: () => Promise<T>
  ): Promise<T | undefined> {
    const result = await run(updateFn, "更新失败");
    if (result) {
      const idx = data.value.findIndex((item) => item.id === id);
      if (idx >= 0) data.value[idx] = result;
    }
    return result;
  }

  async function remove(
    id: string,
    removeFn: () => Promise<void>
  ): Promise<boolean> {
    const result = await run(removeFn, "删除失败");
    if (result !== undefined) {
      data.value = data.value.filter((item) => item.id !== id);
      return true;
    }
    return false;
  }

  return { data, loading, error, fetchAll, create, update, remove, run };
}