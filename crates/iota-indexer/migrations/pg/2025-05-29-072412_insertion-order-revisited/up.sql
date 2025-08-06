DROP TABLE IF EXISTS tx_insertion_order;

-- It provides common ordering for optimistic and checkpointed transactions, whereas
-- `tx_digests.tx_sequence_number` provides ordering only for checkpointed transactions.
--
-- The `sequence_number` in this table defaults to the transaction sequence
-- number assigned in the checkpoint for checkpointed transactions, and to the
-- `SELECT MAX(tx_sequence_number) FROM tx_digests` at the time of insertion
-- for optimistic transactions.
--
-- Deterministic global order is guaranteed by the composite index on
-- `(global_sequence_number, optimistic_sequence_number)`, where
-- `optimistic_sequence_number` is the monotically increasing number
-- that represents the order of execution for optimistic transactions.
--
-- In case of missing digests, the `tx_digests` table is used as a fallback
-- to resolve the transaction order. This is ok because optimistic transactions
-- will be inserted only after creation of this table.
CREATE TABLE tx_global_order (
    tx_digest               BYTEA        PRIMARY KEY,
    global_sequence_number  BIGINT       NOT NULL,
    optimistic_sequence_number     BIGSERIAL,
    chk_tx_sequence_number      BIGINT
);
CREATE UNIQUE INDEX tx_global_order_seq_digest ON tx_global_order (global_sequence_number, optimistic_sequence_number);
CREATE UNIQUE INDEX tx_global_order_chk_tx_seq_num ON tx_global_order (chk_tx_sequence_number);

ALTER TABLE optimistic_transactions                 RENAME insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_senders                   RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_recipients                RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_input_objects             RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_changed_objects           RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_calls_pkg                 RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_calls_mod                 RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_calls_fun                 RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_tx_kinds                     RENAME tx_insertion_order TO optimistic_sequence_number;

ALTER TABLE optimistic_events                       RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_emit_package           RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_emit_module            RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_struct_package         RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_struct_module          RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_struct_name            RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_struct_instantiation   RENAME tx_insertion_order TO optimistic_sequence_number;
ALTER TABLE optimistic_event_senders                RENAME tx_insertion_order TO optimistic_sequence_number;

ALTER TABLE optimistic_transactions                 ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_senders                   ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_recipients                ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_input_objects             ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_changed_objects           ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_calls_pkg                 ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_calls_mod                 ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_calls_fun                 ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_tx_kinds                     ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;

ALTER TABLE optimistic_events                       ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_emit_package           ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_emit_module            ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_struct_package         ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_struct_module          ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_struct_name            ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_struct_instantiation   ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;
ALTER TABLE optimistic_event_senders                ADD COLUMN IF NOT EXISTS global_sequence_number BIGINT NOT NULL;

ALTER TABLE optimistic_tx_senders                  DROP CONSTRAINT optimistic_tx_senders_pkey;
ALTER TABLE optimistic_tx_recipients               DROP CONSTRAINT optimistic_tx_recipients_pkey;
ALTER TABLE optimistic_tx_input_objects            DROP CONSTRAINT optimistic_tx_input_objects_pkey;
ALTER TABLE optimistic_tx_changed_objects          DROP CONSTRAINT optimistic_tx_changed_objects_pkey;
ALTER TABLE optimistic_tx_calls_pkg                DROP CONSTRAINT optimistic_tx_calls_pkg_pkey;
ALTER TABLE optimistic_tx_calls_mod                DROP CONSTRAINT optimistic_tx_calls_mod_pkey;
ALTER TABLE optimistic_tx_calls_fun                DROP CONSTRAINT optimistic_tx_calls_fun_pkey;
ALTER TABLE optimistic_tx_kinds                    DROP CONSTRAINT optimistic_tx_kinds_pkey;

ALTER TABLE optimistic_events                      DROP CONSTRAINT optimistic_events_pkey;
ALTER TABLE optimistic_event_emit_package          DROP CONSTRAINT optimistic_event_emit_package_pkey;
ALTER TABLE optimistic_event_emit_module           DROP CONSTRAINT optimistic_event_emit_module_pkey;
ALTER TABLE optimistic_event_struct_package        DROP CONSTRAINT optimistic_event_struct_package_pkey;
ALTER TABLE optimistic_event_struct_module         DROP CONSTRAINT optimistic_event_struct_module_pkey;
ALTER TABLE optimistic_event_struct_name           DROP CONSTRAINT optimistic_event_struct_name_pkey;
ALTER TABLE optimistic_event_struct_instantiation  DROP CONSTRAINT optimistic_event_struct_instantiation_pkey;
ALTER TABLE optimistic_event_senders               DROP CONSTRAINT optimistic_event_senders_pkey;

