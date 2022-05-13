use std::mem::forget;

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
    let v = Arc::new(AtomicUsize::new(0));
    struct Dropper(Arc<AtomicUsize>);

    impl Drop for Dropper {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            println!("dropped");
        }
    }

    let (sender, receiver) = channel::<Dropper>(10);

    let vc = v.clone();
    let handle = std::thread::spawn(move || {
        for _ in 0..10 {
            sender.send(Dropper(vc.clone())).unwrap();
        }
    });

    for _ in 0..3 {
        // Forget 3 values.
        forget(receiver.recv().unwrap());
    }

    // Drop the rest of the values.
    drop(receiver);
    handle.join().unwrap();
    
    assert_eq!(v.load(Ordering::SeqCst), 7);
}