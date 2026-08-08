# Phase 2 - 批次1：启动流程与引导系统

## 📋 批次概述

**批次目标**：创建启动画面、首次引导页、自动检测AI应用功能

**预计工期**：1周（5个工作日）

**依赖关系**：Phase 1 完成

---

## 🎯 批次目标

1. 创建启动画面组件
2. 创建首次引导页组件
3. 实现自动检测已安装AI应用
4. 实现一键配置功能

---

## 📝 任务清单

### 任务1.1：创建启动画面组件

**文件路径**：`src/components/SplashScreen.vue`

**任务描述**：
显示品牌Logo、加载进度、版本信息

**实现步骤**：

1. 创建启动画面组件
```vue
<!-- src/components/SplashScreen.vue -->
<template>
  <div class="splash-screen" v-if="visible">
    <div class="splash-content">
      <div class="logo-container">
        <img src="@/assets/logo.svg" alt="Silk Logo" class="logo" />
        <h1 class="app-name">Silk</h1>
        <p class="app-tagline">您的个人AI总机</p>
      </div>
      
      <div class="loading-container">
        <n-spin :show="loading" description="正在初始化...">
          <div class="loading-steps">
            <div 
              v-for="(step, index) in loadingSteps" 
              :key="index"
              class="loading-step"
              :class="{ 'completed': step.completed, 'current': step.current }"
            >
              <n-icon v-if="step.completed" :size="16">
                <CheckmarkCircleOutline />
              </n-icon>
              <n-icon v-else-if="step.current" :size="16">
                <SyncOutline />
              </n-icon>
              <n-icon v-else :size="16">
                <EllipseOutline />
              </n-icon>
              <span>{{ step.text }}</span>
            </div>
          </div>
        </n-spin>
      </div>
      
      <div class="version-info">
        <p>版本 {{ version }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { NSpin, NIcon } from 'naive-ui';
import { 
  CheckmarkCircleOutline, 
  SyncOutline, 
  EllipseOutline 
} from '@vicons/ionicons5';

interface LoadingStep {
  text: string;
  completed: boolean;
  current: boolean;
}

const props = defineProps<{
  visible: boolean;
  version?: string;
}>();

const emit = defineEmits<{
  (e: 'complete'): void;
}>();

const loading = ref(true);
const loadingSteps = ref<LoadingStep[]>([
  { text: '初始化应用', completed: false, current: true },
  { text: '加载配置', completed: false, current: false },
  { text: '准备就绪', completed: false, current: false },
]);

onMounted(async () => {
  // 模拟加载过程
  await simulateLoading();
});

async function simulateLoading() {
  // 步骤1：初始化应用
  await delay(500);
  loadingSteps.value[0].completed = true;
  loadingSteps.value[0].current = false;
  loadingSteps.value[1].current = true;
  
  // 步骤2：加载配置
  await delay(500);
  loadingSteps.value[1].completed = true;
  loadingSteps.value[1].current = false;
  loadingSteps.value[2].current = true;
  
  // 步骤3：准备就绪
  await delay(300);
  loadingSteps.value[2].completed = true;
  loadingSteps.value[2].current = false;
  
  loading.value = false;
  
  // 延迟后关闭启动画面
  await delay(500);
  emit('complete');
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}
</script>

<style scoped>
.splash-screen {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 9999;
}

.splash-content {
  text-align: center;
  color: white;
}

.logo-container {
  margin-bottom: 40px;
}

.logo {
  width: 100px;
  height: 100px;
  margin-bottom: 20px;
}

.app-name {
  font-size: 48px;
  font-weight: bold;
  margin: 0 0 10px 0;
}

.app-tagline {
  font-size: 18px;
  opacity: 0.9;
  margin: 0;
}

.loading-container {
  margin-bottom: 40px;
}

.loading-steps {
  display: flex;
  flex-direction: column;
  gap: 12px;
  text-align: left;
  max-width: 300px;
  margin: 0 auto;
}

.loading-step {
  display: flex;
  align-items: center;
  gap: 10px;
  opacity: 0.6;
  transition: opacity 0.3s;
}

.loading-step.completed {
  opacity: 1;
}

.loading-step.current {
  opacity: 1;
}

.version-info {
  opacity: 0.7;
  font-size: 14px;
}
</style>
```

