-- Backward history table: stores the *previous* state of objects when they are
-- superseded by a new checkpoint. Combined with `checkpointed_objects`, this
-- enables consistent view queries via backward diff without the lag and
-- duplication problems of `objects_snapshot`.
CREATE TABLE objects_backward_history (
    object_id                   bytea         NOT NULL,
    object_version              bigint        NOT NULL,
    object_status               smallint      NOT NULL,
    object_digest               bytea,
    superseded_at_checkpoint    bigint        NOT NULL,
    owner_type                  smallint,
    owner_id                    bytea,
    object_type                 text,
    object_type_package         bytea,
    object_type_module          text,
    object_type_name            text,
    serialized_object           bytea,
    coin_type                   text,
    coin_balance                bigint,
    df_kind                     smallint,
    CONSTRAINT objects_backward_history_pk PRIMARY KEY (superseded_at_checkpoint, object_id, object_version)
);

CREATE INDEX objects_backward_history_id_version
    ON objects_backward_history (object_id, object_version, superseded_at_checkpoint);

CREATE INDEX objects_backward_history_owner
    ON objects_backward_history (superseded_at_checkpoint, owner_type, owner_id)
    WHERE owner_type >= 1 AND owner_type <= 2 AND owner_id IS NOT NULL;

-- Supports the dedupe step of the DF listing path in
-- `backward_view/dynamic_fields.rs` and `consistent.rs` (with
-- `apply_filter`): an Index Only Scan that enumerates distinct DF
-- `object_id`s owned by a given parent without scanning every
-- backward-history row of those DFs. See issue #11535.
CREATE INDEX objects_backward_history_owner_df
    ON objects_backward_history (owner_id, object_id)
    WHERE owner_type = 2 AND df_kind IS NOT NULL;

CREATE INDEX objects_backward_history_owner_package_module_name_full_type
    ON objects_backward_history (superseded_at_checkpoint, owner_id, object_type_package, object_type_module, object_type_name, object_type);

CREATE INDEX objects_backward_history_package_module_name_full_type
    ON objects_backward_history (superseded_at_checkpoint, object_type_package, object_type_module, object_type_name, object_type);

CREATE INDEX objects_backward_history_type
    ON objects_backward_history (superseded_at_checkpoint, object_type);

CREATE INDEX objects_backward_history_coin_only
    ON objects_backward_history (superseded_at_checkpoint, coin_type, object_id)
    WHERE coin_type IS NOT NULL;

CREATE INDEX objects_backward_history_coin_owner
    ON objects_backward_history (superseded_at_checkpoint, owner_id, coin_type, object_id)
    WHERE coin_type IS NOT NULL AND owner_type = 1;

CREATE STATISTICS objects_backward_history_type_stats (dependencies, mcv)
    ON object_type, object_type_package, object_type_module, object_type_name
    FROM objects_backward_history;

-- Initialize a watermark entry for backward history. The table starts empty,
-- so copy the upper bound from an existing committer table (all committer
-- tables share the same upper bound) and set the lower bound to indicate
-- that no backward history data is available yet.
INSERT INTO watermarks (
    entity, current_epoch, max_committed_cp, max_committed_tx,
    min_available_epoch, min_bounds_updated_at_timestamp_ms, lowest_unpruned_key,
    min_available_tx, min_available_cp
)
SELECT
    'objects_backward_history',
    current_epoch,
    max_committed_cp,
    max_committed_tx,
    current_epoch,                                    -- min_available_epoch
    (EXTRACT(EPOCH FROM CURRENT_TIMESTAMP) * 1000)::bigint, -- min_bounds_updated_at_timestamp_ms
    max_committed_cp + 1,                             -- lowest_unpruned_key (next cp is first unpruned)
    max_committed_tx + 1,                             -- min_available_tx
    max_committed_cp + 1                              -- min_available_cp
FROM watermarks
WHERE entity = 'transactions'
ON CONFLICT (entity) DO NOTHING;
