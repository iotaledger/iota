-- Materialized fold of the account-discoverability event stream: one row per
-- (key_id, account_id) pair with its latest link state.
-- NOT history (the events table holds that) and NOT prunable: a pruned row
-- could only be rebuilt by replaying events this node may itself have pruned.
CREATE TABLE account_key_links (
    key_id                          BYTEA    NOT NULL,  -- blake2b256(flag || raw_bytes)
    account_id                      BYTEA    NOT NULL,
    scheme                          SMALLINT NOT NULL,  -- signature scheme flag
    source                          SMALLINT NOT NULL,  -- 0 attach, 1 rotate, 2 detach
    status                          SMALLINT NOT NULL,  -- 0 active, 1 unlinked (tombstone)
    last_change_tx_sequence_number  BIGINT   NOT NULL,
    last_change_epoch               BIGINT   NOT NULL,
    PRIMARY KEY (key_id, account_id)
);
CREATE INDEX account_key_links_account ON account_key_links (account_id);
CREATE INDEX account_key_links_active  ON account_key_links (key_id) WHERE status = 0;

-- Every framework SmartAccount, with or without a built-in key. Written only
-- from SmartAccountCreated. An account_key_links row with no match here is some
-- other object a key was attached to. Not prunable, for the same reason as
-- account_key_links.
CREATE TABLE smart_accounts (
    account_id                  BYTEA    NOT NULL PRIMARY KEY,
    immutable                   BOOLEAN  NOT NULL,
    created_tx_sequence_number  BIGINT   NOT NULL,
    created_epoch               BIGINT   NOT NULL
);

-- The current authenticator kind of every framework SmartAccount, with or
-- without a built-in key: 1 ed25519, 2 secp256k1, 3 secp256r1, 4 multisig,
-- 5 passkey (the built-in authenticators), 6 custom. Written from the
-- iota::account creation and authenticator-rotation events. Not prunable, for
-- the same reason as account_key_links.
CREATE TABLE account_authenticators (
    account_id                      BYTEA    NOT NULL PRIMARY KEY,
    kind                            SMALLINT NOT NULL,
    last_change_tx_sequence_number  BIGINT   NOT NULL,
    last_change_epoch               BIGINT   NOT NULL
);
