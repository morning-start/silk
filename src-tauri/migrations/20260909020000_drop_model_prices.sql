-- 删除金额相关列（价格设置与统计已整体移除）。
ALTER TABLE model_mappings DROP COLUMN input_price_per_1m;
ALTER TABLE model_mappings DROP COLUMN output_price_per_1m;
