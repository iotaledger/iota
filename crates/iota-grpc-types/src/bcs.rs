use serde::{Deserialize, Serialize};

use crate::v0::common::BcsData;

impl BcsData {
    pub fn serialize_from<T>(data: &T) -> Result<Self, bcs::Error>
    where
        T: Serialize,
    {
        let serialized = bcs::to_bytes(data)?;
        Ok(BcsData { data: serialized })
    }

    pub fn deserialize_into<T>(&self) -> Result<T, bcs::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        bcs::from_bytes(&self.data)
    }
}
