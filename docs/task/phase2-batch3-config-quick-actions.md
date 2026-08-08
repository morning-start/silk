# Phase 2 - 批次3：配置优化与快捷操作

## 📋 批次概述

**批次目标**：创建预置AI服务配置库、配置导入导出、快捷操作面板

**预计工期**：1周（5个工作日）

**依赖关系**：Phase 1 完成

---

## 🎯 批次目标

1. 创建预置AI服务配置库
2. 实现配置导入导出功能
3. 创建快捷操作面板组件
4. 实现智能默认配置

---

## 📝 任务清单

### 任务3.1：创建预置AI服务配置库

**文件路径**：`src-tauri/src/data/preset_providers.json`

**任务描述**：
预置常用AI服务配置（OpenAI、Claude、文心一言等）

**实现步骤**：

1. 创建预置配置文件
```json
{
  "providers": [
    {
      "id": "openai",
      "name": "OpenAI",
      "description": "访问GPT-4、GPT-3.5等模型",
      "protocols": ["openai_chat"],
      "models": [
        {
          "id": "gpt-4",
          "name": "GPT-4",
          "description": "最强大的模型，适合复杂任务"
        },
        {
          "id": "gpt-4-turbo",
          "name": "GPT-4 Turbo",
          "description": "GPT-4的快速版本，性价比高"
        },
        {
          "id": "gpt-3.5-turbo",
          "name": "GPT-3.5 Turbo",
          "description": "快速且经济实惠，适合日常任务"
        }
      ],
      "api_base_url": "https://api.openai.com",
      "api_key_url": "https://platform.openai.com/api-keys",
      "api_key_placeholder": "sk-...",
      "color": "#10a37f",
      "icon": "logo-openai"
    },
    {
      "id": "claude",
      "name": "Claude",
      "description": "访问Claude 3 Opus、Sonnet等模型",
      "protocols": ["claude_messages"],
      "models": [
        {
          "id": "claude-3-opus-20240229",
          "name": "Claude 3 Opus",
          "description": "最强大的模型，适合复杂推理"
        },
        {
          "id": "claude-3-sonnet-20240229",
          "name": "Claude 3 Sonnet",
          "description": "平衡性能和速度"
        },
        {
          "id": "claude-3-haiku-20240307",
          "name": "Claude 3 Haiku",
          "description": "最快速的模型，适合简单任务"
        }
      ],
      "api_base_url": "https://api.anthropic.com",
      "api_key_url": "https://console.anthropic.com/api-keys",
      "api_key_placeholder": "sk-ant-...",
      "color": "#d97706",
      "icon": "chatbox-outline"
    },
    {
      "id": "gemini",
      "name": "Google Gemini",
      "description": "访问Gemini Pro等模型",
      "protocols": ["openai_chat"],
      "models": [
        {
          "id": "gemini-pro",
          "name": "Gemini Pro",
          "description": "Google的先进模型"
        }
      ],
      "api_base_url": "https://generativelanguage.googleapis.com",
      "api_key_url": "https://makersuite.google.com/app/apikey",
      "api_key_placeholder": "AIza...",
      "color": "#4285f4",
      "icon": "logo-google"
    },
    {
      "id": "wenxin",
      "name": "文心一言",
      "description": "百度的AI大模型",
      "protocols": ["openai_chat"],
      "models": [
        {
          "id": "ernie-bot-4",
          "name": "ERNIE Bot 4.0",
          "description": "最强大的版本"
        },
        {
          "id": "ernie-bot",
          "name": "ERNIE Bot",
          "description": "标准版本"
        }
      ],
      "api_base_url": "https://aip.baidubce.com/rpc/2.0/ai_custom/v1/wenxinworkshop",
      "api_key_url": "https://console.bce.baidu.com/qianfan/ais/console/applicationConsole/application",
      "api_key_placeholder": "请输入API Key",
      "color": "#2932e1",
      "icon": "chatbox-outline"
    },
    {
      "id": "tongyi",
      "name": "通义千问",
      "description": "阿里巴巴的AI大模型",
      "protocols": ["openai_chat"],
      "models": [
        {
          "id": "qwen-max",
          "name": "通义千问 Max",
          "description": "最强大的版本"
        },
        {
          "id": "qwen-turbo",
          "name": "通义千问 Turbo",
          "description": "快速版本"
        }
      ],
      "api_base_url": "https://dashscope.aliyuncs.com/api/v1",
      "api_key_url": "https://dashscope.console.aliyun.com/apiKey",
      "api_key_placeholder": "sk-...",
      "color": "#ff6a00",
      "icon": "chatbox-outline"
    },
    {
      "id": "deepseek",
      "name": "DeepSeek",
      "description": "DeepSeek AI大模型",
      "protocols": ["openai_chat"],
      "models": [
        {
          "id": "deepseek-chat",
          "name": "DeepSeek Chat",
          "description": "对话模型"
        },
        {
          "id": "deepseek-coder",
          "name": "DeepSeek Coder",
          "description": "代码模型"
        }
      ],
      "api_base_url": "https://api.deepseek.com",
      "api_key_url": "https://platform.deepseek.com/api_keys",
      "api_key_placeholder": "sk-...",
      "color": "#0066ff",
      "icon": "chatbox-outline"
    }
  ]
}
```

