use deadlock_proof_mutex::{DeadlockProofMutex, OuterMutexPermission};
use std::sync::Arc;
use std::thread;

fn example_with_exclusive_mutices(my_thread_mutex_permission: OuterMutexPermission) -> OuterMutexPermission {
    // We have two mutices, but each thread can only claim one at once,
    // hence mutex-proof.
    let mutex1 = Arc::new(DeadlockProofMutex::new(0));
    let mutex2 = Arc::new(DeadlockProofMutex::new(0));
    let c_mutex1 = Arc::clone(&mutex1);
    let c_mutex2 = Arc::clone(&mutex2);

    thread::spawn(move || {
        let mutex_permission = OuterMutexPermission::get();
        let mut guard = c_mutex1.lock(mutex_permission).unwrap();
        *guard = 10;
        let mutex_permission = guard.unlock();
        let mut guard = c_mutex2.lock(mutex_permission).unwrap();
        *guard = 20;
    })
    .join()
    .expect("thread::spawn failed");

    let guard = mutex1.lock(my_thread_mutex_permission).unwrap();
    assert_eq!(*guard, 10);
    let my_thread_mutex_permission = guard.unlock();
    let guard2 = mutex2.lock(my_thread_mutex_permission).unwrap();
    assert_eq!(*guard2, 20);
    guard2.unlock()
}

fn example_with_nested_mutices(my_thread_mutex_permission: OuterMutexPermission) {
    // We have three nested mutices, and each thread is forced to claim
    // them in the same order.

    let mutex1 = Arc::new(DeadlockProofMutex::new(0));
    let mutex2 = Arc::new(DeadlockProofMutex::new(0));
    let mutex3 = Arc::new(DeadlockProofMutex::new(0));
    let c_mutex1 = Arc::clone(&mutex1);
    let c_mutex2 = Arc::clone(&mutex2);
    let c_mutex3 = Arc::clone(&mutex3);

    thread::spawn(move || {
        let mutex_permission = OuterMutexPermission::get();
        let (mut guard, inner_permission) =
            c_mutex1.lock_allowing_nested(mutex_permission).unwrap();
        *guard = 10;

        // We now have permission to unlock mutex2
        let (mut guard2, inner_inner_permission) =
            c_mutex2.lock_allowing_nested(inner_permission).unwrap();
        *guard2 = 20;

        // We now have permission to unlock mutex3
        let mut guard3 = c_mutex3.lock(inner_inner_permission).unwrap();
        *guard3 = 30;

        // Explicitly unlock, to show how
        let inner_inner_permission = guard3.unlock();
        guard2.unlock(inner_inner_permission);
    })
    .join()
    .expect("thread::spawn failed");

    // The type system will now insist we claim the mutices in the same order.
    let (guard, inner_permission) = mutex1
        .lock_allowing_nested(my_thread_mutex_permission)
        .unwrap();
    assert_eq!(*guard, 10);
    let (guard2, inner_inner_permission) = mutex2.lock_allowing_nested(inner_permission).unwrap();
    assert_eq!(*guard2, 20);
    let guard3 = mutex3.lock(inner_inner_permission).unwrap();
    assert_eq!(*guard3, 30);
}

fn main() {
    let my_thread_mutex_permission = OuterMutexPermission::get();
    let my_thread_mutex_permission = example_with_exclusive_mutices(my_thread_mutex_permission);
    example_with_nested_mutices(my_thread_mutex_permission);
}
