use std::{cell::Cell, mem::forget};

use super::*;

#[test]
fn test_read_and_write() {
    let (sender, receiver) = channel::<Vec<u32>>(3);
    sender.send(vec![1, 2, 3]).unwrap();
    sender.send(vec![4, 5, 6]).unwrap();
    sender.send(vec![7, 8, 9]).unwrap();
    assert_eq!(receiver.recv().unwrap(), vec![1, 2, 3]);
    assert_eq!(receiver.recv().unwrap(), vec![4, 5, 6]);
    assert_eq!(receiver.recv().unwrap(), vec![7, 8, 9]);
}

#[test]
#[cfg_attr(miri, ignore)]
fn test_read_blocking() {
    let (sender, receiver) = channel::<Vec<u32>>(3);

    std::thread::spawn(move || {
        for i in 0..10 {
            sender.send(vec![i]).unwrap();
            println!("sent {i}");
        }
    });

    for i in 0..10 {
        assert_eq!(receiver.recv().unwrap(), vec![i]);
        println!("received {i}");
    }
}

#[test]
fn test_value_drop() {
    let v = Cell::new(0);
    struct Dropper<'a>(&'a Cell<u32>);

    impl<'a> Drop for Dropper<'a> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let (sender, receiver) = channel::<Dropper>(10);

    for _ in 0..10 {
        sender.send(Dropper(&v)).unwrap();
    }

    for _ in 0..3 {
        // Forget 3 values.
        forget(receiver.recv().unwrap());
    }

    // Drop the rest of the values.
    drop(sender);
    drop(receiver);

    assert_eq!(v.get(), 7);
}

#[test]
fn test_receiver_hang_up() {
    let (sender, receiver) = channel::<u32>(1);

    sender.send(1).unwrap();
    assert_eq!(receiver.recv().unwrap(), 1);

    drop(receiver);
    assert!(sender.send(2).is_err());
}

#[test]
fn test_sender_hang_up() {
    let (sender, receiver) = channel::<u32>(1);

    sender.send(1).unwrap();
    assert_eq!(receiver.recv().unwrap(), 1);

    drop(sender);
    assert!(receiver.recv().is_err());
}