2. 创建预置配置加载服务
```rust
// src-tauri/src/application/preset_providers.rs

use serde::{Deserialize, Serialize};

/// 预置AI服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetProvider {
    pub id: String,
    pub name: String,
    pub description: String,
    pub protocols: Vec<String>,
    pub models: Vec<PresetModel>,
    pub api_base_url: String,
    pub api_key_url: String,
    pub api_key_placeholder: String,
    pub color: String,
    pub icon: String,
}

/// 预置模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetModel {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// 预置配置服务
pub struct PresetProviderService;

impl PresetProviderService {
    /// 获取所有预置配置
    pub fn get_all() -> Vec<PresetProvider> {
        let data = include_str!("../data/preset_providers.json");
        serde_json::from_str(data).unwrap_or_default()
    }
    
    /// 根据ID获取预置配置
    pub fn get_by_id(id: &str) -> Option<PresetProvider> {
        Self::get_all().into_iter().find(|p| p.id == id)
    }
    
    /// 获取所有预置配置的ID列表
    pub fn get_all_ids() -> Vec<String> {
        Self::get_all().iter().map(|p| p.id.clone()).collect()
    }
}
```

3. 创建Tauri命令
```rust
// src-tauri/src/commands/preset_providers.rs

use tauri::command;
use crate::application::preset_providers::{PresetProvider, PresetProviderService};

#[command]
pub async fn get_preset_providers() -> Result<Vec<PresetProvider>, String> {
    Ok(PresetProviderService::get_all())
}

#[command]
pub async fn get_preset_provider_by_id(id: String) -> Result<Option<PresetProvider>, String> {
    Ok(PresetProviderService::get_by_id(&id))
}
```

**验收标准**：
- [ ] 预置配置文件完整
- [ ] 配置加载服务正常
- [ ] Tauri命令功能正常
- [ ] 所有AI服务配置正确

---

### 任务3.2：实现配置导入导出功能

**文件路径**：`src-tauri/src/application/config_transfer.rs`

**任务描述**：
支持配置文件导入导出

**实现步骤**：

