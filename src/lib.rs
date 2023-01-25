use std::{marker::PhantomData, rc::Rc};

use std::{
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard, PoisonError},
};

/// Type representing permission to claim a mutex. This type is !Send and
/// cannot be created outside the current mod due to the private field.
pub struct MutexPermission(PhantomData<Rc<()>>);

thread_local! {
pub static MUTEX_PERMISSION_TOKEN: std::cell::Cell<Option<MutexPermission>>
= std::cell::Cell::new(Some(MutexPermission(PhantomData)))
}

impl MutexPermission {
    /// Get the thread-local mutex claiming permission. This can be called exactly once
    /// per thread, and will panic if it's called more than once in a thread.
    /// Because it may panic, it's strongly recommended that you claim this in the
    /// start up of your program (or thread) and store it in some context object.
    /// This eliminates any chance of runtime panics later.
    /// The resulting zero-sized type can be used as permission to claim a mutex.
    pub fn get() -> MutexPermission {
        MUTEX_PERMISSION_TOKEN
            .with(|thingref| thingref.take())
            .expect("Mutex permission already claimed for this thread")
    }
}

/// A mutex which is compile-time guaranteed not to deadlock.
/// Otherwise identical to [`Mutex`].
pub struct DeadlockProofMutex<T>(Mutex<T>);

impl<T> DeadlockProofMutex<T> {
    /// Create a new deadlock-proof mutex.
    pub fn new(content: T) -> Self {
        Self(Mutex::new(content))
    }

    /// Acquires this mutex, blocking the current thread until it
    /// is able to do so. Similar to [`Mutex::lock`], but requires a permission
    /// token to prove that you can't be causing a deadlock.
    pub fn lock(
        &self,
        permission: MutexPermission,
    ) -> Result<DeadlockProofMutexGuard<T>, PoisonError<MutexGuard<T>>> {
        self.0
            .lock()
            .map(|guard| DeadlockProofMutexGuard(guard, permission))
    }
}

/// Deadlock-proof equivalent to [`MutexGuard`]. It's strongly recommended that you don't
/// allow this mutex to drop, but instead explicitly call [`unlock`] to obtain
/// the permission required to reclaim a mutex later.
pub struct DeadlockProofMutexGuard<'a, T>(MutexGuard<'a, T>, MutexPermission);

impl<'a, T> DeadlockProofMutexGuard<'a, T> {
    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex.
    pub fn unlock(self) -> MutexPermission {
        self.1
    }
}

impl<T> Deref for DeadlockProofMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.deref()
    }
}

impl<T> DerefMut for DeadlockProofMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.deref_mut()
    }
}