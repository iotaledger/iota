// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
// @generated automatically by scripts/indexer-schema/generate.sh

diesel::table! {
    active_addresses (address) {
        address -> Bytea,
        first_appearance_tx -> Int8,
        first_appearance_time -> Int8,
        last_appearance_tx -> Int8,
        last_appearance_time -> Int8,
    }
}

diesel::table! {
    address_metrics (checkpoint) {
        checkpoint -> Int8,
        epoch -> Int8,
        timestamp_ms -> Int8,
        cumulative_addresses -> Int8,
        cumulative_active_addresses -> Int8,
        daily_active_addresses -> Int8,
    }
}

diesel::table! {
    addresses (address) {
        address -> Bytea,
        first_appearance_tx -> Int8,
        first_appearance_time -> Int8,
        last_appearance_tx -> Int8,
        last_appearance_time -> Int8,
    }
}

diesel::table! {
    chain_identifier (checkpoint_digest) {
        checkpoint_digest -> Bytea,
    }
}

diesel::table! {
    checkpoints (sequence_number) {
        sequence_number -> Int8,
        checkpoint_digest -> Bytea,
        epoch -> Int8,
        network_total_transactions -> Int8,
        previous_checkpoint_digest -> Nullable<Bytea>,
        end_of_epoch -> Bool,
        tx_digests -> Array<Nullable<Bytea>>,
        timestamp_ms -> Int8,
        total_gas_cost -> Int8,
        computation_cost -> Int8,
        storage_cost -> Int8,
        storage_rebate -> Int8,
        non_refundable_storage_fee -> Int8,
        checkpoint_commitments -> Bytea,
        validator_signature -> Bytea,
        end_of_epoch_data -> Nullable<Bytea>,
        min_tx_sequence_number -> Nullable<Int8>,
        max_tx_sequence_number -> Nullable<Int8>,
        computation_cost_burned -> Nullable<Int8>,
    }
}

diesel::table! {
    display (object_type) {
        object_type -> Text,
        id -> Bytea,
        version -> Int2,
        bcs -> Bytea,
    }
}

diesel::table! {
    epoch_peak_tps (epoch) {
        epoch -> Int8,
        peak_tps -> Float8,
        peak_tps_30d -> Float8,
    }
}

diesel::table! {
    epochs (epoch) {
        epoch -> Int8,
        first_checkpoint_id -> Int8,
        epoch_start_timestamp -> Int8,
        reference_gas_price -> Int8,
        protocol_version -> Int8,
        total_stake -> Int8,
        storage_fund_balance -> Int8,
        system_state -> Bytea,
        epoch_total_transactions -> Nullable<Int8>,
        last_checkpoint_id -> Nullable<Int8>,
        epoch_end_timestamp -> Nullable<Int8>,
        storage_charge -> Nullable<Int8>,
        storage_rebate -> Nullable<Int8>,
        total_gas_fees -> Nullable<Int8>,
        total_stake_rewards_distributed -> Nullable<Int8>,
        epoch_commitments -> Nullable<Bytea>,
        burnt_tokens_amount -> Nullable<Int8>,
        minted_tokens_amount -> Nullable<Int8>,
    }
}