2. 在App.vue中集成启动画面
```vue
<!-- src/App.vue -->
<script setup lang="ts">
import { ref } from 'vue';
import SplashScreen from './components/SplashScreen.vue';

const showSplash = ref(true);
const appVersion = '1.0.0'; // 从package.json读取

function onSplashComplete() {
  showSplash.value = false;
}
</script>

<template>
  <SplashScreen 
    :visible="showSplash" 
    :version="appVersion"
    @complete="onSplashComplete"
  />
  
  <NConfigProvider>
    <!-- 其他内容 -->
  </NConfigProvider>
</template>
```

**验收标准**：
- [ ] 启动画面正常显示
- [ ] 加载步骤动画流畅
- [ ] 版本信息正确显示
- [ ] 自动关闭功能正常

---

### 任务1.2：创建首次引导页组件

**文件路径**：`src/components/OnboardingWizard.vue`

**任务描述**：
分步引导用户完成初始配置

**实现步骤**：

1. 创建引导页组件
```vue
<!-- src/components/OnboardingWizard.vue -->
<template>
  <n-modal 
    v-model:show="showModal" 
    :closable="false"
    :mask-closable="false"
    style="width: 700px"
  >
    <n-card title="欢迎使用 Silk">
      <div class="wizard-container">
        <!-- 步骤指示器 -->
        <n-steps :current="currentStep" :status="stepStatus">
          <n-step title="欢迎" />
          <n-step title="检测AI应用" />
          <n-step title="配置服务" />
          <n-step title="完成" />
        </n-steps>
        
        <!-- 步骤内容 -->
        <div class="step-content">
          <!-- 步骤1: 欢迎 -->
          <div v-if="currentStep === 0" class="step-welcome">
            <h2>什么是 Silk？</h2>
            <p>Silk 是您的个人AI总机，让您在一个地方访问所有AI服务。</p>
            <div class="features">
              <div class="feature">
                <n-icon :size="24"><FlashOutline /></n-icon>
                <span>一键访问多个AI服务</span>
              </div>
              <div class="feature">
                <n-icon :size="24"><ShieldCheckmarkOutline /></n-icon>
                <span>安全的本地代理</span>
              </div>
              <div class="feature">
                <n-icon :size="24"><SpeedometerOutline /></n-icon>
                <span>智能路由与负载均衡</span>
              </div>
            </div>
          </div>
          
          <!-- 步骤2: 检测AI应用 -->
          <div v-if="currentStep === 1" class="step-detect">
            <h2>检测您的AI应用</h2>
            <p>我们正在检测您电脑上已安装的AI应用...</p>
            
            <n-spin v-if="detecting" size="large" />
            
            <div v-else class="detected-apps">
              <n-list bordered>
                <n-list-item v-for="app in detectedApps" :key="app.name">
                  <template #prefix>
                    <n-icon :size="20" :color="app.color">
                      <component :is="app.icon" />
                    </n-icon>
                  </template>
                  <n-thing 
                    :title="app.name" 
                    :description="app.description"
                  />
                  <template #suffix>
                    <n-tag :type="app.installed ? 'success' : 'default'" size="small">
                      {{ app.installed ? '已安装' : '未安装' }}
                    </n-tag>
                  </template>
                </n-list-item>
              </n-list>
            </div>
          </div>
          
          <!-- 步骤3: 配置服务 -->
          <div v-if="currentStep === 2" class="step-config">
            <h2>配置AI服务</h2>
            <p>选择您要使用的AI服务：</p>
            
            <n-checkbox-group v-model:value="selectedServices">
              <n-space vertical>
                <n-checkbox 
                  v-for="service in availableServices" 
                  :key="service.id"
                  :value="service.id"
                >
                  <div class="service-option">
                    <n-icon :size="20" :color="service.color">
                      <component :is="service.icon" />
                    </n-icon>
                    <div>
                      <strong>{{ service.name }}</strong>
                      <p>{{ service.description }}</p>
                    </div>
                  </div>
                </n-checkbox>
              </n-space>
            </n-checkbox-group>
            
            <div class="api-key-input" v-if="selectedServices.length > 0">
              <h3>API密钥配置</h3>
              <p>请输入您选择的AI服务的API密钥：</p>
              
              <div v-for="serviceId in selectedServices" :key="serviceId" class="key-input">
                <n-input 
                  v-model:value="apiKeys[serviceId]"
                  :placeholder="`请输入 ${getServiceName(serviceId)} 的API密钥`"
                  type="password"
                  show-password-on="click"
                />
              </div>
            </div>
          </div>
          
          <!-- 步骤4: 完成 -->
          <div v-if="currentStep === 3" class="step-complete">
            <n-result status="success" title="配置完成！">
              <template #footer>
                <p>您现在可以开始使用 Silk 了。</p>
                <p>点击下方按钮进入主界面。</p>
              </template>
            </n-result>
          </div>
        </div>
        
        <!-- 操作按钮 -->
        <div class="wizard-actions">
          <n-button v-if="currentStep > 0" @click="prevStep">
            上一步
          </n-button>
          <n-button 
            type="primary" 
            @click="nextStep"
            :loading="saving"
          >
            {{ currentStep === 3 ? '开始使用' : '下一步' }}
          </n-button>
        </div>
      </div>
    </n-card>
  </n-modal>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { 
  NModal, NCard, NSteps, NStep, NButton, NIcon, 
  NList, NListItem, NThing, NTag, NCheckboxGroup, 
  NCheckbox, NSpace, NInput, NResult, NSpin 
} from 'naive-ui';
import { 
  FlashOutline, 
  ShieldCheckmarkOutline, 
  SpeedometerOutline,
  LogoOpenai,
  LogoGoogle,
  ChatboxOutline
} from '@vicons/ionicons5';
import { invoke } from '@tauri-apps/api/core';

interface DetectedApp {
  name: string;
  description: string;
  icon: any;
  color: string;
  installed: boolean;
}

interface AvailableService {
  id: string;
  name: string;
  description: string;
  icon: any;
  color: string;
}

const showModal = ref(true);
const currentStep = ref(0);
const stepStatus = ref<'process' | 'finish' | 'error'>('process');
const detecting = ref(false);
const saving = ref(false);

const detectedApps = ref<DetectedApp[]>([]);
const selectedServices = ref<string[]>([]);
const apiKeys = ref<Record<string, string>>({});

const availableServices: AvailableService[] = [
  {
    id: 'openai',
    name: 'OpenAI (ChatGPT)',
    description: '访问GPT-4、GPT-3.5等模型',
    icon: LogoOpenai,
    color: '#10a37f',
  },
  {
    id: 'claude',
    name: 'Claude',
    description: '访问Claude 3 Opus、Sonnet等模型',
    icon: ChatboxOutline,
    color: '#d97706',
  },
  {
    id: 'gemini',
    name: 'Google Gemini',
    description: '访问Gemini Pro等模型',
    icon: LogoGoogle,
    color: '#4285f4',
  },
];

onMounted(async () => {
  await detectInstalledApps();
});

async function detectInstalledApps() {
  detecting.value = true;
  
  try {
    // 调用后端检测已安装的AI应用
    const result = await invoke<DetectedApp[]>('detect_installed_ai_apps');
    detectedApps.value = result;
  } catch (error) {
    console.error('检测失败:', error);
    // 使用默认列表
    detectedApps.value = availableServices.map(service => ({
      name: service.name,
      description: service.description,
      icon: service.icon,
      color: service.color,
      installed: false,
    }));
  } finally {
    detecting.value = false;
  }
}

function getServiceName(serviceId: string): string {
  const service = availableServices.find(s => s.id === serviceId);
  return service?.name || serviceId;
}

function prevStep() {
  if (currentStep.value > 0) {
    currentStep.value--;
  }
}

async function nextStep() {
  if (currentStep.value < 3) {
    currentStep.value++;
    
    if (currentStep.value === 3) {
      await saveConfiguration();
    }
  } else {
    // 完成引导
    showModal.value = false;
    localStorage.setItem('onboarding_completed', 'true');
  }
}

async function saveConfiguration() {
  saving.value = true;
  
  try {
    // 保存配置到后端
    await invoke('save_onboarding_config', {
      services: selectedServices.value,
      apiKeys: apiKeys.value,
    });
  } catch (error) {
    console.error('保存配置失败:', error);
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.wizard-container {
  padding: 20px 0;
}

.step-content {
  margin: 30px 0;
  min-height: 300px;
}

.step-welcome h2,
.step-detect h2,
.step-config h2 {
  margin-bottom: 20px;
}

.features {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-top: 20px;
}

.feature {
  display: flex;
  align-items: center;
  gap: 12px;
}

.detected-apps {
  margin-top: 20px;
}

.service-option {
  display: flex;
  align-items: center;
  gap: 12px;
}

.service-option p {
  margin: 4px 0 0 0;
  font-size: 12px;
  opacity: 0.7;
}

.api-key-input {
  margin-top: 30px;
  padding-top: 20px;
  border-top: 1px solid #eee;
}

.key-input {
  margin-top: 12px;
}

.wizard-actions {
  display: flex;
  justify-content: space-between;
  margin-top: 30px;
  padding-top: 20px;
  border-top: 1px solid #eee;
}
</style>
```

