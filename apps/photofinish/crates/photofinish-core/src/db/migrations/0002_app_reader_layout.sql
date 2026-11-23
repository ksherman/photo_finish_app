-- PhotoFinish Machine-Local App Database Schema v2
-- Adds persistent reader grouping (e.g. Gym A / Gym B), custom display labels
-- (e.g. Floor / Beam), and left-to-right sort order keyed to hardware reader_id.

ALTER TABLE reader_configs ADD COLUMN custom_label TEXT;
ALTER TABLE reader_configs ADD COLUMN group_name TEXT;
ALTER TABLE reader_configs ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
