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
  log_retention_days: 30,
  default_provider_id: "",
  default_route_id: "",
});

// Gateway Keys
const keys = ref<GatewayKey[]>([]);
const showAddKey = ref(false);
const newKeyName = ref("");
const newKeyMaxConcurrent = ref(10);
const newKeyCreated = ref<{ name: string; key: string } | null>(null);
const showNewKeyModal = ref(false);

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
  if (!newKeyName.value) {
    message.warning("请填写名称");
    return;
  }
  try {
    const result = await api.createGatewayKey({
      name: newKeyName.value,
      key_value: "",  // 空值触发后端自动生成
      max_concurrent: newKeyMaxConcurrent.value,
    });
    message.success("添加成功");
    newKeyCreated.value = {
      name: result.key.name,
      key: result.plain_key,
    };
    showNewKeyModal.value = true;
    newKeyName.value = "";
    newKeyMaxConcurrent.value = 10;
    showAddKey.value = false;
    await loadKeys();
  } catch {
    message.error("添加失败");
  }
}

function copyKey(text: string) {
  navigator.clipboard.writeText(text).then(() => {
    message.success("已复制到剪贴板");
  }).catch(() => {
    message.error("复制失败");
  });
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
        log_retention_days: s.log_retention_days,
        default_provider_id: s.default_provider_id || "",
        default_route_id: s.default_route_id || "",
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
          <NInputNumber v-model:value="newKeyMaxConcurrent" :min="1" :max="1000" placeholder="并发数" style="flex: 0 0 100px" />
          <NButton type="primary" size="small" @click="addKey">生成</NButton>
          <NButton size="small" @click="showAddKey = false">取消</NButton>
        </div>
        <div style="font-size:12px;color:var(--text-color-3,#94a3b8);margin-top:4px">
          Key 值由系统自动生成（<code>sk-gw-xxx</code> 格式），创建后仅展示一次
        </div>
      </div>

      <!-- 新 Key 展示 -->
      <NModal
        v-model:show="showNewKeyModal"
        preset="card"
        title="Key 已创建"
        style="max-width: 480px"
        :bordered="false"
        @update:show="(val: boolean) => { if (!val) newKeyCreated = null; }"
      >
        <div v-if="newKeyCreated" style="display:flex;flex-direction:column;gap:12px">
          <div style="font-size:13px">名称: <strong>{{ newKeyCreated.name }}</strong></div>
          <div style="font-size:13px;margin-bottom:4px">Key 值（请立即复制，关闭后不再显示）:</div>
          <div
            style="
              font-family:'JetBrains Mono','Consolas',monospace;
              font-size:14px;
              background:#1e293b;
              color:#e2e8f0;
              padding:12px 16px;
              border-radius:8px;
              word-break:break-all;
              cursor:pointer;
              user-select:all;
            "
            @click="copyKey(newKeyCreated.key)"
          >
            {{ newKeyCreated.key }}
          </div>
          <NButton size="small" @click="copyKey(newKeyCreated.key)">复制 Key</NButton>
          <NButton size="small" type="primary" @click="newKeyCreated = null">我已保存，关闭</NButton>
        </div>
      </NModal>

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
