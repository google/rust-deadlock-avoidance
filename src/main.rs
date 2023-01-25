use deadlock_proof_mutex::{DeadlockProofMutex, MutexIdentifier, OuterMutexPermission};
use std::sync::Arc;
use std::thread;

struct MutexA;
impl MutexIdentifier for MutexA {}

struct MutexB;
impl MutexIdentifier for MutexB {}

struct MutexC;
impl MutexIdentifier for MutexC {}

struct MutexD;
impl MutexIdentifier for MutexD {}

struct MutexE;
impl MutexIdentifier for MutexE {}

struct MutexF;
impl MutexIdentifier for MutexF {}

struct MutexG;
impl MutexIdentifier for MutexG {}

struct MutexH;
impl MutexIdentifier for MutexH {}

fn example_with_exclusive_mutices(
    my_thread_mutex_permission: OuterMutexPermission,
) -> OuterMutexPermission {
    // We have two mutices, but each thread can only claim one at once,
    // hence deadlock-proof.
    let mutex1 = Arc::new(DeadlockProofMutex::new(0, MutexA));
    let mutex2 = Arc::new(DeadlockProofMutex::new(0, MutexB));
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

fn example_with_nested_mutices(
    my_thread_mutex_permission: OuterMutexPermission,
) -> OuterMutexPermission {
    // We have three nested mutices, and each thread is forced to claim
    // them in the same order, hence deadlock-proof. If any thread claims
    // them in a different order, the type system will prevent the code
    // from compiling.
    let mutex1 = Arc::new(DeadlockProofMutex::new(0, MutexC));
    let mutex2 = Arc::new(DeadlockProofMutex::new(0, MutexD));
    let mutex3 = Arc::new(DeadlockProofMutex::new(0, MutexE));
    let c_mutex1 = Arc::clone(&mutex1);
    let c_mutex2 = Arc::clone(&mutex2);
    let c_mutex3 = Arc::clone(&mutex3);

    thread::spawn(move || {
        let mutex_permission = OuterMutexPermission::get();
        let (mut guard, inner_permission) = c_mutex1.lock_for_nested(mutex_permission).unwrap();
        *guard = 10;

        // We now have permission to unlock mutex2
        let (mut guard2, inner_inner_permission) =
            c_mutex2.lock_for_nested(inner_permission).unwrap();
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
    let (guard, inner_permission) = mutex1.lock_for_nested(my_thread_mutex_permission).unwrap();
    assert_eq!(*guard, 10);
    let (guard2, inner_inner_permission) = mutex2.lock_for_nested(inner_permission).unwrap();
    assert_eq!(*guard2, 20);
    let guard3 = mutex3.lock(inner_inner_permission).unwrap();
    assert_eq!(*guard3, 30);
    guard.unlock(guard2.unlock(guard3.unlock()))
}

fn example_with_sequential_mutices(my_thread_mutex_permission: OuterMutexPermission) {
    // We have three sequential mutices. The type system ensures we always
    // claim A then B then C, never C then B then A.
    let mutex1 = Arc::new(DeadlockProofMutex::new(0, MutexF));
    let mutex2 = Arc::new(DeadlockProofMutex::new(0, MutexG));
    let mutex3 = Arc::new(DeadlockProofMutex::new(0, MutexH));
    let c_mutex1 = Arc::clone(&mutex1);
    let c_mutex2 = Arc::clone(&mutex2);
    let c_mutex3 = Arc::clone(&mutex3);

    thread::spawn(move || {
        let mutex_permission = OuterMutexPermission::get();
        let mut guard = c_mutex1.lock(mutex_permission).unwrap();
        *guard = 10;
        let next_permission = guard.unlock_for_sequential();

        // We now have permission to unlock mutex2
        let mut guard2 = c_mutex2.lock(next_permission).unwrap();
        *guard2 = 20;
        let next_permission = guard2.unlock_for_sequential();

        // We now have permission to unlock mutex3
        let mut guard3 = c_mutex3.lock(next_permission).unwrap();
        *guard3 = 30;

        // Explicitly unlock, to show how to get back to the
        // outermost mutex in case we need to claim something else.
        let _mutex_permission = guard3.unlock().to_earlier().to_earlier();
    })
    .join()
    .expect("thread::spawn failed");

    // The type system will now insist we claim and release the mutices in the
    // same order.
    let guard = mutex1.lock(my_thread_mutex_permission).unwrap();
    assert_eq!(*guard, 10);
    let next_permission = guard.unlock_for_sequential();
    let guard2 = mutex2.lock(next_permission).unwrap();
    assert_eq!(*guard2, 20);
    let next_permission = guard2.unlock_for_sequential();
    let guard3 = mutex3.lock(next_permission).unwrap();
    assert_eq!(*guard3, 30);
}

fn main() {
    let my_thread_mutex_permission = OuterMutexPermission::get();
    let my_thread_mutex_permission = example_with_exclusive_mutices(my_thread_mutex_permission);
    let my_thread_mutex_permission = example_with_nested_mutices(my_thread_mutex_permission);
    example_with_sequential_mutices(my_thread_mutex_permission);
}
