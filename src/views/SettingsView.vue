<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import {
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NSwitch,
  NButton,
  NText,
  NTag,
  NCard,
  NSpace,
  NSelect,
  useMessage,
} from "naive-ui";
import { useGatewayStore } from "../stores/gateway";
import { storeToRefs } from "pinia";
import { api, type GatewayKey } from "../api";

const gatewayStore = useGatewayStore();
const { status, loading } = storeToRefs(gatewayStore);
const message = useMessage();

const formRef = ref<any>(null);
const formValue = ref({
  bind_host: "127.0.0.1",
  bind_port: 2013,
  allow_remote: false,
  auth_token_hash: "",
  log_retention_days: 30,
  default_provider_id: "",
  default_route_id: "",
  rate_limit_enabled: true,
  rate_limit_max_requests_per_minute: 1000,
  rate_limit_max_tokens_per_minute: 10000000,
});

// Gateway Keys
const keys = ref<GatewayKey[]>([]);
const showAddKey = ref(false);
const newKeyName = ref("");
const newKeyValue = ref("");
const newKeyMaxConcurrent = ref(10);

async function loadKeys() {
  try {
    keys.value = await api.listGatewayKeys();
  } catch {
    // ignore
  }
}

async function toggleKey(key: GatewayKey) {
  try {
    await api.updateGatewayKey(key.id, { enabled: !key.enabled });
    message.success(key.enabled ? "已禁用" : "已启用");
    await loadKeys();
  } catch {
    message.error("操作失败");
  }
}

async function deleteKey(id: string) {
  try {
    await api.deleteGatewayKey(id);
    keys.value = keys.value.filter((k) => k.id !== id);
    message.success("已删除");
  } catch {
    message.error("删除失败");
  }
}

async function addKey() {
  if (!newKeyName.value || !newKeyValue.value) {
    message.warning("请填写名称和 Key");
    return;
  }
  try {
    await api.createGatewayKey({
      name: newKeyName.value,
      key_value: newKeyValue.value,
      max_concurrent: newKeyMaxConcurrent.value,
    });
    message.success("添加成功");
    newKeyName.value = "";
    newKeyValue.value = "";
    newKeyMaxConcurrent.value = 10;
    showAddKey.value = false;
    await loadKeys();
  } catch {
    message.error("添加失败");
  }
}

async function handleSave() {
  try {
    await gatewayStore.updateSettings(formValue.value);
    message.success("设置已保存");
  } catch {
    message.error("保存失败");
  }
}

watch(
  status,
  (newStatus) => {
    if (newStatus?.settings) {
      const s = newStatus.settings;
      formValue.value = {
        bind_host: s.bind_host,
        bind_port: s.bind_port,
        allow_remote: s.allow_remote,
        auth_token_hash: s.auth_token_hash || "",
        log_retention_days: s.log_retention_days,
        default_provider_id: s.default_provider_id || "",
        default_route_id: s.default_route_id || "",
        rate_limit_enabled: s.rate_limit_enabled,
        rate_limit_max_requests_per_minute: s.rate_limit_max_requests_per_minute,
        rate_limit_max_tokens_per_minute: s.rate_limit_max_tokens_per_minute,
      };
    }
  },
  { immediate: true }
);

onMounted(() => {
  gatewayStore.fetchStatus();
  loadKeys();
});
</script>

