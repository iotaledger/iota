// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Joins field names with commas for field mask constants.
///
/// # Example
/// ```ignore
/// const MASK: &str = field_mask!(
///     "transaction.digest",
///     "transaction.effects",
///     "command_results"
/// );
/// // Expands to: "transaction.digest,transaction.effects,command_results"
/// ```
#[macro_export]
macro_rules! field_mask {
    // Base case: single field
    ($field:literal) => {
        $field
    };
    // Recursive case: multiple fields
    ($first:literal, $($rest:literal),+ $(,)?) => {
        concat!($first, ",", $crate::field_mask!($($rest),+))
    };
}

/// Creates a lazy batching stream that fetches and batches items on-demand
/// based on message size limits.
///
/// # Example
/// ```ignore
/// create_batching_stream!(
///     requests.into_iter(),
///     (object_id, version),
///     {
///         let result = process(object_id, version);
///         let size = result.encoded_len();
///         (result, size)
///     },
///     max_message_size,
///     GetObjectsResponse,
///     objects,
///     has_next
/// )
/// ```
#[macro_export]
macro_rules! create_batching_stream {
    (
        $requests_iter:expr,
        $item_pattern:pat,
        $process_block:block,
        $max_message_size:expr,
        $response_type:ty,
        $items_field:ident,
        $has_next_field:ident
    ) => {
        async_stream::try_stream! {
            let mut requests_iter = $requests_iter;
            let mut current_batch = Vec::new();
            let mut current_size = 0;
            let mut has_yielded = false;

            loop {
                // Try to get the next item
                match requests_iter.next() {
                    Some($item_pattern) => {
                        // Process the item using the provided block
                        let (result_item, item_size) = $process_block;

                        // Check if a single item exceeds the message size limit
                        if item_size > $max_message_size {
                            Err($crate::error::RpcError::new(
                                tonic::Code::InvalidArgument,
                                format!("Single item size ({} bytes) exceeds max message size ({} bytes)",
                                    item_size, $max_message_size)
                            ))?;
                        }

                        // Check if adding this item would exceed the limit
                        if current_size + item_size > $max_message_size && !current_batch.is_empty() {
                            // Current batch is full, yield it
                            has_yielded = true;
                            yield $response_type {
                                $items_field: current_batch,
                                $has_next_field: true,
                            };
                            // Start new batch with current item
                            current_batch = vec![result_item];
                            current_size = item_size;
                        } else {
                            // Item fits, add to current batch
                            current_batch.push(result_item);
                            current_size += item_size;
                        }
                    }
                    None => {
                        // No more items
                        if !current_batch.is_empty() {
                            yield $response_type {
                                $items_field: current_batch,
                                $has_next_field: false,
                            };
                        } else if !has_yielded {
                            // Return empty response if we haven't yielded anything yet
                            yield $response_type {
                                $items_field: vec![],
                                $has_next_field: false,
                            };
                        }
                        break;
                    }
                }
            }
        }
    };
}
