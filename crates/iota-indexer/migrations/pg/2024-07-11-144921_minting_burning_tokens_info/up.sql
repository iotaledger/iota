ALTER TABLE epochs
ADD COLUMN burnt_tokens_amount   BIGINT,
ADD COLUMN minted_tokens_amount  BIGINT;

ALTER TABLE epochs
RENAME COLUMN leftover_storage_fund_inflow TO burnt_leftover_amount;