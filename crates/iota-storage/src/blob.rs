// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{Read, Write},
    marker::PhantomData,
};

use anyhow::{Result, anyhow, bail};
use byteorder::ReadBytesExt;
use integer_encoding::{VarInt, VarIntReader};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::{Serialize, de::DeserializeOwned};

pub const MAX_VARINT_LENGTH: usize = 10;
pub const BLOB_ENCODING_BYTES: usize = 1;
const MAX_PREALLOCATION: usize = 1 << 20;
/// The largest blob [`Blob::read`] accepts. The length comes from the input,
/// and a small zstd-compressed file can decompress to any length.
const MAX_BLOB_LENGTH: u64 = 1 << 30;

#[derive(Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum BlobEncoding {
    Bcs = 1,
}

pub struct Blob {
    pub data: Vec<u8>,
    pub encoding: BlobEncoding,
}

impl Blob {
    pub fn encode<T: Serialize>(value: &T, encoding: BlobEncoding) -> Result<Self> {
        let value_buf = bcs::to_bytes(value)?;
        let (data, encoding) = match encoding {
            BlobEncoding::Bcs => (value_buf, encoding),
        };
        Ok(Blob { data, encoding })
    }
    pub fn decode<T: DeserializeOwned>(self) -> Result<T> {
        let data = match &self.encoding {
            BlobEncoding::Bcs => self.data,
        };
        let res = bcs::from_bytes(&data)?;
        Ok(res)
    }
    /// Reads one blob from `rbuf`.
    ///
    /// Memory grows with the bytes actually read, not with the length the
    /// blob declares: at most 1 MiB is reserved before reading, and at most
    /// 1 GiB is read.
    ///
    /// # Errors
    ///
    /// Fails if reading `rbuf` fails, if the declared length is 0 or above
    /// 1 GiB, if `rbuf` ends before that many data bytes, or if the encoding
    /// is unknown.
    pub fn read<R: Read>(rbuf: &mut R) -> Result<Blob> {
        let len = rbuf.read_varint::<u64>()?;
        if len == 0 {
            bail!("Invalid object length of 0 in file");
        }
        if len > MAX_BLOB_LENGTH {
            bail!("blob length {len} is above the maximum of {MAX_BLOB_LENGTH} bytes");
        }
        let encoding = rbuf.read_u8()?;
        // The length comes from the input, so only part of it is reserved up
        // front.
        let mut data = Vec::with_capacity(len.min(MAX_PREALLOCATION as u64) as usize);
        rbuf.take(len).read_to_end(&mut data)?;
        if data.len() as u64 != len {
            bail!(
                "blob length {len} is beyond the remaining input of {} bytes",
                data.len()
            );
        }
        let blob = Blob {
            data,
            encoding: BlobEncoding::try_from(encoding)?,
        };
        Ok(blob)
    }
    pub fn write<W: Write>(&self, wbuf: &mut W) -> Result<usize> {
        let mut buf = [0u8; MAX_VARINT_LENGTH];
        let mut counter = 0;
        let n = (self.data.len() as u64).encode_var(&mut buf);
        wbuf.write_all(&buf[0..n])?;
        counter += n;
        buf[0] = self.encoding.into();
        wbuf.write_all(&buf[0..BLOB_ENCODING_BYTES])?;
        counter += 1;
        wbuf.write_all(&self.data)?;
        counter += self.data.len();
        Ok(counter)
    }
    pub fn size(&self) -> usize {
        let mut blob_size = self.data.len().required_space();
        blob_size += BLOB_ENCODING_BYTES;
        blob_size += self.data.len();
        blob_size
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        [vec![self.encoding.into()], self.data.clone()].concat()
    }
    pub fn from_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
        let (encoding, data) = bytes.split_first().ok_or(anyhow!("empty bytes"))?;
        Blob {
            data: data.to_vec(),
            encoding: BlobEncoding::try_from(*encoding)?,
        }
        .decode()
    }
}

