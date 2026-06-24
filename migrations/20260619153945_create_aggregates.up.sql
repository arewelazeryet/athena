-- Add up migration script here
-- add HLL support
CREATE EXTENSION IF NOT EXISTS hll;

-- Baseline s/m aggregate
CREATE MATERIALIZED VIEW scores_per_minute
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 minute', ended_at) AS bucket,
    ruleset_id,
    CASE
        WHEN lazer IS TRUE THEN 1
        ELSE 0
    END AS client_type,

    -- unique IDs
    hll_add_agg(hll_hash_bigint(user_id)) AS user_hll,
    hll_add_agg(hll_hash_bigint(beatmap_id)) AS beatmap_hll,

    -- score throughput
    COUNT(*) AS total_scores_per_min,
    COUNT(*) FILTER (WHERE has_replay = TRUE) AS replays_per_min,
    COUNT(*) FILTER (WHERE is_perfect_combo = TRUE) AS perfect_combos_per_min,

    -- pp
    MIN(pp) AS min_pp,
    MAX(pp) AS max_pp,
    SUM(pp) AS sum_pp,

    -- score sums
    SUM(total_score) AS sum_total_score,
    SUM(classic_total_score) AS sum_classic_total_score,
    SUM(legacy_total_score) AS sum_legacy_total_score,

    -- highest score
    MAX(classic_total_score) AS max_classic_total_score,
    MAX(legacy_total_score) AS max_legacy_total_score,

    -- other
    SUM(accuracy) AS sum_accuracy,
    MAX(max_combo) AS peak_combo
FROM scores
GROUP BY bucket, ruleset_id, client_type WITH NO DATA;

SELECT add_continuous_aggregate_policy('scores_per_minute',
    start_offset => INTERVAL '5 minutes',
    end_offset => INTERVAL '0 minutes',
    schedule_interval => INTERVAL '1 minute');

-- We need to fill the gap before this anyway
-- SELECT add_retention_policy('scores_per_minute', drop_after => INTERVAL '2 days');

CREATE MATERIALIZED VIEW scores_daily_historic
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', bucket) AS day_bucket,
    ruleset_id,
    client_type,

    hll_union_agg(user_hll) AS user_hll,
    hll_union_agg(beatmap_hll) AS beatmap_hll,

    SUM(total_scores_per_min) AS total_daily_scores,
    SUM(replays_per_min) AS daily_scores_with_replays,
    SUM(perfect_combos_per_min) AS daily_perfect_combos,

    MIN(min_pp) AS daily_min_pp,
    MAX(max_pp) AS daily_max_pp,
    SUM(sum_pp) AS daily_sum_pp,

    SUM(sum_total_score) AS daily_sum_total_score,
    SUM(sum_classic_total_score) AS daily_sum_classic_total_score,
    SUM(sum_legacy_total_score) AS daily_sum_legacy_total_score,

    MAX(max_classic_total_score) AS daily_max_classic_total_score,
    MAX(max_legacy_total_score) AS daily_max_legacy_total_score,

    SUM(sum_accuracy) AS daily_sum_accuracy,
    MAX(peak_combo) AS daily_peak_combo
FROM scores_per_minute
GROUP BY day_bucket, ruleset_id, client_type WITH NO DATA;

SELECT add_continuous_aggregate_policy('scores_daily_historic',
    start_offset => INTERVAL '50 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 day',
    initial_start => '2025-06-20 00:00:00',
    timezone => 'UTC');
