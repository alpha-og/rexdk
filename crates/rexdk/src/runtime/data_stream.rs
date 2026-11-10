//! The stream of binary data from a Rex data connection.
//!
//! The stream represents the continuous output of a block's data source.
//! WebSocket message boundaries are not preserved: each item contains an
//! arbitrary chunk of the byte stream.
//!
//! The underlying data connection is managed independently by the worker task.
//! Unexpected connection closures are handled by resuming the stream when
//! possible. The stream ends when the data source ends, the client is dropped,
//! or the worker encounters an unrecoverable error.
//!
//! Dropping the stream asks the worker to close the connection gracefully.
//! Use [`DataStream::close`] when the close must finish before continuing.

use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use bytes::Bytes;
use futures_util::Stream;
use tokio::sync::{mpsc::Receiver, watch};

use crate::Error;

/// A stream of binary data received from a Rex data connection.
///
/// Data is delivered in chunks. Chunk boundaries have no protocol-level
/// meaning; consumers should treat the chunks as parts of one continuous
/// byte stream.
///
/// The stream applies backpressure to the data connection worker. Consumers
/// should read data promptly to avoid accumulating buffered data.
///
/// Bytes are counted as consumed when they are yielded to the consumer. That
/// count is informational; resumption uses the worker's count of bytes
/// accepted for delivery, which can be ahead by the buffered amount.
#[derive(Debug)]
pub struct DataStream {
    rx: Receiver<Result<Bytes, Error>>,
    consumed_bytes: Arc<AtomicU64>,
    received_bytes: Arc<AtomicU64>,
    channel_nonce: Arc<AtomicU64>,
    resume_timeout: Duration,
    shutdown: watch::Sender<bool>,
}

impl DataStream {
    pub(crate) fn new(
        rx: Receiver<Result<Bytes, Error>>,
        consumed_bytes: Arc<AtomicU64>,
        received_bytes: Arc<AtomicU64>,
        channel_nonce: Arc<AtomicU64>,
        resume_timeout: Duration,
        shutdown: watch::Sender<bool>,
    ) -> Self {
        Self {
            rx,
            consumed_bytes,
            received_bytes,
            channel_nonce,
            resume_timeout,
            shutdown,
        }
    }

    /// Receives the next chunk of data.
    ///
    /// Returns `None` when the data stream ends or the client is dropped.
    /// Returns an error if the data connection encounters an unrecoverable
    /// failure.
    pub async fn recv(&mut self) -> Option<Result<Bytes, Error>> {
        let item = self.rx.recv().await;
        self.account_consumed(&item);
        item
    }

    /// Closes the data connection with a normal WebSocket close frame.
    ///
    /// Consumes the stream and waits until the worker has finished. Dropping
    /// the stream instead asks for the same close without waiting for it.
    pub async fn close(mut self) {
        let _ = self.shutdown.send_replace(true);
        while self.rx.recv().await.is_some() {}
    }

    /// Returns the number of stream bytes delivered to the consumer.
    ///
    /// This value is maintained across data connection resumptions and
    /// excludes handshake messages and data buffered but not yet delivered.
    pub fn consumed_bytes(&self) -> u64 {
        self.consumed_bytes.load(Ordering::Relaxed)
    }

    /// Returns the number of stream bytes the worker has accepted for
    /// delivery, including bytes still buffered for the consumer.
    ///
    /// This is the offset sent in a RESUME.
    pub fn received_bytes(&self) -> u64 {
        self.received_bytes.load(Ordering::Relaxed)
    }

    /// The channel nonce from the latest WELCOME.
    ///
    /// Rex uses it to mark its own terminal control sequences in the stream.
    pub fn channel_nonce(&self) -> u64 {
        self.channel_nonce.load(Ordering::Relaxed)
    }

    /// How long the server retains the stream for a resume.
    pub fn resume_timeout(&self) -> Duration {
        self.resume_timeout
    }

    fn account_consumed(&self, item: &Option<Result<Bytes, Error>>) {
        if let Some(Ok(bytes)) = item {
            self.consumed_bytes
                .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        }
    }
}

impl Drop for DataStream {
    fn drop(&mut self) {
        // The worker closes gracefully; Drop cannot wait for it.
        let _ = self.shutdown.send_replace(true);
    }
}

impl Stream for DataStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match this.rx.poll_recv(cx) {
            Poll::Ready(item) => {
                this.account_consumed(&item);
                Poll::Ready(item)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
