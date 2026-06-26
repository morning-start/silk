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
  useMessage,
  NCard,
  NSpace,
} from "naive-ui";
import { useGatewayStore } from "../stores/gateway";
import { storeToRefs } from "pinia";

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
});

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
      };
    }
  },
  { immediate: true }
);

onMounted(() => {
  gatewayStore.fetchStatus();
});
</script>

<template>
  <div>
    <n-text style="font-size: 18px; font-weight: 600">网关设置</n-text>

    <n-card style="margin-top: 16px" title="网络配置">
      <n-form ref="formRef" :model="formValue" label-placement="left" label-width="140">
        <n-form-item label="监听地址">
          <n-input v-model:value="formValue.bind_host" placeholder="127.0.0.1" />
        </n-form-item>
        <n-form-item label="监听端口">
          <n-input-number v-model:value="formValue.bind_port" :min="1" :max="65535" style="width: 100%" />
        </n-form-item>
        <n-form-item label="允许远程访问">
          <n-switch v-model:value="formValue.allow_remote" />
        </n-form-item>
        <n-form-item label="网关认证 Token">
          <n-input v-model:value="formValue.auth_token_hash" placeholder="可选，设置后请求需携带此 Token" type="password" show-password-on="click" />
        </n-form-item>
      </n-form>
    </n-card>

    <n-card style="margin-top: 16px" title="日志配置">
      <n-form :model="formValue" label-placement="left" label-width="140">
        <n-form-item label="日志保留天数">
          <n-input-number v-model:value="formValue.log_retention_days" :min="1" :max="3650" style="width: 100%" />
        </n-form-item>
      </n-form>
    </n-card>

    <div style="margin-top: 16px">
      <n-space>
        <n-button type="primary" :loading="loading" @click="handleSave">保存设置</n-button>
      </n-space>
    </div>
  </div>
</template>
