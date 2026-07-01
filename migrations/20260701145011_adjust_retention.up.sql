-- Add up migration script here
ALTER TABLE scores SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'lazer, ruleset_id',
    timescaledb.compress_orderby = 'ended_at DESC'
);

SELECT add_compression_policy('scores',
    compress_after => INTERVAL '30 days',
    if_not_exists => true
);

SELECT add_retention_policy('scores_per_minute',
    drop_after => INTERVAL '72 hours',
    if_not_exists => true
);
