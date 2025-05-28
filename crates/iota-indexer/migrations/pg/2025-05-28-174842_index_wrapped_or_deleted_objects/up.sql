CREATE TABLE tx_wrapped_or_deleted_objects (
                                  tx_sequence_number          BIGINT       NOT NULL,
                                  object_id                   BYTEA        NOT NULL,
                                  sender                      BYTEA        NOT NULL,
                                  PRIMARY KEY(object_id, tx_sequence_number)
);
CREATE INDEX tx_wrapped_or_deleted_objects_tx_sequence_number_index ON tx_wrapped_or_deleted_objects (tx_sequence_number);
CREATE INDEX tx_wrapped_or_deleted_objects_sender ON tx_wrapped_or_deleted_objects (sender, object_id, tx_sequence_number);