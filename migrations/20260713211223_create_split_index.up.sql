-- Add up migration script here
CREATE INDEX IF NOT EXISTS idx_scores_lazer_split
ON scores (ended_at DESC, user_id) INCLUDE (lazer);