1. 增强配置导入导出服务
```rust
// src-tauri/src/application/config_transfer.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 配置导出格式
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedConfig {
    pub version: String,
    pub exported_at: String,
    pub providers: Vec<ExportedProvider>,
    pub settings: ExportedSettings,
    pub model_mappings: Vec<ExportedModelMapping>,
}

/// 导出的Provider配置
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedProvider {
    pub name: String,
    pub protocols: Vec<String>,
    pub models: Vec<String>,
    pub api_base_url: String,
    pub key_strategy: String,
    pub timeout_seconds: u32,
    pub max_retries: u32,
}

/// 导出的设置
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedSettings {
    pub bind_host: String,
    pub bind_port: u16,
    pub allow_remote: bool,
    pub auto_start_gateway: bool,
    pub launch_at_startup: bool,
}

/// 导出的模型映射
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedModelMapping {
    pub name: String,
    pub target_model: String,
    pub provider_id: String,
}

/// 配置导入导出服务
pub struct ConfigTransferService;

impl ConfigTransferService {
    /// 导出配置
    pub async fn export_config() -> Result<ExportedConfig, String> {
        let pool = crate::get_db_pool()
            .ok_or("数据库未初始化")?;
        
        // 获取所有Provider
        let providers = crate::persistence::ProviderRepo::list_all(pool)
            .await
            .map_err(|e| format!("获取Provider失败: {e}"))?;
        
        // 获取设置
        let settings_path = crate::get_settings_path()
            .ok_or("设置路径未初始化")?;
        let settings = crate::persistence::GatewaySettingsRepo::load_effective(settings_path);
        
        // 获取模型映射
        let model_mappings = crate::persistence::ModelMappingRepo::list_all(pool)
            .await
            .map_err(|e| format!("获取模型映射失败: {e}"))?;
        
        Ok(ExportedConfig {
            version: "1.0.0".to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            providers: providers.iter().map(|p| ExportedProvider {
                name: p.name.clone(),
                protocols: serde_json::from_str(&p.protocols).unwrap_or_default(),
                models: serde_json::from_str(&p.models).unwrap_or_default(),
                api_base_url: p.api_base_url.clone(),
                key_strategy: p.key_strategy.clone(),
                timeout_seconds: p.timeout_seconds as u32,
                max_retries: p.max_retries as u32,
            }).collect(),
            settings: ExportedSettings {
                bind_host: settings.bind_host.clone(),
                bind_port: settings.bind_port as u16,
                allow_remote: settings.allow_remote,
                auto_start_gateway: settings.auto_start_gateway,
                launch_at_startup: settings.launch_at_startup,
            },
            model_mappings: model_mappings.iter().map(|m| ExportedModelMapping {
                name: m.model_name.clone(),
                target_model: m.model_name.clone(),
                provider_id: m.provider_id.clone().unwrap_or_default(),
            }).collect(),
        })
    }
    
    /// 导入配置
    pub async fn import_config(config: ExportedConfig) -> Result<(), String> {
        let pool = crate::get_db_pool()
            .ok_or("数据库未初始化")?;
        
        // 导入Provider
        for provider in config.providers {
            let new_provider = crate::models::Provider {
                id: uuid::Uuid::new_v4().to_string(),
                name: provider.name,
                protocols: serde_json::to_string(&provider.protocols).unwrap_or_default(),
                models: serde_json::to_string(&provider.models).unwrap_or_default(),
                keys: "[]".to_string(),
                key_strategy: provider.key_strategy,
                api_base_url: provider.api_base_url,
                proxy_url: None,
                timeout_seconds: provider.timeout_seconds as i64,
                max_retries: provider.max_retries as i64,
                status: "enabled".to_string(),
                health_status: None,
                last_health_check_at: None,
                metadata_json: None,
                created_at: chrono::Utc::now().naive_utc(),
                updated_at: chrono::Utc::now().naive_utc(),
            };
            
            crate::persistence::ProviderRepo::create(pool, &new_provider)
                .await
                .map_err(|e| format!("导入Provider失败: {e}"))?;
        }
        
        // 导入设置
        let settings_path = crate::get_settings_path()
            .ok_or("设置路径未初始化")?;
        let update = crate::models::UpdateGatewaySettings {
            bind_host: Some(config.settings.bind_host),
            bind_port: Some(config.settings.bind_port as i64),
            allow_remote: Some(config.settings.allow_remote),
            auto_start_gateway: Some(config.settings.auto_start_gateway),
            launch_at_startup: Some(config.settings.launch_at_startup),
            ..Default::default()
        };
        crate::persistence::GatewaySettingsRepo::update(settings_path, &update)
            .map_err(|e| format!("导入设置失败: {e}"))?;
        
        Ok(())
    }
}
```

2. 创建Tauri命令
```rust
// src-tauri/src/commands/config_transfer.rs

use tauri::command;
use crate::application::config_transfer::{ConfigTransferService, ExportedConfig};

#[command]
pub async fn export_app_config() -> Result<ExportedConfig, String> {
    ConfigTransferService::export_config().await
}

#[command]
pub async fn import_app_config(config: ExportedConfig) -> Result<(), String> {
    ConfigTransferService::import_config(config).await
}
```

