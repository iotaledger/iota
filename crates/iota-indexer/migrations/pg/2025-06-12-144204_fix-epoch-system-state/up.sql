UPDATE epochs as cur
SET system_state = prev.system_state
FROM epochs as prev
WHERE cur.epoch = prev.epoch + 1;
