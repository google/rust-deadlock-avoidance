use std::{marker::PhantomData, rc::Rc};

use std::{
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard, PoisonError},
};

/// Some type of permission token required to claim a mutex.
pub trait MutexPermission {}

impl MutexPermission for OuterMutexPermission {}

/// Type representing permission to claim a mutex. This type is !Send and
/// cannot be created outside the current mod due to the private field.
pub struct OuterMutexPermission(PhantomData<Rc<()>>);

thread_local! {
pub static MUTEX_PERMISSION_TOKEN: std::cell::Cell<Option<OuterMutexPermission>>
= std::cell::Cell::new(Some(OuterMutexPermission(PhantomData)))
}

impl OuterMutexPermission {
    /// Get the thread-local mutex claiming permission. This can be called exactly once
    /// per thread, and will panic if it's called more than once in a thread.
    /// Because it may panic, it's strongly recommended that you claim this in the
    /// start up of your program (or thread) and store it in some context object.
    /// This eliminates any chance of runtime panics later.
    /// The resulting zero-sized type can be used as permission to claim a mutex.
    pub fn get() -> OuterMutexPermission {
        MUTEX_PERMISSION_TOKEN
            .with(|thingref| thingref.take())
            .expect("Mutex permission already claimed for this thread")
    }
}

pub struct InnerMutexPermission<P: MutexPermission>(PhantomData<Rc<()>>, PhantomData<P>);

impl<P: MutexPermission> InnerMutexPermission<P> {
    fn new() -> Self {
        Self(PhantomData, PhantomData)
    }
}

impl<P: MutexPermission> MutexPermission for InnerMutexPermission<P> {}

struct PermissionSyncSendWrapper<P: MutexPermission>(P);

unsafe impl<P: MutexPermission> Send for PermissionSyncSendWrapper<P> {}
unsafe impl<P: MutexPermission> Sync for PermissionSyncSendWrapper<P> {}

/// A mutex which is compile-time guaranteed not to deadlock.
/// Otherwise identical to [`Mutex`].
pub struct DeadlockProofMutex<T, P: MutexPermission>(
    Mutex<T>,
    PhantomData<PermissionSyncSendWrapper<P>>,
);

impl<T, P: MutexPermission> DeadlockProofMutex<T, P> {
    /// Create a new deadlock-proof mutex.
    pub fn new(content: T) -> Self {
        Self(Mutex::new(content), PhantomData)
    }

    /// Acquires this mutex, blocking the current thread until it
    /// is able to do so. Similar to [`Mutex::lock`], but requires a permission
    /// token to prove that you can't be causing a deadlock.
    pub fn lock(
        &self,
        permission: P,
    ) -> Result<DeadlockProofMutexGuard<T, P>, PoisonError<MutexGuard<T>>> {
        self.0
            .lock()
            .map(|guard| DeadlockProofMutexGuard(guard, permission))
    }

    /// Acquires this mutex, blocking the current thread until it
    /// is able to do so. Provides a token which can be used to claim a
    /// nested mutex.
    pub fn lock_allowing_nested(
        &self,
        permission: P,
    ) -> Result<
        (DeadlockProofNestedMutexGuard<T, P>, InnerMutexPermission<P>),
        PoisonError<MutexGuard<T>>,
    > {
        self.0.lock().map(|guard| {
            (
                DeadlockProofNestedMutexGuard(guard, permission),
                InnerMutexPermission::new(),
            )
        })
    }
}

/// Deadlock-proof equivalent to [`MutexGuard`]. It's strongly recommended that you don't
/// allow this mutex to drop, but instead explicitly call [`unlock`] to obtain
/// the permission required to reclaim a mutex later.
pub struct DeadlockProofMutexGuard<'a, T, P: MutexPermission>(MutexGuard<'a, T>, P);

impl<'a, T, P: MutexPermission> DeadlockProofMutexGuard<'a, T, P> {
    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex.
    pub fn unlock(self) -> P {
        self.1
    }
}

impl<T, P: MutexPermission> Deref for DeadlockProofMutexGuard<'_, T, P> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.deref()
    }
}

impl<T, P: MutexPermission> DerefMut for DeadlockProofMutexGuard<'_, T, P> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.deref_mut()
    }
}

/// Deadlock-proof equivalent to [`MutexGuard`]. It's strongly recommended that you don't
/// allow this mutex to drop, but instead explicitly call [`unlock`] to obtain
/// the permission required to reclaim a mutex later.
pub struct DeadlockProofNestedMutexGuard<'a, T, P: MutexPermission>(MutexGuard<'a, T>, P);

impl<'a, T, P: MutexPermission> DeadlockProofNestedMutexGuard<'a, T, P> {
    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex.
    pub fn unlock(self, _token: InnerMutexPermission<P>) -> P {
        self.1
    }
}

impl<T, P: MutexPermission> Deref for DeadlockProofNestedMutexGuard<'_, T, P> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0.deref()
    }
}

impl<T, P: MutexPermission> DerefMut for DeadlockProofNestedMutexGuard<'_, T, P> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.deref_mut()
    }
}