2. 在App.vue中集成引导页
```vue
<!-- src/App.vue -->
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import SplashScreen from './components/SplashScreen.vue';
import OnboardingWizard from './components/OnboardingWizard.vue';

const showSplash = ref(true);
const showOnboarding = ref(false);
const appVersion = '1.0.0';

onMounted(() => {
  // 检查是否已完成引导
  const onboardingCompleted = localStorage.getItem('onboarding_completed');
  if (!onboardingCompleted) {
    showOnboarding.value = true;
  }
});

function onSplashComplete() {
  showSplash.value = false;
}
</script>

<template>
  <SplashScreen 
    :visible="showSplash" 
    :version="appVersion"
    @complete="onSplashComplete"
  />
  
  <OnboardingWizard v-if="showOnboarding" />
  
  <NConfigProvider>
    <!-- 其他内容 -->
  </NConfigProvider>
</template>
```

**验收标准**：
- [ ] 引导页正常显示
- [ ] 步骤切换流畅
- [ ] 配置保存功能正常
- [ ] 引导完成后不再显示

---

### 任务1.3：实现自动检测已安装AI应用

**文件路径**：`src-tauri/src/application/auto_detect.rs`

**任务描述**：
检测本地已安装的AI客户端

**实现步骤**：

1. 创建检测服务
```rust
// src-tauri/src/application/auto_detect.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 已安装的AI应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAiApp {
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub config_path: Option<PathBuf>,
    pub api_key_path: Option<PathBuf>,
}

/// AI应用检测服务
pub struct AiAppDetector;

impl AiAppDetector {
    /// 检测所有支持的AI应用
    pub fn detect_all() -> Vec<InstalledAiApp> {
        vec![
            Self::detect_openai(),
            Self::detect_claude(),
            Self::detect_gemini(),
        ]
    }
    
    /// 检测OpenAI应用
    fn detect_openai() -> InstalledAiApp {
        let config_path = dirs::config_dir()
            .map(|p| p.join("OpenAI").join("config.json"));
        
        InstalledAiApp {
            name: "OpenAI (ChatGPT)".to_string(),
            description: "访问GPT-4、GPT-3.5等模型".to_string(),
            installed: config_path.as_ref().map_or(false, |p| p.exists()),
            config_path,
            api_key_path: None,
        }
    }
    
    /// 检测Claude应用
    fn detect_claude() -> InstalledAiApp {
        let config_path = dirs::config_dir()
            .map(|p| p.join("Claude").join("config.json"));
        
        InstalledAiApp {
            name: "Claude".to_string(),
            description: "访问Claude 3 Opus、Sonnet等模型".to_string(),
            installed: config_path.as_ref().map_or(false, |p| p.exists()),
            config_path,
            api_key_path: None,
        }
    }
    
    /// 检测Gemini应用
    fn detect_gemini() -> InstalledAiApp {
        let config_path = dirs::config_dir()
            .map(|p| p.join("Google").join("Gemini").join("config.json"));
        
        InstalledAiApp {
            name: "Google Gemini".to_string(),
            description: "访问Gemini Pro等模型".to_string(),
            installed: config_path.as_ref().map_or(false, |p| p.exists()),
            config_path,
            api_key_path: None,
        }
    }
}
```

