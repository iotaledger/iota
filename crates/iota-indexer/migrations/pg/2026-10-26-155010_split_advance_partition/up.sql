CREATE OR REPLACE PROCEDURE alter_partition_range(table_name TEXT, partition BIGINT, range_start BIGINT, range_end BIGINT)
LANGUAGE plpgsql
AS $$
BEGIN
    EXECUTE format('ALTER TABLE %I DETACH PARTITION %I_partition_%s', table_name, table_name, partition);
    EXECUTE format('ALTER TABLE %I ATTACH PARTITION %I_partition_%s FOR VALUES FROM (%L) TO (%L)', table_name, table_name, partition, range_start, range_end);
END;
$$;

CREATE OR REPLACE PROCEDURE create_partition(table_name TEXT, partition BIGINT, range_start BIGINT)
LANGUAGE plpgsql
AS $$
BEGIN
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I_partition_%s PARTITION OF %I FOR VALUES FROM (%L) TO (MAXVALUE)', table_name, partition, table_name, range_start);
END;
$$;

DROP PROCEDURE IF EXISTS advance_partition;
