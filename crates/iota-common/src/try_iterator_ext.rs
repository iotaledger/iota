// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub trait TryIteratorExt<T, E>: Iterator<Item = Result<T, E>> + Sized {
    /// Tries to collect items from an iterator, stopping early on first error,
    /// with an optional limit and optional take_while filter_fn.
    /// This avoids creating an intermediate Vec when we just need to propagate
    /// errors.
    fn try_map_while_and_collect<U, F, P, B>(self, mut predicate: P, map_fn: F) -> B
    where
        F: Fn(T) -> U,
        P: FnMut(&T) -> bool,
        B: FromIterator<Result<U, E>>,
    {
        FromIterator::from_iter(self.map_while(|result| -> Option<Result<U, E>> {
            match result {
                Ok(v) => {
                    if predicate(&v) {
                        Some(Ok(map_fn(v)))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            }
        }))
    }

    fn try_skip_filter_map_and_collect<U, F, P, B>(
        self,
        mut limit: Option<usize>,
        mut predicate: Option<P>,
        map_fn: F,
    ) -> B
    where
        F: Fn(T) -> U,
        P: FnMut(&T) -> bool,
        B: FromIterator<Result<U, E>>,
    {
        self.try_map_while_and_collect(
            |v| {
                let within_limit = if let Some(ref mut limited) = limit {
                    if *limited == 0 {
                        false
                    } else {
                        *limited -= 1;
                        true
                    }
                } else {
                    true
                };

                let pass_filter = if let Some(ref mut filter) = predicate {
                    filter(v)
                } else {
                    true
                };
                within_limit && pass_filter
            },
            map_fn,
        )
    }
}

impl<I, T, E> TryIteratorExt<T, E> for I where I: Iterator<Item = Result<T, E>> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_skip_filter_map_and_collect() {
        let data: Vec<Result<i32, &str>> = vec![Ok(1), Ok(2), Ok(3), Ok(8), Ok(4)];
        let result: Vec<_> = data.into_iter().try_skip_filter_map_and_collect(
            Some(4),
            Some(|&x: &i32| x < 8),
            |x| x * 2,
        );
        assert_eq!(result, vec![Ok(2), Ok(4), Ok(6)]); // stops at 8, so only gets 1,2,3 doubled
    }

    #[test]
    fn test_try_skip_filter_map_and_collect_with_error() {
        let data: Vec<Result<i32, &str>> = vec![Ok(1), Ok(2), Err("error"), Ok(8), Ok(4)];
        let result: Result<Vec<i32>, &str> = data.into_iter().try_skip_filter_map_and_collect(
            Some(4),
            Some(|&x: &i32| x < 8),
            |x| x * 2,
        );
        assert_eq!(result, Err("error"));
    }
}
