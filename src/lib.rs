use std::{marker::PhantomData, rc::Rc};

use std::{
    ops::{Deref, DerefMut},
    sync::{Mutex, MutexGuard, PoisonError},
};

/// Some type of permission token required to claim a mutex.
pub trait MutexPermission {}

impl MutexPermission for OuterMutexPermission {}

/// Permission to claim an "outer" mutex. That is, a class of mutices where
/// only one can be claimed at once in each thread, thus preventing deadlock.
/// An instance of this object can be obtained using [`OuterMutexPermission::get`].
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

/// Permission to claim some nested mutex. This can be obtained from
/// [`DeadlockProofMutex::lock_for_nested`].
pub struct NestedMutexPermission<P: MutexPermission>(PhantomData<Rc<()>>, PhantomData<P>);

impl<P: MutexPermission> NestedMutexPermission<P> {
    fn new() -> Self {
        Self(PhantomData, PhantomData)
    }
}

impl<P: MutexPermission> MutexPermission for NestedMutexPermission<P> {}

/// Permission to claim some nested mutex. This can be obtained from
/// [`DeadlockProofMutex::lock_for_nested`].
pub struct SequentialMutexPermission<P: MutexPermission>(PhantomData<Rc<()>>, P);

impl<P: MutexPermission> SequentialMutexPermission<P> {
    fn new(permission: P) -> Self {
        Self(PhantomData, permission)
    }

    /// Consumes this sequential permission to return the permission
    /// token earlier in the sequence.
    pub fn to_earlier(self) -> P {
        self.1
    }
}

impl<P: MutexPermission> MutexPermission for SequentialMutexPermission<P> {}

struct PermissionSyncSendWrapper<P: MutexPermission>(P);

/// Unsafety: these types are only ever used within `PhantomData` and not
/// exposed beyond this mod, so this is not semantically important.
/// We need to do this because these permission tokens must not themselves
/// be sent between threads (we carefully ensure they're not `Send`) but
/// the mutex needs to be parameterized over this permission type.
unsafe impl<P: MutexPermission> Send for PermissionSyncSendWrapper<P> {}
unsafe impl<P: MutexPermission> Sync for PermissionSyncSendWrapper<P> {}

/// A mutex which is compile-time guaranteed not to deadlock.
/// Otherwise identical to [`Mutex`], though at the moment only a subset
/// of APIs are implemented.
///
/// To use this, you will need to obtain some form of mutex permission token.
/// One of these can be obtained per thread from [`OuterMutexPermission::get`].
/// Other such permission tokens can be obtained from APIs within this class
/// itself. Three patterns are possible:
///
/// * Each thread can hold only one mutex at once (because each thread uses
///   a [`OuterMutexPermission`]
/// * Each thread claims mutex in a specific identical nested order. The
///   first mutex is claimed using a [`OuterMutexPermission`] and subsequent
///   mutices are claimed using [`DeadlockProofMutex::lock_for_nested`].
/// * Each thread claims mutices then releases them in a specific identical
///   nested order. The first mutex is claimed using [`OuterMutexPermission`]
///   and subsequent mutices are claimed using [`DeadlockProofMutexGuard::unlock_for_sequential`]
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
    pub fn lock_for_nested(
        &self,
        permission: P,
    ) -> Result<
        (
            DeadlockProofNestedMutexGuard<T, P>,
            NestedMutexPermission<P>,
        ),
        PoisonError<MutexGuard<T>>,
    > {
        self.0.lock().map(|guard| {
            (
                DeadlockProofNestedMutexGuard(guard, permission),
                NestedMutexPermission::new(),
            )
        })
    }
}

/// Deadlock-proof equivalent to [`MutexGuard`]. It's strongly recommended that you don't
/// allow this mutex to drop, but instead explicitly call [`DeadlockProofMutexGuard::unlock`] to obtain
/// the permission required to reclaim a mutex later.
pub struct DeadlockProofMutexGuard<'a, T, P: MutexPermission>(MutexGuard<'a, T>, P);

impl<'a, T, P: MutexPermission> DeadlockProofMutexGuard<'a, T, P> {
    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex.
    pub fn unlock(self) -> P {
        self.1
    }

    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex. Also, returns an extra
    /// mutex permission token so that you can claim another mutex in
    /// a certain sequence, which the type system will guarantee is the same
    /// for all threads.
    pub fn unlock_for_sequential(self) -> SequentialMutexPermission<P> {
        SequentialMutexPermission::new(self.1)
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
/// allow this mutex to drop, but instead explicitly call [`DeadlockProofMutexGuard::unlock`] to obtain
/// the permission required to reclaim a mutex later.
pub struct DeadlockProofNestedMutexGuard<'a, T, P: MutexPermission>(MutexGuard<'a, T>, P);

impl<'a, T, P: MutexPermission> DeadlockProofNestedMutexGuard<'a, T, P> {
    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex.
    pub fn unlock(self, _token: NestedMutexPermission<P>) -> P {
        self.1
    }

    /// Unlock the mutex. Returns the mutex permission token such that you
    /// can use it again to claim a different mutex. Also, returns an extra
    /// mutex permission token so that you can claim another mutex in
    /// a certain sequence, which the type system will guarantee is the same
    /// for all threads.
    pub fn unlock_for_sequential(self) -> SequentialMutexPermission<P> {
        SequentialMutexPermission::new(self.1)
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
