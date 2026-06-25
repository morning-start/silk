-- 网关全局设置表
CREATE TABLE IF NOT EXISTS gateway_settings (
    id                  TEXT PRIMARY KEY CHECK(id = 'default'),
    bind_host           TEXT NOT NULL DEFAULT '127.0.0.1',
    bind_port           INTEGER NOT NULL DEFAULT 3000 CHECK(bind_port > 0 AND bind_port <= 65535),
    allow_remote        INTEGER NOT NULL DEFAULT 0 CHECK(allow_remote IN (0, 1)),
    auth_token_hash     TEXT,
    log_retention_days  INTEGER NOT NULL DEFAULT 30 CHECK(log_retention_days >= 1 AND log_retention_days <= 3650),
    default_provider_id TEXT REFERENCES providers(id),
    default_route_id    TEXT REFERENCES routing_rules(id),
    created_at          DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at          DATETIME NOT NULL DEFAULT (datetime('now'))
);

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
