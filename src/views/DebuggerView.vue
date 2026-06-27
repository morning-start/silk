<script setup lang="ts">
import { ref } from "vue";
import {
  NCard,
  NButton,
  NInput,
  NSelect,
  NRadioGroup,
  NRadio,
  NSpace,
  NTag,
  NText,
  useMessage,
} from "naive-ui";

const message = useMessage();

const baseUrlOptions = [
  { label: "自定义", value: "custom" },
  { label: "https://api.openai.com/v1", value: "openai" },
  { label: "https://api.anthropic.com/v1", value: "anthropic" },
  { label: "https://api.deepseek.com/v1", value: "deepseek" },
  { label: "https://api.moonshot.cn/v1", value: "moonshot" },
];

const baseUrl = ref("custom");
const requestUrl = ref("http://127.0.0.1:2013");
const method = ref("POST");
const protocol = ref("chat");

const headers = ref([
  { key: "Authorization", value: "Bearer sk-..." },
  { key: "Content-Type", value: "application/json" },
]);

const requestBody = ref(`{
  "model": "gpt-4o",
  "messages": [
    { "role": "user", "content": "Hello!" }
  ]
}`);

const responseStatus = ref<number | null>(null);
const responseBody = ref("");
const responseTime = ref("");
const showResponse = ref(false);
const sending = ref(false);

function addHeader() {
  headers.value.push({ key: "", value: "" });
}

function removeHeader(index: number) {
  headers.value.splice(index, 1);
}

async function sendRequest() {
  sending.value = true;
  showResponse.value = false;
  responseStatus.value = null;
  responseBody.value = "";

  try {
    const startTime = performance.now();

    const res = await fetch(requestUrl.value, {
      method: method.value,
      headers: headers.value
        .filter((h) => h.key && h.value)
        .reduce((acc, h) => ({ ...acc, [h.key]: h.value }), {} as Record<string, string>),
      body: method.value === "POST" || method.value === "PUT" ? requestBody.value : undefined,
    });

    const elapsed = Math.round(performance.now() - startTime);
    responseTime.value = `${elapsed}ms`;
    responseStatus.value = res.status;

    const text = await res.text();
    try {
      responseBody.value = JSON.stringify(JSON.parse(text), null, 2);
    } catch {
      responseBody.value = text;
    }

    showResponse.value = true;
  } catch (e: any) {
    message.error("请求失败: " + (e.message || "未知错误"));
    responseStatus.value = 0;
    responseBody.value = e.message || "请求失败";
    showResponse.value = true;
  } finally {
    sending.value = false;
  }
}

function handleBaseUrlChange(val: string) {
  if (val !== "custom") {
    requestUrl.value = val + "/chat/completions";
  }
}
</script>

<template>
  <div class="debugger">
    <h2 class="page-title">API 调试</h2>

    <NCard title="请求构建器" :bordered="false" class="mb-16 section-card" size="small">
      <div class="form-stack">
        <div class="form-row">
          <div class="field" style="flex: 1">
            <label class="field-label">Base URL</label>
            <NSelect
              v-model:value="baseUrl"
              :options="baseUrlOptions"
              @update:value="handleBaseUrlChange"
            />
          </div>
          <div class="field" style="flex: 2">
            <label class="field-label">请求地址</label>
            <NInput v-model:value="requestUrl" placeholder="输入完整的请求 URL" />
          </div>
        </div>

        <div class="form-row">
          <div class="field" style="flex: 0 0 140px">
            <label class="field-label">请求方法</label>
            <NSelect
              v-model:value="method"
              :options="[
                { label: 'GET', value: 'GET' },
                { label: 'POST', value: 'POST' },
                { label: 'PUT', value: 'PUT' },
                { label: 'DELETE', value: 'DELETE' },
              ]"
            />
          </div>
          <div class="field" style="flex: 1">
            <label class="field-label">支持协议</label>
            <NRadioGroup v-model:value="protocol">
              <NSpace>
                <NRadio value="chat">Chat</NRadio>
                <NRadio value="response">Response</NRadio>
                <NRadio value="claude">Anthropic</NRadio>
                <NRadio value="embeddings">Embeddings</NRadio>
              </NSpace>
            </NRadioGroup>
          </div>
        </div>

        <hr class="rule" />

        <div>
          <div class="section-header">
            <strong style="font-size: 13px">请求头</strong>
            <NButton text size="small" @click="addHeader">+ 添加</NButton>
          </div>
          <div class="header-list">
            <div v-for="(hdr, i) in headers" :key="i" class="kv-row">
              <NInput v-model:value="hdr.key" placeholder="Header" style="flex: 1" />
              <NInput v-model:value="hdr.value" placeholder="Value" style="flex: 2" />
              <NButton quaternary size="small" type="error" @click="removeHeader(i)">×</NButton>
            </div>
          </div>
        </div>

        <div style="margin-top: 12px">
          <label class="field-label">请求体</label>
          <NInput
            v-model:value="requestBody"
            type="textarea"
            :rows="8"
            style="margin-top: 6px"
            placeholder="输入 JSON 请求体"
          />
        </div>

        <NButton
          type="primary"
          style="margin-top: 16px"
          @click="sendRequest"
          :loading="sending"
        >
          发送请求
        </NButton>
      </div>
    </NCard>

    <NCard
      v-if="showResponse"
      title="响应"
      :bordered="false"
      class="section-card"
      size="small"
    >
      <template #header-extra>
        <NSpace :size="8" align="center">
          <NTag
            v-if="responseStatus"
            :type="responseStatus && responseStatus < 300 ? 'success' : responseStatus && responseStatus < 500 ? 'warning' : 'error'"
            size="small"
          >
            {{ responseStatus }}
          </NTag>
          <NText depth="3" style="font-size: 12px">耗时 {{ responseTime }}</NText>
        </NSpace>
      </template>
      <NInput
        :value="responseBody"
        type="textarea"
        :rows="12"
        readonly
        placeholder="响应内容"
        style="font-family: 'JetBrains Mono', 'Consolas', monospace; font-size: 12px"
      />
    </NCard>
  </div>
</template>

<style scoped>
.debugger {
  max-width: 960px;
}

.page-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 20px;
}

.mb-16 {
  margin-bottom: 16px;
}

.section-card {
  border-radius: 12px;
}

.form-stack {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.form-row {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
}

.field {
  display: flex;
  flex-direction: column;
}

.field-label {
  font-size: 13px;
  font-weight: 500;
  margin-bottom: 6px;
  color: var(--text-color-2, #475569);
}

.rule {
  border: none;
  border-top: 1px solid var(--border-color, #e2e8f0);
  margin: 16px 0;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.header-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.kv-row {
  display: flex;
  gap: 8px;
  align-items: center;
}
</style>