**验收标准**：
- [ ] 配置导出功能正常
- [ ] 配置导入功能正常
- [ ] 导入导出格式兼容
- [ ] 错误处理完善

---

### 任务3.3：创建快捷操作面板组件

**文件路径**：`src/components/QuickActions.vue`

**任务描述**：
常用AI服务快捷入口，一键切换

**实现步骤**：

1. 创建快捷操作面板
```vue
<!-- src/components/QuickActions.vue -->
<template>
  <div class="quick-actions">
    <n-card title="快捷操作">
      <template #header-extra>
        <n-button text @click="showSettings = true">
          <template #icon>
            <n-icon><SettingsOutline /></n-icon>
          </template>
        </n-button>
      </template>
      
      <!-- 快捷服务列表 -->
      <div class="service-grid">
        <div 
          v-for="service in quickServices" 
          :key="service.id"
          class="service-card"
          :class="{ 'active': service.active }"
          @click="toggleService(service)"
        >
          <div class="service-icon" :style="{ backgroundColor: service.color }">
            <n-icon :size="24" color="white">
              <component :is="service.icon" />
            </n-icon>
          </div>
          <div class="service-info">
            <div class="service-name">{{ service.name }}</div>
            <div class="service-status">
              <n-tag :type="service.active ? 'success' : 'default'" size="small">
                {{ service.active ? '已启用' : '未启用' }}
              </n-tag>
            </div>
          </div>
        </div>
      </div>
      
      <!-- 快捷操作按钮 -->
      <div class="action-buttons">
        <n-button type="primary" @click="startAllServices">
          <template #icon>
            <n-icon><PlayOutline /></n-icon>
          </template>
          启动所有服务
        </n-button>
        <n-button @click="stopAllServices">
          <template #icon>
            <n-icon><StopOutline /></n-icon>
          </template>
          停止所有服务
        </n-button>
        <n-button @click="testAllConnections">
          <template #icon>
            <n-icon><WifiOutline /></n-icon>
          </template>
          测试连接
        </n-button>
      </div>
    </n-card>
    
    <!-- 设置对话框 -->
    <n-modal v-model:show="showSettings" style="width: 600px">
      <n-card title="快捷操作设置">
        <n-form :model="settings" label-placement="left" label-width="100">
          <n-form-item label="显示服务">
            <n-checkbox-group v-model:value="settings.visibleServices">
              <n-space>
                <n-checkbox 
                  v-for="service in allServices" 
                  :key="service.id"
                  :value="service.id"
                >
                  {{ service.name }}
                </n-checkbox>
              </n-space>
            </n-checkbox-group>
          </n-form-item>
          <n-form-item label="自动启动">
            <n-switch v-model:value="settings.autoStart" />
          </n-form-item>
          <n-form-item label="显示状态">
            <n-switch v-model:value="settings.showStatus" />
          </n-form-item>
        </n-form>
        
        <template #action>
          <n-space>
            <n-button @click="showSettings = false">取消</n-button>
            <n-button type="primary" @click="saveSettings">保存</n-button>
          </n-space>
        </template>
      </n-card>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { 
  NCard, NButton, NIcon, NTag, NModal, NForm, NFormItem, 
  NCheckboxGroup, NCheckbox, NSwitch, NSpace, useMessage 
} from 'naive-ui';
import { 
  SettingsOutline, PlayOutline, StopOutline, WifiOutline,
  LogoOpenai, LogoGoogle, ChatboxOutline 
} from '@vicons/ionicons5';
import { invoke } from '@tauri-apps/api/core';

interface QuickService {
  id: string;
  name: string;
  icon: any;
  color: string;
  active: boolean;
  enabled: boolean;
}

const message = useMessage();

const showSettings = ref(false);
const settings = ref({
  visibleServices: ['openai', 'claude', 'gemini'],
  autoStart: false,
  showStatus: true,
});

const allServices: QuickService[] = [
  { id: 'openai', name: 'OpenAI', icon: LogoOpenai, color: '#10a37f', active: false, enabled: true },
  { id: 'claude', name: 'Claude', icon: ChatboxOutline, color: '#d97706', active: false, enabled: true },
  { id: 'gemini', name: 'Gemini', icon: LogoGoogle, color: '#4285f4', active: false, enabled: true },
  { id: 'wenxin', name: '文心一言', icon: ChatboxOutline, color: '#2932e1', active: false, enabled: true },
  { id: 'tongyi', name: '通义千问', icon: ChatboxOutline, color: '#ff6a00', active: false, enabled: true },
  { id: 'deepseek', name: 'DeepSeek', icon: ChatboxOutline, color: '#0066ff', active: false, enabled: true },
];

const quickServices = computed(() => {
  return allServices.filter(s => 
    settings.value.visibleServices.includes(s.id)
  );
});

onMounted(async () => {
  await loadServiceStatus();
  loadSettings();
});

async function loadServiceStatus() {
  try {
    const status = await invoke<Record<string, boolean>>('get_service_status');
    allServices.forEach(service => {
      service.active = status[service.id] || false;
    });
  } catch (error) {
    console.error('加载服务状态失败:', error);
  }
}

function loadSettings() {
  const saved = localStorage.getItem('quick_actions_settings');
  if (saved) {
    try {
      settings.value = JSON.parse(saved);
    } catch (e) {
      console.error('加载设置失败:', e);
    }
  }
}

function saveSettings() {
  localStorage.setItem('quick_actions_settings', JSON.stringify(settings.value));
  showSettings.value = false;
  message.success('设置已保存');
}

async function toggleService(service: QuickService) {
  try {
    if (service.active) {
      await invoke('stop_service', { serviceId: service.id });
      service.active = false;
      message.success(`已停止 ${service.name}`);
    } else {
      await invoke('start_service', { serviceId: service.id });
      service.active = true;
      message.success(`已启动 ${service.name}`);
    }
  } catch (error: any) {
    message.error(error.message || '操作失败');
  }
}

async function startAllServices() {
  try {
    await invoke('start_all_services');
    allServices.forEach(s => s.active = true);
    message.success('已启动所有服务');
  } catch (error: any) {
    message.error(error.message || '启动失败');
  }
}

async function stopAllServices() {
  try {
    await invoke('stop_all_services');
    allServices.forEach(s => s.active = false);
    message.success('已停止所有服务');
  } catch (error: any) {
    message.error(error.message || '停止失败');
  }
}

async function testAllConnections() {
  try {
    const results = await invoke<Record<string, boolean>>('test_all_connections');
    const successCount = Object.values(results).filter(Boolean).length;
    const totalCount = Object.keys(results).length;
    message.success(`连接测试完成: ${successCount}/${totalCount} 成功`);
  } catch (error: any) {
    message.error(error.message || '测试失败');
  }
}
</script>

<style scoped>
.quick-actions {
  height: 100%;
}

.service-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 16px;
  margin-bottom: 20px;
}

.service-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  border: 1px solid #eee;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.service-card:hover {
  border-color: #18a058;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.service-card.active {
  border-color: #18a058;
  background-color: #f0fff4;
}

.service-icon {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.service-info {
  flex: 1;
}

.service-name {
  font-weight: bold;
  margin-bottom: 4px;
}

.service-status {
  font-size: 12px;
}

.action-buttons {
  display: flex;
  gap: 12px;
}
</style>
```