/// An iterator over blobs in a blob file.
pub struct BlobIter<T> {
    reader: Box<dyn Read>,
    _phantom: PhantomData<T>,
}

impl<T: DeserializeOwned> BlobIter<T> {
    pub fn new(reader: Box<dyn Read>) -> Self {
        Self {
            reader,
            _phantom: PhantomData,
        }
    }
    fn next_blob(&mut self) -> Result<T> {
        let blob = Blob::read(&mut self.reader)?;
        blob.decode()
    }
}

impl<T: DeserializeOwned> Iterator for BlobIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_blob().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::{FileCompression, StorageFormat, make_iterator};

    const MAGIC: u32 = 0x0000BEEF;

    /// Encodes a blob header: the length varint followed by the BCS encoding
    /// byte.
    fn header(len: u64) -> Vec<u8> {
        let mut bytes = len.encode_var_vec();
        bytes.push(BlobEncoding::Bcs.into());
        bytes
    }

    #[test]
    fn write_read() {
        // The second value is larger than the space reserved before reading.
        for value in [vec![1u8, 2, 3], vec![7u8; 3 * MAX_PREALLOCATION]] {
            let blob = Blob::encode(&value, BlobEncoding::Bcs).unwrap();
            let mut bytes = Vec::new();
            blob.write(&mut bytes).unwrap();
            let read = Blob::read(&mut Cursor::new(bytes)).unwrap();
            assert_eq!(read.data, blob.data);
            assert_eq!(read.decode::<Vec<u8>>().unwrap(), value);
        }
    }

    #[test]
    fn rejects_length_beyond_input() {
        for len in [MAX_BLOB_LENGTH, 4] {
            let mut bytes = header(len);
            bytes.extend_from_slice(&[0u8; 3]);
            let err = Blob::read(&mut Cursor::new(bytes)).err().unwrap();
            assert!(
                err.to_string()
                    .contains("is beyond the remaining input of 3 bytes"),
                "{err}"
            );
        }
    }

    /// A blob longer than the maximum is refused, also from a zstd-compressed
    /// file.
    #[test]
    fn rejects_length_above_maximum() {
        for compression in [FileCompression::None, FileCompression::Zstd] {
            for len in [MAX_BLOB_LENGTH + 1, u64::MAX] {
                let mut blob = header(len);
                blob.extend_from_slice(&[0u8; 3]);
                let mut file = MAGIC.to_be_bytes().to_vec();
                file.push(StorageFormat::Blob.into());
                file.push(compression.into());
                match compression {
                    FileCompression::None => file.extend(blob),
                    FileCompression::Zstd => {
                        FileCompression::zstd_compress(&mut Cursor::new(blob), &mut file).unwrap()
                    }
                }

                let (mut reader, _) = crate::read(MAGIC, Cursor::new(file)).unwrap();
                let err = Blob::read(&mut reader).err().unwrap();
                assert!(
                    err.to_string().contains(&format!(
                        "blob length {len} is above the maximum of {MAX_BLOB_LENGTH} bytes"
                    )),
                    "{err}"
                );
            }
        }
    }

    #[test]
    fn blob_iter_stops_at_length_beyond_input() {
        for compression in [FileCompression::None, FileCompression::Zstd] {
            let mut blobs = Vec::new();
            Blob::encode(&7u64, BlobEncoding::Bcs)
                .unwrap()
                .write(&mut blobs)
                .unwrap();
            blobs.extend(header(4));
            blobs.extend_from_slice(&[0u8; 3]);

            let mut file = MAGIC.to_be_bytes().to_vec();
            file.push(StorageFormat::Blob.into());
            file.push(compression.into());
            match compression {
                FileCompression::None => file.extend(blobs),
                FileCompression::Zstd => {
                    FileCompression::zstd_compress(&mut Cursor::new(blobs), &mut file).unwrap()
                }
            }

            let values: Vec<u64> = make_iterator(MAGIC, Cursor::new(file)).unwrap().collect();
            assert_eq!(values, vec![7]);
        }
    }
}
