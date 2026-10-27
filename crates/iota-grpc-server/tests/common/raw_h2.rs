// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! HTTP/2 frames read by hand, for tests that must see frames that a client
//! library does not show, and a connection that a client can write to before
//! it has read anything from the server. The frame layout is from RFC 9113
//! section 4.1.

use std::{
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};

/// How long a read waits for the server's next frame.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

pub const SETTINGS: u8 = 0x4;
pub const WINDOW_UPDATE: u8 = 0x8;

const ACK: u8 = 0x1;

pub const SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
pub const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;

/// One frame that the server sent.
#[derive(Debug)]
pub struct Frame {
    pub kind: u8,
    pub flags: u8,
    pub stream_id: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    /// For SETTINGS frames.
    pub fn is_ack(&self) -> bool {
        self.flags & ACK != 0
    }

    /// The value of `id` in a SETTINGS frame, if the frame has it.
    pub fn setting(&self, id: u16) -> Option<u32> {
        self.payload.as_chunks::<6>().0.iter().find_map(|entry| {
            (u16::from_be_bytes([entry[0], entry[1]]) == id)
                .then(|| u32::from_be_bytes([entry[2], entry[3], entry[4], entry[5]]))
        })
    }

    /// The increment of a WINDOW_UPDATE frame.
    pub fn window_increment(&self) -> u32 {
        u32::from_be_bytes(self.payload[..4].try_into().unwrap()) & 0x7fff_ffff
    }
}

/// One frame, as bytes on the wire.
pub fn frame(kind: u8, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len()).unwrap();
    let mut out = Vec::with_capacity(9 + payload.len());
    out.extend_from_slice(&len.to_be_bytes()[1..]);
    out.push(kind);
    out.push(flags);
    out.extend_from_slice(&stream_id.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// `message` with its gRPC header: no compression, and the length.
pub fn grpc_message(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + message.len());
    out.push(0);
    out.extend_from_slice(&u32::try_from(message.len()).unwrap().to_be_bytes());
    out.extend_from_slice(message);
    out
}

/// The first bytes that a client sends: the client preface and an empty
/// SETTINGS frame.
pub fn client_preface() -> Vec<u8> {
    let mut out = PREFACE.to_vec();
    out.extend(frame(SETTINGS, 0, 0, &[]));
    out
}

/// The reading half of a connection.
pub struct FrameReader {
    read: OwnedReadHalf,
}

impl FrameReader {
    /// The server's next frame, or `None` once the server has closed the
    /// connection. Fails if no frame comes within the read timeout.
    pub async fn next(&mut self) -> Option<Frame> {
        tokio::time::timeout(READ_TIMEOUT, async {
            let mut head = [0u8; 9];
            self.read.read_exact(&mut head).await.ok()?;
            let len = u32::from_be_bytes([0, head[0], head[1], head[2]]) as usize;
            let mut payload = vec![0u8; len];
            self.read.read_exact(&mut payload).await.ok()?;
            Some(Frame {
                kind: head[3],
                flags: head[4],
                stream_id: u32::from_be_bytes(head[5..9].try_into().unwrap()) & 0x7fff_ffff,
                payload,
            })
        })
        .await
        .expect("the server must send a frame or close the connection")
    }
}

/// A connection that has sent the client preface and empty SETTINGS.
pub struct Connection {
    pub reader: FrameReader,
    pub writer: OwnedWriteHalf,
}

impl Connection {
    pub async fn open(server: SocketAddr) -> Self {
        let (read, mut writer) = TcpStream::connect(server).await.unwrap().into_split();
        writer.write_all(&client_preface()).await.unwrap();
        Self {
            reader: FrameReader { read },
            writer,
        }
    }
}

/// A TCP connection that reads nothing until it is opened, so that a client
/// on it can send frames without having seen anything from the server.
pub struct GatedIo {
    inner: TcpStream,
    gate: Gate,
}

/// Opens a [`GatedIo`].
#[derive(Clone)]
pub struct Gate {
    open: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Gate {
    pub fn open(&self) {
        self.open.store(true, Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

impl GatedIo {
    pub async fn connect(server: SocketAddr) -> (Self, Gate) {
        let gate = Gate {
            open: Arc::default(),
            waker: Arc::default(),
        };
        let inner = TcpStream::connect(server).await.unwrap();
        (
            Self {
                inner,
                gate: gate.clone(),
            },
            gate,
        )
    }
}

impl AsyncRead for GatedIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if !this.gate.open.load(Ordering::SeqCst) {
            *this.gate.waker.lock().unwrap() = Some(cx.waker().clone());
            if !this.gate.open.load(Ordering::SeqCst) {
                return Poll::Pending;
            }
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for GatedIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
