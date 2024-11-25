// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::system_display {

    use std::string;

    use iota::balance::Balance;
    use iota::iota::IOTA;
    use iota::timelock::TimeLock;

    use iota_system::iota_system::IotaSystemState;
    use iota_system::staking_pool::StakedIota;
    use iota_system::timelocked_staking::TimelockedStakedIota;

    #[allow(unused_function)]
    /// Create a `Display` object for the `StakedIota` type.
    fun create_staked_iota_display_v1(
        iota_system: &mut IotaSystemState,
        ctx: &mut TxContext,
    ) {
        // A display object content.
        let keys = vector[
            string::utf8(b"principal"),
            string::utf8(b"stake_activation_epoch"),
        ];

        let values = vector[
            string::utf8(b"{principal}"),
            string::utf8(b"{stake_activation_epoch}"),
        ];
    
        // Create a display object.
        let mut display = iota_system.new_system_display_with_fields<StakedIota>(keys, values, ctx);

        // Commit the display object to apply changes.
        display.update_version();

        // Store the display object.
        iota_system.insert_system_display_object(display);
    }

    #[allow(unused_function)]
    /// Create a `Display` object for the `TimelockedStakedIota` type.
    fun create_timelocked_staked_iota_display_v1(
        iota_system: &mut IotaSystemState,
        ctx: &mut TxContext,
    ) {
        // A display object content.
        let keys = vector[
            string::utf8(b"principal"),
            string::utf8(b"stake_activation_epoch"),
            string::utf8(b"expiration_timestamp_ms"),
            string::utf8(b"label"),
        ];

        let values = vector[
            string::utf8(b"{staked_iota.principal}"),
            string::utf8(b"{staked_iota.stake_activation_epoch}"),
            string::utf8(b"{expiration_timestamp_ms}"),
            string::utf8(b"{label}"),
        ];
    
        // Create a display object.
        let mut display = iota_system.new_system_display_with_fields<TimelockedStakedIota>(keys, values, ctx);

        // Commit the display object to apply changes.
        display.update_version();

        // Store the display object.
        iota_system.insert_system_display_object(display);
    }

    #[allow(unused_function)]
    /// Create a `Display` object for the `TimeLock<Balance<IOTA>>` type.
    fun create_timelocked_iota_display_v1(
        iota_system: &mut IotaSystemState,
        ctx: &mut TxContext,
    ) {
        // A display object content.
        let keys = vector[
            string::utf8(b"locked"),
            string::utf8(b"expiration_timestamp_ms"),
        ];

        let values = vector[
            string::utf8(b"{locked}"),
            string::utf8(b"{expiration_timestamp_ms}"),
        ];
    
        // Create a display object.
        let mut display = iota_system.new_system_display_with_fields<TimeLock<Balance<IOTA>>>(keys, values, ctx);

        // Commit the display object to apply changes.
        display.update_version();

        // Store the display object.
        iota_system.insert_system_display_object(display);
    }
}
