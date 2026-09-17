-- Materialized fold of the account-discoverability event stream: one row per
-- (key_id, account_id) pair with its latest link state.
-- NOT history (the events table holds that) and NOT prunable: a pruned row
-- could only be rebuilt by replaying events this node may itself have pruned.
CREATE TABLE account_key_links (
    key_id                          BYTEA    NOT NULL,  -- blake2b256(flag || raw_bytes)
    account_id                      BYTEA    NOT NULL,
    scheme                          SMALLINT NOT NULL,  -- signature scheme flag
    source                          SMALLINT NOT NULL,  -- 0 attach, 1 rotate, 2 detach, 3 claim
    status                          SMALLINT NOT NULL,  -- 0 active, 1 unlinked (tombstone)
    last_change_tx_sequence_number  BIGINT   NOT NULL,
    last_change_epoch               BIGINT   NOT NULL,
    PRIMARY KEY (key_id, account_id)
);
CREATE INDEX account_key_links_account ON account_key_links (account_id);
CREATE INDEX account_key_links_active  ON account_key_links (key_id) WHERE status = 0;

-- Accounts created by a ClaimAccount transaction. Written only from
-- SmartAccountClaimed. Absence means the account was not claimed.
-- Not prunable, for the same reason as account_key_links.
CREATE TABLE claimed_accounts (
    account_id                BYTEA    NOT NULL PRIMARY KEY,
    key_id                    BYTEA    NOT NULL,  -- the key that claimed the address
    immutable                 BOOLEAN  NOT NULL,
    claim_tx_sequence_number  BIGINT   NOT NULL,
    claim_epoch               BIGINT   NOT NULL
);
CREATE INDEX claimed_accounts_key ON claimed_accounts (key_id);
