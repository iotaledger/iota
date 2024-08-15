-- This file should undo anything in `up.sql`

-- These views are placed on top because they depend on tables, if dropping the tables first and then the views it will result in an error.
-- We could use CASCADE or placing them on top before dropping the tables
DROP VIEW network_metrics;
DROP VIEW real_time_tps;

-- Diesel
DROP FUNCTION IF EXISTS diesel_manage_updated_at(_tbl regclass);
DROP FUNCTION IF EXISTS diesel_set_updated_at();

-- Events
DROP TABLE IF EXISTS events;

-- Objects
DROP TABLE IF EXISTS objects;
DROP TABLE IF EXISTS objects_history;
DROP TABLE IF EXISTS objects_snapshot;

-- Transactions
DROP TABLE IF EXISTS transactions;

-- Checkpoints
DROP TABLE IF EXISTS checkpoints;

-- Epochs
DROP TABLE IF EXISTS epochs;

-- Packages
DROP TABLE IF EXISTS packages;

-- Tx Recipients
DROP TABLE IF EXISTS tx_recipients;

-- Tx Senders
DROP TABLE IF EXISTS tx_senders;

-- Tx Input Objects
DROP TABLE IF EXISTS tx_input_objects;

-- Tx Changed Objects
DROP TABLE IF EXISTS tx_changed_objects;

-- Tx calls
DROP TABLE IF EXISTS tx_calls;

-- Display
DROP TABLE IF EXISTS display;

DROP FUNCTION IF EXISTS query_cost(TEXT);
DROP PROCEDURE IF EXISTS advance_partition;

-- Extended Api metrics for the explorer
DROP TABLE IF EXISTS tx_count_metrics;
DROP TABLE IF EXISTS move_calls;
DROP TABLE IF EXISTS move_call_metrics;
DROP TABLE IF EXISTS addresses;
DROP TABLE IF EXISTS active_addresses;
DROP TABLE IF EXISTS address_metrics;
DROP TABLE IF EXISTS epoch_peak_tps;