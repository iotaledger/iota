// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
                            yield paste::paste! {
                                $response_type::default()
                                    .[<with_ $items_field>](current_batch)
                                    .[<with_ $has_next_field>](true)
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
                            yield paste::paste! {
                                $response_type::default()
                                    .[<with_ $items_field>](current_batch)
                                    .[<with_ $has_next_field>](false)
                            };
                        } else if !has_yielded {
                            // Return empty response if we haven't yielded anything yet
                            yield paste::paste! {
                                $response_type::default()
                                    .[<with_ $items_field>](vec![])
                                    .[<with_ $has_next_field>](false)
                            };
                        }
                        break;
                    }
                }
            }
        }
    };
}

/// Appends IOTA-specific metadata headers to a gRPC response.
///
/// This macro simplifies adding checkpoint and blockchain metadata headers
/// to gRPC responses without repeating boilerplate code.
/// Tonic does not currently support interceptors that can modify responses,
/// so this macro provides a convenient way to append headers directly.
///
/// # Example
/// ```ignore
/// let response = Response::new(result);
/// Ok(append_info_headers!(response, self.reader))
/// ```
#[macro_export]
macro_rules! append_info_headers {
    ($response:expr, $reader:expr) => {{ $crate::append_info_headers($reader, $response) }};
}
