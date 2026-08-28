import { Store } from "@tauri-apps/plugin-store";

let store: Store | null = null;

/**
 * 获取或创建 store 实例
 */
async function getStore(): Promise<Store> {
  if (!store) {
    store = await Store.load("settings.json");
  }
  return store;
}

/**
 * 从 store 读取值
 */
export async function storeGet<T>(key: string): Promise<T | null> {
  const s = await getStore();
  return (await s.get<T>(key)) ?? null;
}

/**
 * 写入值到 store
 */
export async function storeSet<T>(key: string, value: T): Promise<void> {
  const s = await getStore();
  await s.set(key, value);
  await s.save();
}

/**
 * 从 store 删除值
 */
export async function storeDelete(key: string): Promise<void> {
  const s = await getStore();
  await s.delete(key);
  await s.save();
}

/**
 * 检查 store 中是否存在某个 key
 */
export async function storeHas(key: string): Promise<boolean> {
  const s = await getStore();
  return await s.has(key);
}

/**
 * 获取 store 中所有 key
 */
export async function storeKeys(): Promise<string[]> {
  const s = await getStore();
  return await s.keys();
}

/**
 * 获取 store 中所有值
 */
export async function storeEntries<T>(): Promise<[string, T][]> {
  const s = await getStore();
  return (await s.entries()) as [string, T][];
}

/**
 * 清空 store
 */
export async function storeClear(): Promise<void> {
  const s = await getStore();
  await s.clear();
  await s.save();
}