2. 创建Tauri命令
```rust
// src-tauri/src/commands/auto_detect.rs

use tauri::command;
use crate::application::auto_detect::{AiAppDetector, InstalledAiApp};

#[command]
pub async fn detect_installed_ai_apps() -> Result<Vec<InstalledAiApp>, String> {
    Ok(AiAppDetector::detect_all())
}
```

3. 注册命令
```rust
// src-tauri/src/lib.rs

// 在invoke_handler中添加
.invoke_handler(tauri::generate_handler![
    // ... 其他命令
    commands::auto_detect::detect_installed_ai_apps,
])
```

**验收标准**：
- [ ] 检测功能正常工作
- [ ] 返回结果格式正确
- [ ] 跨平台兼容性良好

---

### 任务1.4：实现一键配置功能

**文件路径**：`src-tauri/src/application/quick_setup.rs`

**任务描述**：
根据检测结果自动生成配置

**实现步骤**：

1. 创建配置服务
```rust
// src-tauri/src/application/quick_setup.rs

use serde::{Deserialize, Serialize};
use crate::models::Provider;

/// 快速配置请求
#[derive(Debug, Deserialize)]
pub struct QuickSetupRequest {
    pub services: Vec<String>,
    pub api_keys: std::collections::HashMap<String, String>,
}

/// 快速配置响应
#[derive(Debug, Serialize)]
pub struct QuickSetupResponse {
    pub success: bool,
    pub message: String,
    pub configured_services: Vec<String>,
}

/// 快速配置服务
pub struct QuickSetupService;

impl QuickSetupService {
    /// 执行快速配置
    pub async fn setup(request: QuickSetupRequest) -> Result<QuickSetupResponse, String> {
        let mut configured_services = Vec::new();
        
        for service_id in &request.services {
            if let Some(api_key) = request.api_keys.get(service_id) {
                let provider = Self::create_provider(service_id, api_key)?;
                
                // 保存到数据库
                let pool = crate::get_db_pool()
                    .ok_or("数据库未初始化")?;
                
                crate::persistence::ProviderRepo::create(pool, &provider)
                    .await
                    .map_err(|e| format!("保存配置失败: {e}"))?;
                
                configured_services.push(service_id.clone());
            }
        }
        
        Ok(QuickSetupResponse {
            success: true,
            message: format!("成功配置 {} 个服务", configured_services.len()),
            configured_services,
        })
    }
    
    /// 创建Provider配置
    fn create_provider(service_id: &str, api_key: &str) -> Result<Provider, String> {
        match service_id {
            "openai" => Ok(Provider {
                id: uuid::Uuid::new_v4().to_string(),
                name: "OpenAI".to_string(),
                protocols: r#"["openai_chat"]"#.to_string(),
                models: r#"["gpt-4", "gpt-3.5-turbo"]"#.to_string(),
                keys: serde_json::to_string(&[serde_json::json!({
                    "name": "主密钥",
                    "value": api_key,
                    "enabled": true,
                    "weight": 1
                })]).unwrap_or_default(),
                key_strategy: "round_robin".to_string(),
                api_base_url: "https://api.openai.com".to_string(),
                proxy_url: None,
                timeout_seconds: 30,
                max_retries: 3,
                status: "enabled".to_string(),
                health_status: None,
                last_health_check_at: None,
                metadata_json: None,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            }),
            "claude" => Ok(Provider {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Claude".to_string(),
                protocols: r#"["claude_messages"]"#.to_string(),
                models: r#"["claude-3-opus-20240229", "claude-3-sonnet-20240229"]"#.to_string(),
                keys: serde_json::to_string(&[serde_json::json!({
                    "name": "主密钥",
                    "value": api_key,
                    "enabled": true,
                    "weight": 1
                })]).unwrap_or_default(),
                key_strategy: "round_robin".to_string(),
                api_base_url: "https://api.anthropic.com".to_string(),
                proxy_url: None,
                timeout_seconds: 30,
                max_retries: 3,
                status: "enabled".to_string(),
                health_status: None,
                last_health_check_at: None,
                metadata_json: None,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            }),
            "gemini" => Ok(Provider {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Google Gemini".to_string(),
                protocols: r#"["openai_chat"]"#.to_string(),
                models: r#"["gemini-pro"]"#.to_string(),
                keys: serde_json::to_string(&[serde_json::json!({
                    "name": "主密钥",
                    "value": api_key,
                    "enabled": true,
                    "weight": 1
                })]).unwrap_or_default(),
                key_strategy: "round_robin".to_string(),
                api_base_url: "https://generativelanguage.googleapis.com".to_string(),
                proxy_url: None,
                timeout_seconds: 30,
                max_retries: 3,
                status: "enabled".to_string(),
                health_status: None,
                last_health_check_at: None,
                metadata_json: None,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            }),
            _ => Err(format!("不支持的服务: {service_id}")),
        }
    }
}
```

