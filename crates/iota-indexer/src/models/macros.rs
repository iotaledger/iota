// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Implement `From` optimistic records to checkpoint records and vice
/// versa.
///
/// # Example
///
/// ```ignore
/// optimistic_from_into_checkpoint!(OptimisticEventEmitPackage, StoredEventEmitPackage, {
///    event_sequence_number,
///    package,
///    sender,
/// });
/// ```
macro_rules! optimistic_from_into_checkpoint {
    ($optimistic_record:ident, $checkpoint_record:ident, { $($field:ident),* $(,)? }) => {
        impl From<$optimistic_record> for $checkpoint_record {
            fn from(item: $optimistic_record) -> Self {
                Self {
                    tx_sequence_number: item.tx_insertion_order,
                    $($field: item.$field),*
                }
            }
        }

        impl From<$checkpoint_record> for $optimistic_record {
            fn from(item: $checkpoint_record) -> Self {
                Self {
                    tx_insertion_order: item.tx_sequence_number,
                    $($field: item.$field),*
                }
            }
        }
    };
}
