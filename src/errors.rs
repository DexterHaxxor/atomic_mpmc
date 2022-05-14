use std::{error::Error, fmt};

// for documentation
#[allow(unused_imports)]
use super::{Receiver, Sender};

/// Cause of a [`SendError`] or [`RecvError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCause {
    /// There are no more senders or receivers, and the operation would either discard data or block.
    HungUp,
    /// The channel is empty or full, and the operation would block.
    WouldBlock,
}

impl fmt::Display for ErrorCause {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorCause::HungUp => write!(f, "channel hung up"),
            ErrorCause::WouldBlock => write!(f, "channel would block"),
        }
    }
}

/// Error returned by [`Sender::send`].
///
/// It contains the cause of the error, as well as the data that was attempted to be sent.
pub struct SendError<T>(
    /// The data that was attempted to be sent.
    pub T,
    /// The cause of the error.
    pub ErrorCause,
);

impl<T> SendError<T> {
    /// Returns the data that was attempted to be sent.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(1);
    ///
    /// // drop the receiver to cause a send error
    /// std::mem::drop(receiver);
    ///
    /// assert_eq!(sender.send(1).unwrap_err().into_inner(), 1);
    /// ```
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SendError").field(&self.1).finish()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SendError: {}", self.1)
    }
}

impl<T> Error for SendError<T> {}

/// Error returned by [`Receiver::recv`].
///
/// It contains the cause of the error.
#[derive(Debug, Clone, Copy)]
pub struct RecvError(
    /// The cause of the error.
    pub ErrorCause,
);

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecvError: {}", self.0)
    }
}

impl Error for RecvError {}
