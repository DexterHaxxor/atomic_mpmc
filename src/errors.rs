use std::{error::Error, fmt};

// for documentation
#[allow(unused_imports)]
use super::{Sender, Receiver};

/// Cause of a [`SendError`] or [`RecvError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCause {
    /// There are no more senders or receivers, and the operation would either discard data or block.
    HungUp,
    /// The channel is empty or full, and the operation would block.
    WouldBlock,
}

/// Error returned by [`Sender::send`].
/// 
/// It contains the cause of the error, as well as the data that was attempted to be sent.
pub struct SendError<T>(
    /// The data that was attempted to be sent.
    pub T,
    /// The cause of the error.
    pub ErrorCause
);

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SendError").field(&" ... ").finish()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SendError: the receiving end hung up")
    }
}

impl<T> Error for SendError<T> {}

/// Error returned by [`Receiver::recv`].
/// 
/// It contains the cause of the error.
#[derive(Debug, Clone, Copy)]
pub struct RecvError(
    /// The cause of the error.
    pub ErrorCause
);

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecvError: the sending end hung up")
    }
}

impl Error for RecvError {}