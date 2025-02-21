ALTER TABLE epochs
DROP COLUMN burnt_tokens_amount,
DROP COLUMN minted_tokens_amount;

ALTER TABLE epochs
RENAME COLUMN burnt_leftover_amount TO leftover_storage_fund_inflow;