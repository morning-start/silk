-- gateway_settings 表已迁移为 JSON 配置文件，不再需要数据库存储
DROP TABLE IF EXISTS gateway_settings;
