<template>
  <div class="help-overlay" v-if="show" @click.self="$emit('close')">
    <div class="help-modal">
      <div class="help-header">
        <h2>帮助中心</h2>
        <button class="btn-close" @click="$emit('close')">✕</button>
      </div>

      <div class="help-tabs">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="tab-btn"
          :class="{ 'active': activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          {{ tab.label }}
        </button>
      </div>

      <div class="help-content">
        <!-- 快速开始 -->
        <div v-if="activeTab === 'quickstart'" class="tab-content">
          <h3>快速开始</h3>
          <div class="steps">
            <div class="step">
              <div class="step-number">1</div>
              <div class="step-content">
                <h4>添加AI服务</h4>
                <p>点击左侧菜单的"AI服务"，然后点击"添加服务"按钮。选择您要使用的AI服务，输入API密钥。</p>
              </div>
            </div>
            <div class="step">
              <div class="step-number">2</div>
              <div class="step-content">
                <h4>启动网关</h4>
                <p>在仪表盘页面，点击"启动网关"按钮。网关启动后，您可以通过本地地址访问AI服务。</p>
              </div>
            </div>
            <div class="step">
              <div class="step-number">3</div>
              <div class="step-content">
                <h4>开始使用</h4>
                <p>在您的AI客户端中，将API地址设置为Silk的地址。例如：<code>http://127.0.0.1:1877/v1/chat/completions</code></p>
              </div>
            </div>
          </div>
        </div>

        <!-- 常见问题 -->
        <div v-if="activeTab === 'faq'" class="tab-content">
          <h3>常见问题</h3>
          <div class="faq-list">
            <div v-for="(faq, index) in faqs" :key="index" class="faq-item">
              <div class="faq-question" @click="faq.open = !faq.open">
                <span>{{ faq.question }}</span>
                <span class="faq-toggle">{{ faq.open ? '−' : '+' }}</span>
              </div>
              <div v-if="faq.open" class="faq-answer">
                <p v-html="faq.answer"></p>
              </div>
            </div>
          </div>
        </div>

        <!-- 使用教程 -->
        <div v-if="activeTab === 'tutorials'" class="tab-content">
          <h3>使用教程</h3>
          <div class="tutorial-list">
            <div class="tutorial-item" @click="openTutorial('basic')">
              <div class="tutorial-icon">📖</div>
              <div class="tutorial-info">
                <h4>基础配置教程</h4>
                <p>学习如何配置AI服务和网关</p>
              </div>
            </div>
            <div class="tutorial-item" @click="openTutorial('advanced')">
              <div class="tutorial-icon">🔧</div>
              <div class="tutorial-info">
                <h4>高级功能教程</h4>
                <p>学习使用模型映射、负载均衡等高级功能</p>
              </div>
            </div>
            <div class="tutorial-item" @click="openTutorial('troubleshooting')">
              <div class="tutorial-icon">🔍</div>
              <div class="tutorial-info">
                <h4>故障排除指南</h4>
                <p>解决常见问题的方法</p>
              </div>
            </div>
          </div>
        </div>

        <!-- 关于 -->
        <div v-if="activeTab === 'about'" class="tab-content">
          <div class="about-section">
            <div class="about-logo">
              <div class="logo-icon">
                <svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
                  <circle cx="50" cy="50" r="45" fill="#0891b2" />
                  <text x="50" y="65" font-family="Arial" font-size="40" font-weight="bold" fill="white" text-anchor="middle">S</text>
                </svg>
              </div>
              <h3>Silk</h3>
              <p class="version">版本 {{ version }}</p>
            </div>
            <div class="about-links">
              <p><strong>官方网站</strong>：<a href="https://silk.dev" target="_blank">silk.dev</a></p>
              <p><strong>GitHub</strong>：<a href="https://github.com/silk/silk" target="_blank">github.com/silk/silk</a></p>
              <p><strong>文档</strong>：<a href="https://docs.silk.dev" target="_blank">docs.silk.dev</a></p>
            </div>
            <p class="copyright">© 2024 Silk. All rights reserved.</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

defineProps<{
  show: boolean;
}>();

defineEmits<{
  (e: 'close'): void;
}>();

const version = '1.0.0';
const activeTab = ref('quickstart');

const tabs = [
  { id: 'quickstart', label: '快速开始' },
  { id: 'faq', label: '常见问题' },
  { id: 'tutorials', label: '使用教程' },
  { id: 'about', label: '关于' },
];