2. 创建Tauri命令
```rust
// src-tauri/src/commands/quick_setup.rs

use tauri::command;
use crate::application::quick_setup::{QuickSetupRequest, QuickSetupResponse, QuickSetupService};

#[command]
pub async fn save_onboarding_config(
    services: Vec<String>,
    api_keys: std::collections::HashMap<String, String>,
) -> Result<QuickSetupResponse, String> {
    let request = QuickSetupRequest {
        services,
        api_keys,
    };
    
    QuickSetupService::setup(request).await
}
```

3. 注册命令
```rust
// src-tauri/src/lib.rs

// 在invoke_handler中添加
.invoke_handler(tauri::generate_handler![
    // ... 其他命令
    commands::auto_detect::detect_installed_ai_apps,
    commands::quick_setup::save_onboarding_config,
])
```

**验收标准**：
- [ ] 配置保存功能正常
- [ ] 支持所有AI服务
- [ ] 错误处理完善

---

## 📦 批次交付物

1. `src/components/SplashScreen.vue` - 启动画面组件
2. `src/components/OnboardingWizard.vue` - 引导页组件
3. `src-tauri/src/application/auto_detect.rs` - 自动检测服务
4. `src-tauri/src/application/quick_setup.rs` - 一键配置服务
5. 更新的`src-tauri/src/lib.rs` - 注册新命令

---

## ✅ 批次验收标准

- [ ] 启动画面正常显示
- [ ] 引导页功能完整
- [ ] 自动检测功能正常
- [ ] 一键配置功能正常
- [ ] 所有单元测试通过
- [ ] 代码通过cargo clippy检查

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务1.1 | 1天 | 第1天 | 第1天 |
| 任务1.2 | 1.5天 | 第2天 | 第3天上午 |
| 任务1.3 | 1天 | 第3天下午 | 第4天 |
| 任务1.4 | 1天 | 第5天 | 第5天 |
