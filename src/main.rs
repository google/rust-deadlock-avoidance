use deadlock_proof_mutex::{DeadlockProofMutex, OuterMutexPermission};

fn main() {
    use std::sync::Arc;
    use std::thread;

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

    let my_thread_mutex_permission = OuterMutexPermission::get();

    let guard = mutex1.lock(my_thread_mutex_permission).unwrap();
    assert_eq!(*guard, 10);
    let my_thread_mutex_permission = guard.unlock();
    assert_eq!(*mutex2.lock(my_thread_mutex_permission).unwrap(), 20);
}
