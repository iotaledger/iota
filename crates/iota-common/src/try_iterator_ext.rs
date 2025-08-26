// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub trait TryIteratorExt<T, E>: Iterator<Item = Result<T, E>> + Sized {
    /// Tries to collect items from an iterator, stopping early on first error,
    /// with an optional limit and optional take_while filter_fn.
    /// This avoids creating an intermediate Vec when we just need to propagate
    /// errors.
    fn try_take_while_map_and_collect<U, F, P>(
        self,
        limit: Option<usize>,
        filter_fn: Option<P>,
        map_fn: F,
    ) -> Result<Vec<U>, E>
    where
        F: FnMut(T) -> U,
        P: FnMut(&T) -> bool;
}

impl<I, T, E> TryIteratorExt<T, E> for I
where
    I: Iterator<Item = Result<T, E>>,
{
    fn try_take_while_map_and_collect<U, F, P>(
        self,
        limit: Option<usize>,
        mut filter_fn: Option<P>,
        mut map_fn: F,
    ) -> Result<Vec<U>, E>
    where
        F: FnMut(T) -> U,
        P: FnMut(&T) -> bool,
    {
        let mut result = Vec::new();

        for (count, item_result) in self.enumerate() {
            // Check limit first
            if let Some(max_count) = limit {
                if count >= max_count {
                    break;
                }
            }

            // Try to get the item, propagating error immediately
            let item = item_result?;

            // Check condition if provided
            if let Some(ref mut filter) = filter_fn {
                if !filter(&item) {
                    break;
                }
            }

            // Map and add to result
            result.push(map_fn(item));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_take_while_map_and_collect() {
        let data: Vec<Result<i32, &str>> = vec![Ok(1), Ok(2), Ok(3), Ok(8), Ok(4)];
        let result =
            data.into_iter()
                .try_take_while_map_and_collect(Some(4), Some(|&x: &i32| x < 8), |x| x * 2);
        assert_eq!(result, Ok(vec![2, 4, 6])); // stops at 8, so only gets 1,2,3 doubled
    }

    #[test]
    fn test_try_take_while_map_and_collect_with_error() {
        let data: Vec<Result<i32, &str>> = vec![Ok(1), Ok(2), Err("error"), Ok(8), Ok(4)];
        let result =
            data.into_iter()
                .try_take_while_map_and_collect(Some(4), Some(|&x: &i32| x < 8), |x| x * 2);
        assert_eq!(result, Err("error"));
    }
}
