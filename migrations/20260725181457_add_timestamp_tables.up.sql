-- Add up migration script here
CREATE TABLE IF NOT EXISTS measurements (
    inserted_at TIMESTAMPTZ NOT NULL PRIMARY KEY,
    stable BIGINT NOT NULL,
    lazer BIGINT NOT NULL
);

SELECT create_hypertable('measurements', by_range('inserted_at'), if_not_exists => TRUE);

-- Daily peaks
CREATE MATERIALIZED VIEW changelog_counts_daily_aggregate
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', inserted_at) AS day_bucket,
    MAX(stable) as stable_peak,
    MAX(lazer) as lazer_peak,
    CAST(AVG(stable) AS BIGINT) as stable_avg,
    CAST(AVG(lazer) AS BIGINT) as lazer_avg,
    MIN(stable) as stable_min,
    MIN(lazer) as lazer_min
FROM measurements
GROUP BY day_bucket WITH NO DATA;

SELECT add_continuous_aggregate_policy('changelog_counts_daily_aggregate',
    start_offset => INTERVAL '50 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 day',
    initial_start => '2026-05-23 00:00:00',
    timezone => 'UTC');