diesel::table! {
    event_emit_module (package, module, tx_sequence_number, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    event_emit_package (package, tx_sequence_number, event_sequence_number) {
        package -> Bytea,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    event_senders (sender, tx_sequence_number, event_sequence_number) {
        sender -> Bytea,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
    }
}

diesel::table! {
    event_struct_instantiation (package, module, type_instantiation, tx_sequence_number, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        type_instantiation -> Text,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    event_struct_module (package, module, tx_sequence_number, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    event_struct_name (package, module, type_name, tx_sequence_number, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        type_name -> Text,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    event_struct_package (package, tx_sequence_number, event_sequence_number) {
        package -> Bytea,
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    events (tx_sequence_number, event_sequence_number) {
        tx_sequence_number -> Int8,
        event_sequence_number -> Int8,
        transaction_digest -> Bytea,
        senders -> Array<Nullable<Bytea>>,
        package -> Bytea,
        module -> Text,
        event_type -> Text,
        timestamp_ms -> Int8,
        bcs -> Bytea,
    }
}

diesel::table! {
    feature_flags (protocol_version, flag_name) {
        protocol_version -> Int8,
        flag_name -> Text,
        flag_value -> Bool,
    }
}

diesel::table! {
    move_call_metrics (id) {
        id -> Int8,
        epoch -> Int8,
        day -> Int8,
        move_package -> Text,
        move_module -> Text,
        move_function -> Text,
        count -> Int8,
    }
}

diesel::table! {
    move_calls (transaction_sequence_number, move_package, move_module, move_function) {
        transaction_sequence_number -> Int8,
        checkpoint_sequence_number -> Int8,
        epoch -> Int8,
        move_package -> Bytea,
        move_module -> Text,
        move_function -> Text,
    }
}

diesel::table! {
    objects (object_id) {
        object_id -> Bytea,
        object_version -> Int8,
        object_digest -> Bytea,
        owner_type -> Int2,
        owner_id -> Nullable<Bytea>,
        object_type -> Nullable<Text>,
        object_type_package -> Nullable<Bytea>,
        object_type_module -> Nullable<Text>,
        object_type_name -> Nullable<Text>,
        serialized_object -> Bytea,
        coin_type -> Nullable<Text>,
        coin_balance -> Nullable<Int8>,
        df_kind -> Nullable<Int2>,
    }
}

diesel::table! {
    objects_history (checkpoint_sequence_number, object_id, object_version) {
        object_id -> Bytea,
        object_version -> Int8,
        object_status -> Int2,
        object_digest -> Nullable<Bytea>,
        checkpoint_sequence_number -> Int8,
        owner_type -> Nullable<Int2>,
        owner_id -> Nullable<Bytea>,
        object_type -> Nullable<Text>,
        object_type_package -> Nullable<Bytea>,
        object_type_module -> Nullable<Text>,
        object_type_name -> Nullable<Text>,
        serialized_object -> Nullable<Bytea>,
        coin_type -> Nullable<Text>,
        coin_balance -> Nullable<Int8>,
        df_kind -> Nullable<Int2>,
    }
}

diesel::table! {
    objects_snapshot (object_id) {
        object_id -> Bytea,
        object_version -> Int8,
        object_status -> Int2,
        object_digest -> Nullable<Bytea>,
        checkpoint_sequence_number -> Int8,
        owner_type -> Nullable<Int2>,
        owner_id -> Nullable<Bytea>,
        object_type -> Nullable<Text>,
        object_type_package -> Nullable<Bytea>,
        object_type_module -> Nullable<Text>,
        object_type_name -> Nullable<Text>,
        serialized_object -> Nullable<Bytea>,
        coin_type -> Nullable<Text>,
        coin_balance -> Nullable<Int8>,
        df_kind -> Nullable<Int2>,
    }
}

diesel::table! {
    objects_version (object_id, object_version) {
        object_id -> Bytea,
        object_version -> Int8,
        cp_sequence_number -> Int8,
    }
}

diesel::table! {
    optimistic_event_emit_module (package, module, tx_insertion_order, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_event_emit_package (package, tx_insertion_order, event_sequence_number) {
        package -> Bytea,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_event_senders (sender, tx_insertion_order, event_sequence_number) {
        sender -> Bytea,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
    }
}

diesel::table! {
    optimistic_event_struct_instantiation (package, module, type_instantiation, tx_insertion_order, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        type_instantiation -> Text,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_event_struct_module (package, module, tx_insertion_order, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_event_struct_name (package, module, type_name, tx_insertion_order, event_sequence_number) {
        package -> Bytea,
        module -> Text,
        type_name -> Text,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_event_struct_package (package, tx_insertion_order, event_sequence_number) {
        package -> Bytea,
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_events (tx_insertion_order, event_sequence_number) {
        tx_insertion_order -> Int8,
        event_sequence_number -> Int8,
        transaction_digest -> Bytea,
        senders -> Array<Nullable<Bytea>>,
        package -> Bytea,
        module -> Text,
        event_type -> Text,
        bcs -> Bytea,
    }
}

diesel::table! {
    optimistic_transactions (insertion_order) {
        insertion_order -> Int8,
        transaction_digest -> Bytea,
        raw_transaction -> Bytea,
        raw_effects -> Bytea,
        object_changes -> Array<Nullable<Bytea>>,
        balance_changes -> Array<Nullable<Bytea>>,
        events -> Array<Nullable<Bytea>>,
        transaction_kind -> Int2,
        success_command_count -> Int2,
    }
}

diesel::table! {
    optimistic_tx_calls_fun (package, module, func, tx_insertion_order) {
        tx_insertion_order -> Int8,
        package -> Bytea,
        module -> Text,
        func -> Text,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_tx_calls_mod (package, module, tx_insertion_order) {
        tx_insertion_order -> Int8,
        package -> Bytea,
        module -> Text,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_tx_calls_pkg (package, tx_insertion_order) {
        tx_insertion_order -> Int8,
        package -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_tx_changed_objects (object_id, tx_insertion_order) {
        tx_insertion_order -> Int8,
        object_id -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_tx_input_objects (object_id, tx_insertion_order) {
        tx_insertion_order -> Int8,
        object_id -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_tx_kinds (tx_kind, tx_insertion_order) {
        tx_insertion_order -> Int8,
        tx_kind -> Int2,
    }
}

diesel::table! {
    optimistic_tx_recipients (recipient, tx_insertion_order) {
        tx_insertion_order -> Int8,
        recipient -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    optimistic_tx_senders (sender, tx_insertion_order) {
        tx_insertion_order -> Int8,
        sender -> Bytea,
    }
}

diesel::table! {
    packages (package_id, original_id, package_version) {
        package_id -> Bytea,
        original_id -> Bytea,
        package_version -> Int8,
        move_package -> Bytea,
        checkpoint_sequence_number -> Int8,
    }
}

diesel::table! {
    protocol_configs (protocol_version, config_name) {
        protocol_version -> Int8,
        config_name -> Text,
        config_value -> Nullable<Text>,
    }
}

diesel::table! {
    pruner_cp_watermark (checkpoint_sequence_number) {
        checkpoint_sequence_number -> Int8,
        min_tx_sequence_number -> Int8,
        max_tx_sequence_number -> Int8,
    }
}

diesel::table! {
    transactions (tx_sequence_number) {
        tx_sequence_number -> Int8,
        transaction_digest -> Bytea,
        raw_transaction -> Bytea,
        raw_effects -> Bytea,
        checkpoint_sequence_number -> Int8,
        timestamp_ms -> Int8,
        object_changes -> Array<Nullable<Bytea>>,
        balance_changes -> Array<Nullable<Bytea>>,
        events -> Array<Nullable<Bytea>>,
        transaction_kind -> Int2,
        success_command_count -> Int2,
    }
}

diesel::table! {
    tx_calls_fun (package, module, func, tx_sequence_number) {
        tx_sequence_number -> Int8,
        package -> Bytea,
        module -> Text,
        func -> Text,
        sender -> Bytea,
    }
}

diesel::table! {
    tx_calls_mod (package, module, tx_sequence_number) {
        tx_sequence_number -> Int8,
        package -> Bytea,
        module -> Text,
        sender -> Bytea,
    }
}

diesel::table! {
    tx_calls_pkg (package, tx_sequence_number) {
        tx_sequence_number -> Int8,
        package -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    tx_changed_objects (object_id, tx_sequence_number) {
        tx_sequence_number -> Int8,
        object_id -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    tx_count_metrics (checkpoint_sequence_number) {
        checkpoint_sequence_number -> Int8,
        epoch -> Int8,
        timestamp_ms -> Int8,
        total_transaction_blocks -> Int8,
        total_successful_transaction_blocks -> Int8,
        total_successful_transactions -> Int8,
    }
}

diesel::table! {
    tx_digests (tx_digest) {
        tx_digest -> Bytea,
        tx_sequence_number -> Int8,
    }
}

diesel::table! {
    tx_input_objects (object_id, tx_sequence_number) {
        tx_sequence_number -> Int8,
        object_id -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    tx_insertion_order (tx_digest) {
        tx_digest -> Bytea,
        insertion_order -> Int8,
    }
}

diesel::table! {
    tx_kinds (tx_kind, tx_sequence_number) {
        tx_sequence_number -> Int8,
        tx_kind -> Int2,
    }
}

diesel::table! {
    tx_recipients (recipient, tx_sequence_number) {
        tx_sequence_number -> Int8,
        recipient -> Bytea,
        sender -> Bytea,
    }
}

diesel::table! {
    tx_senders (sender, tx_sequence_number) {
        tx_sequence_number -> Int8,
        sender -> Bytea,
    }
}

diesel::joinable!(optimistic_event_emit_module -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_event_emit_package -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_event_senders -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_event_struct_instantiation -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_event_struct_module -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_event_struct_name -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_event_struct_package -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_events -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_calls_fun -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_calls_mod -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_calls_pkg -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_changed_objects -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_input_objects -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_kinds -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_recipients -> optimistic_transactions (tx_insertion_order));
diesel::joinable!(optimistic_tx_senders -> optimistic_transactions (tx_insertion_order));

#[macro_export]
macro_rules! for_all_tables {
    ($action:path) => {
        $action!(
            active_addresses,
            address_metrics,
            addresses,
            chain_identifier,
            checkpoints,
            display,
            epoch_peak_tps,
            epochs,
            event_emit_module,
            event_emit_package,
            event_senders,
            event_struct_instantiation,
            event_struct_module,
            event_struct_name,
            event_struct_package,
            events,
            feature_flags,
            move_call_metrics,
            move_calls,
            objects,
            objects_history,
            objects_snapshot,
            objects_version,
            optimistic_event_emit_module,
            optimistic_event_emit_package,
            optimistic_event_senders,
            optimistic_event_struct_instantiation,
            optimistic_event_struct_module,
            optimistic_event_struct_name,
            optimistic_event_struct_package,
            optimistic_events,
            optimistic_transactions,
            optimistic_tx_calls_fun,
            optimistic_tx_calls_mod,
            optimistic_tx_calls_pkg,
            optimistic_tx_changed_objects,
            optimistic_tx_input_objects,
            optimistic_tx_kinds,
            optimistic_tx_recipients,
            optimistic_tx_senders,
            packages,
            protocol_configs,
            pruner_cp_watermark,
            transactions,
            tx_calls_fun,
            tx_calls_mod,
            tx_calls_pkg,
            tx_changed_objects,
            tx_count_metrics,
            tx_digests,
            tx_input_objects,
            tx_insertion_order,
            tx_kinds,
            tx_recipients,
            tx_senders
        );
    };
}
pub use for_all_tables;
for_all_tables!(diesel::allow_tables_to_appear_in_same_query);
