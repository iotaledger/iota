// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A prost codec that refuses a message carrying more entries than its type
//! allows, before decoding the entries past that count.
//!
//! Protobuf writes no count ahead of a repeated field: each entry is a field of
//! its own on the wire. An empty `bytes` entry is two bytes there and a 32-byte
//! `Bytes` once decoded, so the message size limit alone lets a peer make the
//! decoder build 16 times the message in `Bytes`.

use std::marker::PhantomData;

use bytes::{Buf, BufMut};
use prost::{
    DecodeError, Message,
    encoding::{DecodeContext, WireType},
};
use tonic::{
    Status,
    codec::{BufferSettings, Codec, DecodeBuf, Decoder},
};
use tonic_prost::{ProstCodec, ProstEncoder};

/// A message whose decoding refuses more than `MAX_ENTRIES` entries.
pub(crate) trait EntryLimit {
    /// Maximum number of entries one encoded message may contain. Each element
    /// of a `Vec<Bytes>` field counts as one entry, and a whole `Vec<u32>` field
    /// counts as one, because protobuf encodes the numbers together.
    const MAX_ENTRIES: usize;
    /// Message of the status the decoder returns when it refuses a message of
    /// this type: `TOO_MANY_ENTRIES_IN_REQUEST` or
    /// `TOO_MANY_ENTRIES_IN_RESPONSE`.
    const REFUSAL: &'static str;
}

/// Refusal message for a request carrying more entries than its type allows.
pub(crate) const TOO_MANY_ENTRIES_IN_REQUEST: &str =
    "request carries more entries than its type allows";

/// Refusal message for a response carrying more entries than its type allows.
pub(crate) const TOO_MANY_ENTRIES_IN_RESPONSE: &str =
    "response carries more entries than its type allows";

/// `MAX_ENTRIES` of a request whose fields are all numbers. Decoding keeps at
/// most one `u32` per byte of the request, so at the default 1 MiB request size
/// limit it builds at most 4 MiB.
pub(crate) const NO_ENTRY_LIMIT: usize = usize::MAX;

/// A prost codec whose decoder refuses a message carrying more entries than
/// its type allows.
#[derive(Debug, Clone)]
pub(crate) struct BoundedProstCodec<T, U> {
    _pd: PhantomData<(T, U)>,
}

impl<T, U> Default for BoundedProstCodec<T, U> {
    fn default() -> Self {
        Self { _pd: PhantomData }
    }
}

impl<T, U> Codec for BoundedProstCodec<T, U>
where
    T: Message + Send + 'static,
    U: Message + Default + EntryLimit + Send + 'static,
{
    type Encode = T;
    type Decode = U;

    type Encoder = ProstEncoder<T>;
    type Decoder = BoundedProstDecoder<U>;

    fn encoder(&mut self) -> Self::Encoder {
        ProstCodec::<T, U>::raw_encoder(BufferSettings::default())
    }

    fn decoder(&mut self) -> Self::Decoder {
        BoundedProstDecoder { _pd: PhantomData }
    }
}

/// Decodes `U`, refusing a message carrying more than `U::MAX_ENTRIES`
/// entries.
#[derive(Debug)]
pub(crate) struct BoundedProstDecoder<U> {
    _pd: PhantomData<U>,
}

impl<U: Message + Default + EntryLimit> Decoder for BoundedProstDecoder<U> {
    type Item = U;
    type Error = Status;

    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<U>, Status> {
        decode_bounded(buf).map(Some)
    }
}

fn decode_bounded<U: Message + Default + EntryLimit>(buf: impl Buf) -> Result<U, Status> {
    let mut bounded = Bounded::<U>::default();
    // Parse errors map to INTERNAL, as in tonic-prost.
    bounded
        .merge(buf)
        .map_err(|e| Status::internal(e.to_string()))?;
    if bounded.entries > U::MAX_ENTRIES {
        // OUT_OF_RANGE, as tonic answers a message over its size limit.
        return Err(Status::out_of_range(U::REFUSAL));
    }
    Ok(bounded.message)
}

/// `M` decoded up to its first `M::MAX_ENTRIES` entries; an entry past them
/// drops the rest of the message unread.
#[derive(Debug, Default)]
struct Bounded<M> {
    message: M,
    entries: usize,
}

impl<M: Message + EntryLimit> Message for Bounded<M> {
    fn encode_raw(&self, buf: &mut impl BufMut) {
        self.message.encode_raw(buf);
    }

    fn merge_field(
        &mut self,
        tag: u32,
        wire_type: WireType,
        buf: &mut impl Buf,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        self.entries += 1;
        if self.entries > M::MAX_ENTRIES {
            buf.advance(buf.remaining());
            return Ok(());
        }
        self.message.merge_field(tag, wire_type, buf, ctx)
    }

    fn encoded_len(&self) -> usize {
        self.message.encoded_len()
    }

    fn clear(&mut self) {
        self.message.clear();
        self.entries = 0;
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[derive(Clone, PartialEq, prost::Message)]
    struct Entries {
        #[prost(bytes = "bytes", repeated, tag = "1")]
        entries: Vec<Bytes>,
    }

    impl EntryLimit for Entries {
        const MAX_ENTRIES: usize = 4;
        const REFUSAL: &'static str = TOO_MANY_ENTRIES_IN_RESPONSE;
    }

    fn encoded(entries: Vec<Bytes>) -> Bytes {
        Entries { entries }.encode_to_vec().into()
    }

    #[test]
    fn a_message_at_the_ceiling_decodes_whole() {
        let entries: Vec<_> = (0..4u8).map(|i| Bytes::from(vec![i; 3])).collect();

        let decoded: Entries = decode_bounded(encoded(entries.clone())).unwrap();

        assert_eq!(decoded.entries, entries);
    }

    #[test]
    fn a_message_past_the_ceiling_is_refused() {
        let status = decode_bounded::<Entries>(encoded(vec![Bytes::new(); 5])).unwrap_err();

        assert_eq!(status.code(), tonic::Code::OutOfRange);
        assert_eq!(status.message(), TOO_MANY_ENTRIES_IN_RESPONSE);
    }

    /// However many entries a message carries, decoding builds at most the
    /// ceiling's worth and reads no further.
    #[test]
    fn decoding_stops_at_the_ceiling() {
        let mut buf = encoded(vec![Bytes::new(); 1 << 16]);
        let mut bounded = Bounded::<Entries>::default();

        bounded.merge(&mut buf).unwrap();

        assert_eq!(bounded.message.entries.len(), 4);
        assert_eq!(bounded.entries, 5);
        assert!(!buf.has_remaining());
    }

    /// A malformed message is a parse error, not a refusal.
    #[test]
    fn a_truncated_entry_is_not_a_refusal() {
        // Field 1 declares five bytes and carries none.
        let status = decode_bounded::<Entries>(Bytes::from_static(&[0x0a, 0x05])).unwrap_err();

        assert_eq!(status.code(), tonic::Code::Internal);
        assert_ne!(status.message(), TOO_MANY_ENTRIES_IN_RESPONSE);
    }
}