**验收标准**：
- [ ] 快捷操作面板显示正常
- [ ] 服务切换功能正常
- [ ] 设置保存功能正常
- [ ] 批量操作功能正常

---

### 任务3.4：实现智能默认配置

**文件路径**：`src-tauri/src/application/smart_defaults.rs`

**任务描述**：
根据用户环境自动推荐配置

**实现步骤**：

1. 创建智能默认配置服务
```rust
// src-tauri/src/application/smart_defaults.rs

use serde::{Deserialize, Serialize};

/// 智能默认配置推荐
#[derive(Debug, Serialize, Deserialize)]
pub struct SmartDefaultRecommendation {
    pub provider_id: String,
    pub provider_name: String,
    pub reason: String,
    pub confidence: f32, // 0.0 - 1.0
    pub auto_configure: bool,
}

/// 智能默认配置服务
pub struct SmartDefaultsService;

impl SmartDefaultsService {
    /// 获取智能默认配置推荐
    pub fn get_recommendations() -> Vec<SmartDefaultRecommendation> {
        let mut recommendations = Vec::new();
        
        // 检测已安装的应用
        let installed_apps = crate::application::auto_detect::AiAppDetector::detect_all();
        
        for app in installed_apps {
            if app.installed {
                let (provider_id, provider_name, reason) = match app.name.as_str() {
                    "OpenAI (ChatGPT)" => (
                        "openai",
                        "OpenAI",
                        "检测到您已安装ChatGPT，推荐使用OpenAI服务"
                    ),
                    "Claude" => (
                        "claude",
                        "Claude",
                        "检测到您已安装Claude，推荐使用Claude服务"
                    ),
                    "Google Gemini" => (
                        "gemini",
                        "Google Gemini",
                        "检测到您已安装Gemini，推荐使用Google Gemini服务"
                    ),
                    _ => continue,
                };
                
                recommendations.push(SmartDefaultRecommendation {
                    provider_id: provider_id.to_string(),
                    provider_name: provider_name.to_string(),
                    reason: reason.to_string(),
                    confidence: 0.8,
                    auto_configure: true,
                });
            }
        }
        
        // 如果没有检测到已安装的应用，推荐OpenAI
        if recommendations.is_empty() {
            recommendations.push(SmartDefaultRecommendation {
                provider_id: "openai".to_string(),
                provider_name: "OpenAI".to_string(),
                reason: "OpenAI是最流行的AI服务，推荐作为默认选择".to_string(),
                confidence: 0.6,
                auto_configure: false,
            });
        }
        
        recommendations
    }
    
    /// 自动配置推荐的服务
    pub async fn auto_configure_recommended() -> Result<Vec<String>, String> {
        let recommendations = Self::get_recommendations();
        let mut configured = Vec::new();
        
        for rec in recommendations {
            if rec.auto_configure {
                // 这里可以添加自动配置逻辑
                // 例如：从已安装应用中读取API密钥
                configured.push(rec.provider_id);
            }
        }
        
        Ok(configured)
    }
}
```