<template>
  <div class="settings-page">
    <div class="toolbar">
      <div class="toolbar-left">
        <h2 class="page-title">系统设置</h2>
      </div>
      <div class="toolbar-right">
        <NButton type="primary" size="small" @click="handleSave" :loading="loading">保存更改</NButton>
      </div>
    </div>

    <!-- 网关基础 -->
    <NCard :bordered="false" class="settings-card" size="small" title="网关基础">
      <NForm ref="formRef" :model="formValue" label-placement="left" label-width="100">
        <div class="form-row">
          <NFormItem label="监听地址" style="flex: 1">
            <NInput v-model:value="formValue.bind_host" placeholder="127.0.0.1" />
          </NFormItem>
          <NFormItem label="监听端口" style="flex: 1">
            <NInputNumber v-model:value="formValue.bind_port" :min="1" :max="65535" style="width: 100%" />
          </NFormItem>
        </div>
        <div class="form-row">
          <NFormItem label="允许远程访问" style="flex: 1">
            <NSwitch v-model:value="formValue.allow_remote" />
          </NFormItem>
          <NFormItem label="日志保留天数" style="flex: 1">
            <NInputNumber v-model:value="formValue.log_retention_days" :min="1" :max="3650" style="width: 100%" />
          </NFormItem>
        </div>
        <NFormItem label="网关认证 Token">
          <NInput
            v-model:value="formValue.auth_token_hash"
            placeholder="可选，设置后请求需携带此 Token"
            type="password"
            show-password-on="click"
          />
        </NFormItem>
      </NForm>
    </NCard>

    <!-- 限流配额 -->
    <NCard :bordered="false" class="settings-card" size="small" title="限流配额">
      <NForm :model="formValue" label-placement="left" label-width="100">
        <div class="form-row">
          <NFormItem label="每分钟请求上限" style="flex: 1">
            <NInputNumber v-model:value="formValue.rate_limit_max_requests_per_minute" :min="1" :max="100000" style="width: 100%" />
          </NFormItem>
          <NFormItem label="每分钟 Token 上限" style="flex: 1">
            <NInputNumber v-model:value="formValue.rate_limit_max_tokens_per_minute" :min="1" :max="100000000" style="width: 100%" />
          </NFormItem>
        </div>
        <NFormItem label="启用限流">
          <NSwitch v-model:value="formValue.rate_limit_enabled" />
        </NFormItem>
      </NForm>
    </NCard>

    <!-- Key 管理 -->
    <NCard :bordered="false" class="settings-card" size="small" title="Key 管理">
      <template #header-extra>
        <NButton size="small" @click="showAddKey = !showAddKey">+ 添加 Key</NButton>
      </template>

      <div v-if="showAddKey" class="add-key-box">
        <div class="form-row" style="margin-bottom: 8px">
          <NInput v-model:value="newKeyName" placeholder="名称" style="flex: 1" />
          <NInput v-model:value="newKeyValue" placeholder="Key 值 (sk-...)" style="flex: 2" />
          <NInputNumber v-model:value="newKeyMaxConcurrent" :min="1" :max="1000" placeholder="并发数" style="flex: 0 0 100px" />
          <NButton type="primary" size="small" @click="addKey">确定</NButton>
          <NButton size="small" @click="showAddKey = false">取消</NButton>
        </div>
      </div>

      <div class="keys-list">
        <div v-for="key in keys" :key="key.id" class="key-row">
          <div class="key-info">
            <span class="key-name">{{ key.name }}</span>
            <NTag size="small" style="font-family: 'JetBrains Mono', 'Consolas', monospace">
              {{ key.key_prefix }}****
            </NTag>
            <NTag :type="key.enabled ? 'success' : 'warning'" size="small">
              {{ key.enabled ? '启用' : '禁用' }}
            </NTag>
            <span class="key-concurrent" v-if="key.max_concurrent">并发: {{ key.max_concurrent }}</span>
          </div>
          <NSpace :size="4">
            <NButton size="tiny" quaternary @click="toggleKey(key)">
              {{ key.enabled ? '禁用' : '启用' }}
            </NButton>
            <NButton size="tiny" quaternary type="error" @click="deleteKey(key.id)">删除</NButton>
          </NSpace>
        </div>
        <NText v-if="keys.length === 0" depth="3" style="display: block; text-align: center; padding: 24px">
          暂无 Key，点击上方"+ 添加 Key"创建
        </NText>
      </div>
    </NCard>

    <!-- 日志规则 -->
    <NCard :bordered="false" class="settings-card" size="small" title="日志规则">
      <NForm :model="formValue" label-placement="left" label-width="100">
        <NFormItem label="日志保留天数" style="max-width: 360px">
          <NInputNumber v-model:value="formValue.log_retention_days" :min="1" :max="3650" style="width: 100%" />
        </NFormItem>
        <NFormItem label="自动清理">
          <NSwitch :value="true" />
          <span style="margin-left: 8px; font-size: 13px; color: var(--text-color-3, #94a3b8)">自动清理超过保留天数的日志</span>
        </NFormItem>
      </NForm>
    </NCard>

    <!-- 全局负载均衡 -->
    <NCard :bordered="false" class="settings-card" size="small" title="全局负载均衡">
      <NForm :model="formValue" label-placement="left" label-width="100">
        <div class="form-row">
          <NFormItem label="默认调度策略" style="flex: 1">
            <NSelect
              :value="'weighted_round_robin'"
              :options="[
                { label: '加权轮询', value: 'weighted_round_robin' },
                { label: '轮询', value: 'round_robin' },
                { label: '最少连接', value: 'least_conn' },
              ]"
            />
          </NFormItem>
          <NFormItem label="健康检查间隔" style="flex: 1">
            <NSelect
              :value="'30s'"
              :options="[
                { label: '10 秒', value: '10s' },
                { label: '30 秒', value: '30s' },
                { label: '1 分钟', value: '1m' },
                { label: '5 分钟', value: '5m' },
              ]"
            />
          </NFormItem>
        </div>
        <div class="form-row">
          <NFormItem label="失败切换阈值" style="flex: 1">
            <NInputNumber :value="3" :min="1" :max="20" style="width: 100%" />
          </NFormItem>
          <NFormItem label="恢复检测次数" style="flex: 1">
            <NInputNumber :value="2" :min="1" :max="20" style="width: 100%" />
          </NFormItem>
        </div>
        <NText depth="3" style="font-size: 13px; display: block; margin-top: 8px">
          此设置为全局默认，可在<strong>模型池</strong>中为单个模型覆盖调度策略和健康检查配置。
        </NText>
      </NForm>
    </NCard>
  </div>
</template>

<style scoped>
.settings-page {
  max-width: 1200px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toolbar-right {
  display: flex;
  align-items: center;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.settings-card {
  border-radius: 12px;
  margin-bottom: 16px;
}

.form-row {
  display: flex;
  gap: 12px;
}

.add-key-box {
  background: var(--card-color-alt, #f8fafc);
  border-radius: 8px;
  padding: 16px;
  margin-bottom: 16px;
  border: 1px solid var(--border-color, #e2e8f0);
}

.keys-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.key-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--border-color, #e2e8f0);
  transition: background 0.2s;
}

.key-row:hover {
  background: var(--hover-color, #f8fafc);
}

.key-info {
  display: flex;
  align-items: center;
  gap: 8px;
}

.key-name {
  font-weight: 600;
  font-size: 13px;
  min-width: 60px;
}

.key-concurrent {
  font-size: 12px;
  color: var(--text-color-3, #94a3b8);
}
</style>
