-- Add down migration script here
SELECT remove_continuous_aggregate_policy('scores_per_minute');

SELECT remove_continuous_aggregate_policy('scores_daily_historic');

DROP MATERIALIZED VIEW scores_daily_historic;

DROP MATERIALIZED VIEW scores_per_minute;
