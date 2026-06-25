ALTER TABLE providers ADD COLUMN health_status TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE providers ADD COLUMN last_health_check_at DATETIME;
ALTER TABLE providers ADD COLUMN metadata_json TEXT;

ALTER TABLE routing_rules ADD COLUMN match_host TEXT;
ALTER TABLE routing_rules ADD COLUMN inbound_protocol TEXT NOT NULL DEFAULT 'any';
ALTER TABLE routing_rules ADD COLUMN outbound_protocol TEXT NOT NULL DEFAULT 'openai_response';
ALTER TABLE routing_rules ADD COLUMN failover_provider_id TEXT REFERENCES providers(id);
ALTER TABLE routing_rules ADD COLUMN metadata_json TEXT;

ALTER TABLE request_logs ADD COLUMN route_id TEXT REFERENCES routing_rules(id);
ALTER TABLE request_logs ADD COLUMN inbound_protocol TEXT;
ALTER TABLE request_logs ADD COLUMN outbound_protocol TEXT;
ALTER TABLE request_logs ADD COLUMN status_code INTEGER;
ALTER TABLE request_logs ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0);
ALTER TABLE request_logs ADD COLUMN stream_enabled INTEGER NOT NULL DEFAULT 0 CHECK(stream_enabled IN (0, 1));
ALTER TABLE request_logs ADD COLUMN cache_hit INTEGER NOT NULL DEFAULT 0 CHECK(cache_hit IN (0, 1));
ALTER TABLE request_logs ADD COLUMN request_size_bytes INTEGER;
ALTER TABLE request_logs ADD COLUMN response_size_bytes INTEGER;
ALTER TABLE request_logs ADD COLUMN error_code TEXT;

CREATE INDEX IF NOT EXISTS idx_routing_rules_host_method_priority
ON routing_rules(match_host, match_method, enabled, priority);

CREATE INDEX IF NOT EXISTS idx_request_logs_route_timestamp
ON request_logs(route_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_request_logs_protocol_timestamp
ON request_logs(inbound_protocol, timestamp DESC);
