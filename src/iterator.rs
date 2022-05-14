use sealed::sealed;
use std::iter::FusedIterator;

use crate::{Receiver, RecvError};

/// A helper trait for implementing [`Iter`].
#[sealed]
#[doc(hidden)]
pub trait Recv {
    type Item;
    fn recv(&self) -> Result<Self::Item, RecvError>;
    fn try_recv(&self) -> Result<Self::Item, RecvError>;
}

#[sealed]
impl<T> Recv for Receiver<T> {
    type Item = T;
    fn recv(&self) -> Result<Self::Item, RecvError> {
        self.recv()
    }

    fn try_recv(&self) -> Result<Self::Item, RecvError> {
        self.try_recv()
    }
}

#[sealed]
impl<T> Recv for &Receiver<T> {
    type Item = T;
    fn recv(&self) -> Result<Self::Item, RecvError> {
        (**self).recv()
    }

    fn try_recv(&self) -> Result<Self::Item, RecvError> {
        (**self).try_recv()
    }
}

/// Iterator over the values of a receiver.
/// The iterator will return `None` when the channel is hung up.
#[derive(Debug)]
pub struct Iter<R>(Option<R>);

impl<R> Iter<R> {
    pub(super) fn new(receiver: R) -> Self {
        Self(Some(receiver))
    }
}

impl<R: Recv> Iterator for Iter<R> {
    type Item = R::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.as_ref().map(|r| r.recv()) {
            Some(Ok(v)) => Some(v),
            Some(Err(_)) => {
                self.0 = None;
                None
            }
            None => None,
        }
    }
}

impl<R: Recv> FusedIterator for Iter<R> {}

/// An iterator over the pending values of a channel. This iterator will
/// return `None` when the channel is hung up or the channel is empty.
#[derive(Debug)]
pub struct TryIter<R>(Option<R>);

impl<R> TryIter<R> {
    pub(super) fn new(receiver: R) -> Self {
        Self(Some(receiver))
    }
}

impl<R: Recv> Iterator for TryIter<R> {
    type Item = R::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.as_ref().map(|r| r.try_recv()) {
            Some(Ok(v)) => Some(v),
            Some(Err(_)) => {
                self.0 = None;
                None
            }
            None => None,
        }
    }
}
