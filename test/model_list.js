// get-models.js
const BASE_URL = "https://apihub.agnes-ai.com/v1/models";
// 本地无鉴权随便填，有密钥就替换 sk-xxx
const API_KEY = "sk-P1cD4xVrLCfdxemQOXpw3FJAvjx8ue5aH5AaZnuIrU9ATf9A";

async function fetchModels() {
  try {
    const res = await fetch(BASE_URL, {
      method: "GET",
      headers: {
        "Authorization": `Bearer ${API_KEY}`,
        "Content-Type": "application/json"
      }
    });

    if (!res.ok) {
      throw new Error(`请求失败，状态码：${res.status}`);
    }

    const json = await res.json();
    console.log("完整接口响应：");
    console.log(JSON.stringify(json, null, 2));

    // 提取所有模型 id
    const modelIds = json.data.map(item => item.id);
    console.log("\n=== 可用模型列表 ===");
    console.log(modelIds);
  } catch (err) {
    console.error("拉取模型失败：", err.message);
  }
}

fetchModels();