-- One row for each public key dynamic field
-- `Field<PublicKeyFieldName, PublicKey>` that is attached to a `SmartAccount`
-- object. For a given `SmartAccount` object (of id `account_id`) there is at
-- most one row in this table.
CREATE TABLE smart_account_keys (
    -- The id of the dynamic field that contains a key
    object_id                   bytea         PRIMARY KEY,
    -- Object version of the dynamic field
    object_version              bigint        NOT NULL,
    -- The id of `SmartAccount` the field is attached to.
    account_id                  bytea         NOT NULL,
    -- Hash of public key that is stored in the dynamic field:
    -- blake2b256(scheme flag || raw public key bytes).
    key_id                      bytea         NOT NULL
);

-- User queries would be filtering by `key_id`, and pagination would order by
-- `account_id`.
CREATE INDEX smart_account_keys_key_id ON smart_account_keys (key_id, account_id);