const faqs = ref([
  {
    question: '如何获取API密钥？',
    answer: '每个AI服务都有自己的API密钥获取方式：<br><strong>OpenAI</strong>：访问 <a href="https://platform.openai.com/api-keys" target="_blank">platform.openai.com</a><br><strong>Claude</strong>：访问 <a href="https://console.anthropic.com/api-keys" target="_blank">console.anthropic.com</a>',
    open: false,
  },
  {
    question: '网关启动失败怎么办？',
    answer: '网关启动失败可能有以下原因：<br>• <strong>端口被占用</strong>：尝试修改端口号，或关闭占用端口的程序<br>• <strong>权限不足</strong>：以管理员身份运行Silk<br>• <strong>防火墙阻止</strong>：检查防火墙设置，允许Silk访问网络',
    open: false,
  },
  {
    question: '如何查看请求日志？',
    answer: '点击左侧菜单的"请求日志"，可以查看所有AI服务的请求记录。您可以按时间、服务、状态等条件过滤日志。',
    open: false,
  },
  {
    question: '如何备份配置？',
    answer: '在设置页面的"数据管理"标签中，点击"导出配置"按钮。配置文件将保存为JSON格式，您可以随时导入恢复。',
    open: false,
  },
  {
    question: '支持哪些AI服务？',
    answer: 'Silk 目前支持以下AI服务：<br>• OpenAI (ChatGPT)<br>• Claude<br>• Google Gemini<br>• 文心一言<br>• 通义千问<br>• DeepSeek<br>• 以及其他兼容OpenAI API格式的服务',
    open: false,
  },
]);

function openTutorial(type: string) {
  // 打开教程页面
  window.open(`/tutorials/${type}`, '_blank');
}
</script>

<style scoped>
.help-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 10000;
  backdrop-filter: blur(4px);
}

.help-modal {
  background: white;
  border-radius: 16px;
  width: 600px;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.help-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px 24px;
  border-bottom: 1px solid #e2e8f0;
}

.help-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: #1e293b;
}

.btn-close {
  background: none;
  border: none;
  font-size: 20px;
  color: #94a3b8;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
}

.btn-close:hover {
  background: #f1f5f9;
  color: #475569;
}

.help-tabs {
  display: flex;
  border-bottom: 1px solid #e2e8f0;
  padding: 0 24px;
}

.tab-btn {
  padding: 12px 16px;
  background: none;
  border: none;
  font-size: 14px;
  color: #64748b;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
}

.tab-btn:hover {
  color: #1e293b;
}

.tab-btn.active {
  color: #0891b2;
  border-bottom-color: #0891b2;
}

.help-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.tab-content h3 {
  margin: 0 0 20px;
  font-size: 18px;
  font-weight: 600;
  color: #1e293b;
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.step {
  display: flex;
  gap: 16px;
}

.step-number {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: #0891b2;
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 600;
  flex-shrink: 0;
}

.step-content h4 {
  margin: 0 0 4px;
  font-size: 15px;
  font-weight: 600;
  color: #1e293b;
}

.step-content p {
  margin: 0;
  font-size: 14px;
  color: #475569;
  line-height: 1.6;
}

.step-content code {
  background: #f1f5f9;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 13px;
  color: #0891b2;
}

.faq-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.faq-item {
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  overflow: hidden;
}

.faq-question {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 16px;
  cursor: pointer;
  font-weight: 500;
  color: #1e293b;
  transition: background 0.2s;
}

.faq-question:hover {
  background: #f8fafc;
}

.faq-toggle {
  font-size: 18px;
  color: #94a3b8;
}

.faq-answer {
  padding: 0 16px 14px;
  font-size: 14px;
  color: #475569;
  line-height: 1.6;
}

.faq-answer p {
  margin: 0;
}

.tutorial-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.tutorial-item {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
}

.tutorial-item:hover {
  border-color: #0891b2;
  background: #f0fdfa;
}

.tutorial-icon {
  font-size: 24px;
}

.tutorial-info h4 {
  margin: 0 0 4px;
  font-size: 15px;
  font-weight: 600;
  color: #1e293b;
}

.tutorial-info p {
  margin: 0;
  font-size: 13px;
  color: #64748b;
}

.about-section {
  text-align: center;
}

.about-logo {
  margin-bottom: 24px;
}

.logo-icon {
  width: 64px;
  height: 64px;
  margin: 0 auto 12px;
}

.logo-icon svg {
  width: 100%;
  height: 100%;
}

.about-logo h3 {
  margin: 0 0 4px;
  font-size: 24px;
  font-weight: 600;
  color: #1e293b;
}

.version {
  margin: 0;
  color: #94a3b8;
  font-size: 14px;
}

.about-links {
  margin-bottom: 20px;
}

.about-links p {
  margin: 8px 0;
  font-size: 14px;
  color: #475569;
}

.about-links a {
  color: #0891b2;
  text-decoration: none;
}

.about-links a:hover {
  text-decoration: underline;
}

.copyright {
  margin: 0;
  font-size: 12px;
  color: #94a3b8;
}
</style>