2. 创建Tauri命令
```rust
// src-tauri/src/commands/smart_defaults.rs

use tauri::command;
use crate::application::smart_defaults::{SmartDefaultRecommendation, SmartDefaultsService};

#[command]
pub async fn get_smart_default_recommendations() -> Result<Vec<SmartDefaultRecommendation>, String> {
    Ok(SmartDefaultsService::get_recommendations())
}

#[command]
pub async fn auto_configure_recommended() -> Result<Vec<String>, String> {
    SmartDefaultsService::auto_configure_recommended().await
}
```

**验收标准**：
- [ ] 智能推荐功能正常
- [ ] 自动检测功能正常
- [ ] 推荐理由清晰
- [ ] 自动配置功能正常

---

## 📦 批次交付物

1. `src-tauri/src/data/preset_providers.json` - 预置配置文件
2. `src-tauri/src/application/preset_providers.rs` - 预置配置服务
3. `src-tauri/src/application/config_transfer.rs` - 配置导入导出服务
4. `src/components/QuickActions.vue` - 快捷操作面板
5. `src-tauri/src/application/smart_defaults.rs` - 智能默认配置服务

---

## ✅ 批次验收标准

- [ ] 预置配置功能完整
- [ ] 配置导入导出功能正常
- [ ] 快捷操作面板功能正常
- [ ] 智能默认配置功能正常
- [ ] 所有单元测试通过
- [ ] 代码通过cargo clippy检查

---

## 📅 时间安排

| 任务 | 预计时间 | 开始时间 | 结束时间 |
|------|----------|----------|----------|
| 任务3.1 | 1天 | 第1天 | 第1天 |
| 任务3.2 | 1天 | 第2天 | 第2天 |
| 任务3.3 | 1.5天 | 第3天 | 第4天上午 |
| 任务3.4 | 1天 | 第4天下午 | 第5天 |
