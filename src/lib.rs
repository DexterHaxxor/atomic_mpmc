//! Fast, atomic, multi-producer, multi-consumer queue.
//!
//! This is a queue which supports multiple producers and consumers,
//! even from different threads. It works as a FIFO queue, in a circular
//! buffer. The [`Sender`] and [`Receiver`] types are used to send and
//! receive values, and they implement [`Send`], [`Sync`], and [`Clone`].
//!
//! The [`channel`] function is used to create a channel.

#![warn(missing_docs)]

use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

mod waiter;
use waiter::Waiter;

mod errors;
pub use errors::{ErrorCause, RecvError, SendError};

mod iterator;
pub use iterator::{Iter, TryIter};

#[cfg(test)]
mod tests;

struct Node<T> {
    data: MaybeUninit<UnsafeCell<T>>,

    /// Whether data is initialized.
    /// Stupid name, but I'm not changing it.
    hot: AtomicBool,
}

impl<T> Default for Node<T> {
    fn default() -> Self {
        Node {
            data: MaybeUninit::uninit(),
            hot: Default::default(),
        }
    }
}

impl<T> Node<T> {
    #[inline(always)]
    fn hot(&self) -> bool {
        self.hot.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn set_hot(&self, hot: bool) {
        self.hot.store(hot, Ordering::Relaxed);
    }

    #[inline(always)]
    fn data(&self) -> *mut T {
        UnsafeCell::raw_get(self.data.as_ptr())
    }
}

impl<T> core::fmt::Debug for Node<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Node").field("hot", &self.hot()).finish()
    }
}

impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: This is safe because hot is only ever set to true
            // after the data is initialized.
            if self.hot() {
                ptr::drop_in_place(self.data.as_mut_ptr());
            }
        }
    }
}

#[derive(Debug)]
struct Channel<T> {
    data: Vec<Node<T>>,

    write: AtomicUsize,
    read: AtomicUsize,

    receivers: AtomicUsize,
    senders: AtomicUsize,

    writable: Waiter,
    readable: Waiter,
}

impl<T> Channel<T> {
    // The members of this struct should all get inlined into the public API.

    #[inline(always)]
    fn new(capacity: usize) -> Self {
        let mut data = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            data.push(Default::default());
        }

        Self {
            data,

            write: Default::default(),
            read: Default::default(),

            receivers: Default::default(),
            senders: Default::default(),

            writable: Waiter::new(true),
            readable: Waiter::new(false),
        }
    }

    #[inline(always)]
    fn check_senders(&self) -> Result<(), RecvError> {
        if self.senders.load(Ordering::Relaxed) == 0 {
            Err(RecvError(ErrorCause::HungUp))
        } else {
            Ok(())
        }
    }

    #[inline(always)]
    fn check_receivers(&self, value: T) -> Result<T, SendError<T>> {
        if self.receivers.load(Ordering::Relaxed) == 0 {
            Err(SendError(value, ErrorCause::HungUp))
        } else {
            Ok(value)
        }
    }

    #[inline(always)]
    fn get_node<'a>(&'a self, from: &AtomicUsize) -> &'a Node<T> {
        unsafe {
            // SAFETY: The index is always in bounds, because of the modulo.
            self.data
                .get_unchecked(from.fetch_add(1, Ordering::Relaxed) % self.data.len())
        }
    }

    #[inline(always)]
    fn try_node<'a>(&'a self, from: &AtomicUsize) -> (&'a Node<T>, usize) {
        let index = from.load(Ordering::Relaxed);

        (
            unsafe {
                // SAFETY: The index is always in bounds, because of the modulo.
                self.data.get_unchecked(index % self.data.len())
            },
            index,
        )
    }

    #[inline(always)]
    fn write(&self, value: T) -> Result<(), SendError<T>> {
        let value = self.check_receivers(value)?;
        self.writable.wait();

        let node = self.get_node(&self.write);

        if node.hot() {
            self.writable.reset();
            self.writable.wait();
        }

        unsafe {
            // SAFETY: The node is not hot, so it is safe to write to it.
            ptr::write(node.data(), value);
        }

        node.set_hot(true);
        self.readable.set();

        Ok(())
    }

    #[inline(always)]
    fn try_write(&self, value: T) -> Result<(), SendError<T>> {
        let value = self.check_receivers(value)?;
        loop {
            let node = self.try_node(&self.write);

            if node.0.hot() {
                self.writable.reset();
                // Return error when the channel is full
                return Err(SendError(value, ErrorCause::WouldBlock));
            }

            if let Err(_) = self.write.compare_exchange(
                node.1,
                node.1 + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                // A thread stole the node, try again...
                continue;
            }

            unsafe {
                // SAFETY: The node is not hot, so it is safe to write to it.
                ptr::write(node.0.data(), value);
            }

            node.0.set_hot(true);
            self.readable.set();

            return Ok(());
        }
    }

    #[inline(always)]
    fn read(&self) -> Result<T, RecvError> {
        self.readable.wait();

        let node = self.get_node(&self.read);

        if !node.hot() {
            self.check_senders()?;
            self.readable.reset();
            self.readable.wait();
        }

        let value = unsafe {
            // SAFETY: The node is hot, so it is safe to read from it.
            ptr::read(node.data())
        };
        node.set_hot(false);
        self.writable.set();

        Ok(value)
    }

    #[inline(always)]
    fn try_read(&self) -> Result<T, RecvError> {
        loop {
            let node = self.try_node(&self.read);

            if !node.0.hot() {
                self.check_senders()?;
                self.readable.reset();
                // Return error when the channel is empty
                return Err(RecvError(ErrorCause::WouldBlock));
            }

            if let Err(_) =
                self.read
                    .compare_exchange(node.1, node.1 + 1, Ordering::Relaxed, Ordering::Relaxed)
            {
                // A thread stole the node, try again...
                continue;
            }

            let value = unsafe {
                // SAFETY: The node is hot, so it is safe to read from it.
                ptr::read(node.0.data())
            };
            node.0.set_hot(false);
            self.writable.set();

            return Ok(value);
        }
    }
}

