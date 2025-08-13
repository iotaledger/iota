// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::Error;

pub trait ConvertTo<T>: Send + Sized {
    fn convert(self) -> Result<T, Error>;

    fn convert_unchecked(self) -> T {
        self.convert().unwrap()
    }
}

impl<T: Send + Sized> ConvertTo<T> for T {
    fn convert(self) -> Result<T, Error> {
        Ok(self)
    }

    fn convert_unchecked(self) -> T {
        self
    }
}

impl<T: Copy + Send + Sync + Sized> ConvertTo<T> for &T {
    fn convert(self) -> Result<T, Error> {
        Ok(*self)
    }

    fn convert_unchecked(self) -> T {
        *self
    }
}
