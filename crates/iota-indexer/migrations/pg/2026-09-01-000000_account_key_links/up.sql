-- The materialized fold of the account-discoverability event stream: one row
-- per (key_id, account_id) pair holding the latest state of that link.
--
-- This is derived state, not history -- the events table holds the history --
-- so the pruner must never touch it: a pruned row cannot be rebuilt without
-- replaying events that may themselves already be gone.
CREATE TABLE account_key_links (
    key_id                          BYTEA        NOT NULL,
    account_id                      BYTEA        NOT NULL,
    -- signature scheme flag of the controlling key
    scheme                          SMALLINT     NOT NULL,
    -- provenance of the latest change: 0 = claim, 1 = attach, 2 = rotate, 3 = detach
    source                          SMALLINT     NOT NULL,
    -- 0 = active, 1 = unlinked (tombstone, kept for "you used to control this" recovery)
    status                          SMALLINT     NOT NULL,
    last_change_tx_sequence_number  BIGINT       NOT NULL,
    last_change_epoch               BIGINT       NOT NULL,
    PRIMARY KEY (key_id, account_id)
);

-- Reverse lookup: which keys control a given account.
CREATE INDEX account_key_links_account ON account_key_links (account_id);

-- The hot path -- discovery by key -- only ever reads active links.
CREATE INDEX account_key_links_active ON account_key_links (key_id) WHERE status = 0;
