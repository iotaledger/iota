CREATE OR REPLACE FUNCTION notify_checkpoint_committed()
RETURNS TRIGGER AS $$
BEGIN
    -- Send notification with just the range - let client query events
    PERFORM pg_notify('checkpoint_committed',
        json_build_object(
            'checkpoint_sequence_number', NEW.sequence_number,
            'min_tx_sequence_number', NEW.min_tx_sequence_number,
            'max_tx_sequence_number', NEW.max_tx_sequence_number
        )::text
    );

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;


CREATE TRIGGER checkpoint_committed_trigger
    AFTER INSERT ON checkpoints
    FOR EACH ROW
    EXECUTE FUNCTION notify_checkpoint_committed();
