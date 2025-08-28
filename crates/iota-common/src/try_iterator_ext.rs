// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub trait TryIteratorExt<T, E>: Iterator<Item = Result<T, E>> + Sized {
    /// Tries to map and collect items from a fallible iterator, stopping early
    /// on the first error.
    fn try_map_while_and_collect<U, P, B>(self, mut predicate: P) -> B
    where
        P: FnMut(T) -> Option<U>,
        B: FromIterator<Result<U, E>>,
    {
        FromIterator::from_iter(self.map_while(|result| result.map(&mut predicate).transpose()))
    }

    /// Try taking at most limit items while predicate holds collecting mapped
    /// values and breaking early on errors.
    fn try_take_map_while_and_collect<U, F, P, B>(
        self,
        limit: Option<usize>,
        mut predicate: P,
        map_fn: F,
    ) -> Result<B, E>
    where
        F: Fn(T) -> U,
        P: FnMut(&T) -> bool,
        B: FromIterator<U>,
    {
        let predicate = |v| predicate(&v).then(|| map_fn(v));

        if let Some(limit) = limit {
            self.take(limit).try_map_while_and_collect(predicate)
        } else {
            self.try_map_while_and_collect(predicate)
        }
    }
}

impl<I, T, E> TryIteratorExt<T, E> for I where I: Iterator<Item = Result<T, E>> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_skip_filter_map_and_collect() {
        let result: Result<Vec<_>, &str> = [1, 2, 3, 8]
            .into_iter()
            .map(Ok)
            .chain(std::iter::from_fn(|| panic!()))
            .try_take_map_while_and_collect(Some(5), |&x: &i32| x < 8, |x| x * 2);
        assert_eq!(result, Ok(vec![2, 4, 6])); // stops at 8
    }

    #[test]
    fn test_try_skip_filter_map_and_collect_with_error() {
        let result: Result<Vec<_>, _> = [Ok(1), Ok(2), Err("error")]
            .into_iter()
            .chain(std::iter::from_fn(|| panic!()))
            .try_take_map_while_and_collect(None, |&x: &i32| x < 8, |x| x * 2);
        assert_eq!(result, Err("error")); // stops on the first error
    }

    #[test]
    fn test_try_skip_filter_map_and_collect_with_limit() {
        let result: Result<Vec<_>, &str> = [Ok(1), Ok(2)]
            .into_iter()
            .chain(std::iter::from_fn(|| panic!()))
            .try_take_map_while_and_collect(Some(2), |&x: &i32| x < 8, |x| x * 2);
        assert_eq!(result, Ok(vec![2, 4])); // respects limit stopping before panic
    }
}