unsafe impl<T: Send> Send for Channel<T> {}
unsafe impl<T: Send> Sync for Channel<T> {}

/// A sender for a MPMC channel.
///
/// This struct is created by the [`channel`] function. It provides methods for
/// sending data to the channel.
#[derive(Debug)]
pub struct Sender<T>(Arc<Channel<T>>);

impl<T> Sender<T> {
    fn new(channel: Arc<Channel<T>>) -> Self {
        channel.senders.fetch_add(1, Ordering::Relaxed);
        Self(channel)
    }

    /// Send a value to the channel. This function will block the current thread
    /// if the channel is full.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(1);
    ///
    /// sender.send(1).unwrap();
    /// ```
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.0.write(value)
    }

    /// Attempt to send a value to the channel. This function will return
    /// `Err(SendError(value, ErrorCause::WouldBlock))` if the channel is full.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(1);
    ///
    /// sender.send(1).unwrap();
    /// sender.try_send(2).unwrap_err();
    /// ```
    pub fn try_send(&self, value: T) -> Result<(), SendError<T>> {
        self.0.try_write(value)
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.0.senders.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

/// A receiver for a MPMC channel.
///
/// This struct is created by the [`channel`] function. It provides methods for
/// receiving data from the channel.
///
/// This struct implements the [`IntoIterator`] trait, which means that you can
/// convert it to an iterator over received values.
#[derive(Debug)]
pub struct Receiver<T>(Arc<Channel<T>>);

impl<T> Receiver<T> {
    fn new(channel: Arc<Channel<T>>) -> Receiver<T> {
        channel.receivers.fetch_add(1, Ordering::Relaxed);
        Receiver(channel)
    }

    /// Receive a value from the channel. This function will block the current
    /// thread if the channel is empty.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(1);
    ///
    /// sender.send(1).unwrap();
    /// assert_eq!(receiver.recv().unwrap(), 1);
    /// ```
    pub fn recv(&self) -> Result<T, RecvError> {
        self.0.read()
    }

    /// Attempt to receive a value from the channel. This function will return
    /// `Err(RecvError(ErrorCause::WouldBlock))` if the channel is empty.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(1);
    ///
    /// sender.send(1).unwrap();
    /// assert_eq!(receiver.try_recv().unwrap(), 1);
    /// assert!(receiver.try_recv().is_err());
    /// ```
    pub fn try_recv(&self) -> Result<T, RecvError> {
        self.0.try_read()
    }

    /// Creates an iterator over the values of this channel.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(2);
    ///
    /// sender.send(1).unwrap();
    /// sender.send(2).unwrap();
    ///
    /// let mut iter = receiver.iter();
    /// assert_eq!(iter.next().unwrap(), 1);
    /// assert_eq!(iter.next().unwrap(), 2);
    /// ```
    pub fn iter(&self) -> Iter<&Self> {
        Iter::new(self)
    }

    /// Creates a new iterator over the pending values of this channel.
    ///
    /// # Examples
    /// ```
    /// use atomic_mpmc::channel;
    ///
    /// let (sender, receiver) = channel::<i32>(2);
    ///
    /// sender.send(1).unwrap();
    /// sender.send(2).unwrap();
    ///
    /// let mut iter = receiver.try_iter();
    /// assert_eq!(iter.next().unwrap(), 1);
    /// assert_eq!(iter.next().unwrap(), 2);
    /// assert!(iter.next().is_none());
    /// ```
    pub fn try_iter(&self) -> TryIter<&Self> {
        TryIter::new(self)
    }

    /// Turn this channel into an iterator over pending values.
    /// For more information, see [`Self::try_iter`].
    pub fn into_try_iter(self) -> TryIter<Self> {
        TryIter::new(self)
    }
}

impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = Iter<Receiver<T>>;

    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self)
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.0.receivers.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

/// Creates a multi-producer, multi-consumer channel.
///
/// This channel has a buffer of size `capacity`,
/// and any writes beyond that will block until a read is performed.
///
/// The [`Sender`] and [`Receiver`] returned by this function are
/// cloneable and implement [`Send`], [`Sync`], and [`Clone`], meaning
/// that they can be used across thread boundaries.
///
/// # Examples
/// ```
/// use atomic_mpmc::channel;
///
/// let (sender, receiver) = channel::<i32>(10);
///
/// // send a value
/// sender.send(1).unwrap();
///
/// // receive a value
/// let value = receiver.recv().unwrap();
/// assert_eq!(value, 1);
/// ```
pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(Channel::new(capacity));
    (Sender::new(channel.clone()), Receiver::new(channel))
}
