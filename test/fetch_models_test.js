// 模拟后端 fetch_provider_models 的完整流程
// 用于验证 URL 构造和响应解析是否匹配

const TEST_CASES = [
  {
    name: "Agines Hub",
    base_url: "https://apihub.agnes-ai.com",
    api_key: "sk-P1cD4xVrLCfdxemQOXpw3FJAvjx8ue5aH5AaZnuIrU9ATf9A",
  },
];

// ---- 模拟后端 normalize_api_base_url ----
function normalizeUrl(url) {
  let trimmed = url.replace(/\/+$/, '');      // trim_end_matches('/')
  if (trimmed.endsWith('/v1')) {
    return trimmed.slice(0, -3).replace(/\/+$/, '');
  }
  return trimmed;
}

// ---- 模拟后端 ProviderModelInfo 解析 ----
function parseModels(json) {
  // 检查 success 字段
  if (json.success !== undefined && json.success !== null) {
    if (json.success !== true) {
      return { error: json.message || "API 返回失败状态" };
    }
  }

  const data = json.data;
  if (!Array.isArray(data)) {
    return { error: "响应中未找到模型列表 (data 字段)" };
  }

  const models = data
    .filter(item => item.id)
    .map(item => ({
      id: item.id,
      object: item.object || null,
      created: item.created ?? null,
      owned_by: item.owned_by || null,
      supported_endpoint_types: Array.isArray(item.supported_endpoint_types)
        ? item.supported_endpoint_types
        : [],
    }));

  if (models.length === 0) {
    return { error: "未获取到任何模型" };
  }

  models.sort((a, b) => a.id.localeCompare(b.id));
  return { models };
}

// ---- 测试入口 ----
async function runTest({ name, base_url, api_key }) {
  console.log(`\n========== 测试: ${name} ==========`);

  // 1. 测试 URL 构造
  const normalized = normalizeUrl(base_url);
  const testUrl = `${normalized}/v1/models`;
  console.log(`原始 URL : ${base_url}`);
  console.log(`规范化   : ${normalized}`);
  console.log(`最终请求 : ${testUrl}`);
  console.log(`API Key  : ${api_key.slice(0, 8)}...`);

  // 2. 发送请求
  console.log(`\n发送请求...`);
  try {
    const res = await fetch(testUrl, {
      method: "GET",
      headers: {
        "Authorization": `Bearer ${api_key}`,
        "Content-Type": "application/json",
      },
    });

    console.log(`HTTP ${res.status} ${res.statusText}`);

    if (!res.ok) {
      const body = await res.text();
      console.log(`响应体: ${body.slice(0, 300)}`);
      return;
    }

    const text = await res.text();
    let json;
    try {
      json = JSON.parse(text);
    } catch (e) {
      console.log(`JSON 解析失败: ${e.message}`);
      console.log(`原始内容: ${text.slice(0, 300)}`);
      return;
    }

    // 3. 打印完整响应（缩略）
    console.log(`\n完整响应 (截取前 1000 字符):`);
    const jsonStr = JSON.stringify(json, null, 2);
    console.log(jsonStr.length > 1000 ? jsonStr.slice(0, 1000) + "\n..." : jsonStr);

    // 4. 模拟后端解析
    console.log(`\n--- 后端解析结果 ---`);
    const result = parseModels(json);
    if (result.error) {
      console.log(`❌ 解析失败: ${result.error}`);
      return;
    }

    console.log(`✅ 共获取 ${result.models.length} 个模型:`);
    result.models.forEach(m => {
      const owner = m.owned_by ? ` [${m.owned_by}]` : '';
      const endpoints = m.supported_endpoint_types.length > 0
        ? ` 端点: ${m.supported_endpoint_types.join(', ')}`
        : '';
      console.log(`   ${m.id}${owner}${endpoints}`);
    });
  } catch (err) {
    console.log(`❌ 请求异常: ${err.message}`);
  }
}

async function main() {
  // 运行所有测试用例
  for (const tc of TEST_CASES) {
    await runTest(tc);
  }

  // 额外：测试各种 URL 输入的规范化结果
  console.log(`\n========== URL 规范化测试 ==========`);
  const urls = [
    "https://api.openai.com",
    "https://api.openai.com/v1",
    "https://api.openai.com/v1/",
    "https://api.anthropic.com",
    "https://api.anthropic.com/v1",
    "https://apihub.agnes-ai.com",
    "https://apihub.agnes-ai.com/v1",
    "https://custom.proxy.com/some/path",
  ];
  for (const url of urls) {
    const norm = normalizeUrl(url);
    const final = `${norm}/v1/models`;
    console.log(`  ${url.padEnd(45)} → ${norm.padEnd(40)} → ${final}`);
  }
}

main().catch(console.error);