ALTER TABLE optimistic_tx_senders                  DROP CONSTRAINT optimistic_tx_senders_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_recipients               DROP CONSTRAINT optimistic_tx_recipients_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_input_objects            DROP CONSTRAINT optimistic_tx_input_objects_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_changed_objects          DROP CONSTRAINT optimistic_tx_changed_objects_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_calls_pkg                DROP CONSTRAINT optimistic_tx_calls_pkg_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_calls_mod                DROP CONSTRAINT optimistic_tx_calls_mod_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_calls_fun                DROP CONSTRAINT optimistic_tx_calls_fun_tx_insertion_order_fkey;
ALTER TABLE optimistic_tx_kinds                    DROP CONSTRAINT optimistic_tx_kinds_tx_insertion_order_fkey;

ALTER TABLE optimistic_events                      DROP CONSTRAINT optimistic_events_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_emit_package          DROP CONSTRAINT optimistic_event_emit_package_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_emit_module           DROP CONSTRAINT optimistic_event_emit_module_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_struct_package        DROP CONSTRAINT optimistic_event_struct_package_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_struct_module         DROP CONSTRAINT optimistic_event_struct_module_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_struct_name           DROP CONSTRAINT optimistic_event_struct_name_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_struct_instantiation  DROP CONSTRAINT optimistic_event_struct_instantiation_tx_insertion_order_fkey;
ALTER TABLE optimistic_event_senders               DROP CONSTRAINT optimistic_event_senders_tx_insertion_order_fkey;

ALTER TABLE optimistic_transactions                DROP CONSTRAINT optimistic_transactions_pkey;

ALTER TABLE optimistic_transactions                ADD PRIMARY KEY (global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_senders                  ADD PRIMARY KEY (sender, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_recipients               ADD PRIMARY KEY (recipient, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_input_objects            ADD PRIMARY KEY (object_id, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_changed_objects          ADD PRIMARY KEY (object_id, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_calls_pkg                ADD PRIMARY KEY (package, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_calls_mod                ADD PRIMARY KEY (package, module, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_calls_fun                ADD PRIMARY KEY (package, module, func, global_sequence_number, optimistic_sequence_number);
ALTER TABLE optimistic_tx_kinds                    ADD PRIMARY KEY (tx_kind, global_sequence_number, optimistic_sequence_number);

ALTER TABLE optimistic_events                      ADD PRIMARY KEY (global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_emit_package          ADD PRIMARY KEY (package, global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_emit_module           ADD PRIMARY KEY (package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_struct_package        ADD PRIMARY KEY (package, global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_struct_module         ADD PRIMARY KEY (package, module, global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_struct_name           ADD PRIMARY KEY (package, module, type_name, global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_struct_instantiation  ADD PRIMARY KEY (package, module, type_instantiation, global_sequence_number, optimistic_sequence_number, event_sequence_number);
ALTER TABLE optimistic_event_senders               ADD PRIMARY KEY (sender, global_sequence_number, optimistic_sequence_number, event_sequence_number);

-- optimistic_event_emit_package
ALTER TABLE optimistic_event_emit_package
ADD CONSTRAINT optimistic_event_emit_package_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;

-- optimistic_event_emit_module
ALTER TABLE optimistic_event_emit_module
ADD CONSTRAINT optimistic_event_emit_module_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;

-- optimistic_event_sender
ALTER TABLE optimistic_event_senders
ADD CONSTRAINT optimistic_event_senders_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;

-- optimistic_event_struct_package
ALTER TABLE optimistic_event_struct_package
ADD CONSTRAINT optimistic_event_struct_package_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;

-- optimistic_event_struct_module
ALTER TABLE optimistic_event_struct_module
ADD CONSTRAINT optimistic_event_struct_module_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;

-- optimistic_event_struct_name
ALTER TABLE optimistic_event_struct_name
ADD CONSTRAINT optimistic_event_struct_name_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;

-- optimistic_event_struct_instantiation
ALTER TABLE optimistic_event_struct_instantiation
ADD CONSTRAINT optimistic_event_struct_instantiation_fk
FOREIGN KEY (global_sequence_number, optimistic_sequence_number)
REFERENCES optimistic_transactions(global_sequence_number, optimistic_sequence_number)
ON DELETE CASCADE;
