-- We skip the down migration as we wouldn't able to restore
-- the data of the most recent epoch.
--
-- Adding a noop query to enable side effects on diesel
-- reversals.
SELECT 'noop';
