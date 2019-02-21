//! A variant of RwLock with sublocks that can be opened for reading iff the main lock is currently
//! opened for reading, opened for writing iff the main lock is currently opened for writing.

use std::cell::{ UnsafeCell };
use std::ops::{ Deref, DerefMut };
use std::sync::atomic::{ AtomicBool, Ordering };
use std::sync::{ Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockResult };
pub use std::sync::LockResult;

pub struct Liveness {
    /// `true` as long as the `MainLock is acquired, `false` after that.
    is_alive: AtomicBool,

    /// `true` if the `MainLock was acquired mutable, `false` otherwise.
    is_mut: AtomicBool
}

pub struct SubCell<T> {
    cell: UnsafeCell<T>,

    liveness: Arc<Liveness>,
}

impl<T> SubCell<T> {
    pub fn new<'a>(liveness: &Arc<Liveness>, value: T) -> Self {
        SubCell {
            cell: UnsafeCell::new(value),
            liveness: liveness.clone(),
        }
    }
    pub fn borrow(&self) -> &T {
        assert!(self.liveness.is_alive.load(Ordering::Relaxed), "Attempting to borrow after the MainLock was released");
        unsafe { &*self.cell.get() }
    }

    pub fn borrow_mut(&self) -> &mut T {
        assert!(self.liveness.is_alive.load(Ordering::Relaxed), "Attempting to borrow_mut after the MainLock was released.");
        assert!(self.liveness.is_mut.load(Ordering::Relaxed), "Attempting to borrow_mut but the MainLock was acquired immutably.");
        unsafe { &mut *self.cell.get() }
    }

}

/// With respect to Send and Sync, SubCell behaves as a RwLock.
unsafe impl<T> Send for SubCell<T> where T: Send + Sync {
}

/// With respect to Send and Sync, SubCell behaves as a RwLock.
unsafe impl<T> Sync for SubCell<T> where T: Send + Sync {
}

/// A variant of RwLock with sublocks that can be opened for reading iff the main lock is currently
/// opened for reading, opened for writing iff the main lock is currently opened for writing.
///
/// ```
/// use sublock::sync::atomlock::*;
///
/// use std::collections::HashMap;
/// use std::sync::Arc;
///
/// struct State {
///   live: Arc<Liveness>,
///   data: HashMap<usize, SubCell<usize>>
/// }
/// impl State {
///   fn insert(&mut self, key: usize, value: usize) {
///     self.data.insert(key, SubCell::new(&self.live, value));
///   }
/// }
///
/// let lock = MainLock::new(|liveness| State {
///   live: liveness.clone(),
///   data: HashMap::new()
/// });
///
/// {
///     println!("* Attempt to read in the MainLock.");
///     let guard = lock.read().unwrap();
///     assert_eq!(guard.data.len(), 0);
/// }
///
/// {
///     println!("* Attempt to write in the MainLock.");
///     let mut guard = lock.write().unwrap();
///     guard.insert(0, 42);
///     assert_eq!(guard.data.len(), 1);
/// }
///
/// {
///     println!("* Attempt to read in a SubCell in `read()`.");
///     let guard = lock.read().unwrap();
///     assert_eq!(guard.data.len(), 1);
///     let cell = guard.data.get(&0).unwrap();
///     assert_eq!(*cell.borrow(), 42);
/// }
///
/// {
///     println!("* Attempt to read and write in a SubCell in `write()`.");
///     let guard = lock.write().unwrap();
///     assert_eq!(guard.data.len(), 1);
///     let cell = guard.data.get(&0).unwrap();
///     assert_eq!(*cell.borrow(), 42);
///
///     *cell.borrow_mut() = 99;
///     assert_eq!(*cell.borrow(), 99);
/// }
///
/// {
///     println!("* Check that the SubCell changes are kept.");
///     let guard = lock.read().unwrap();
///     assert_eq!(guard.data.len(), 1);
///     let cell = guard.data.get(&0).unwrap();
///     assert_eq!(*cell.borrow(), 99);
/// }
/// ```
pub struct MainLock<T> {
    lock: RwLock<T>,
    liveness: Arc<Liveness>,
}

impl<T> Drop for MainLock<T> {
    fn drop(&mut self) {
        self.liveness.is_alive.store(false, Ordering::Relaxed);
        self.liveness.is_mut.store(false, Ordering::Relaxed);
    }
}

pub type ReadGuard<'a, T> = RwLockReadGuard<'a, T>;

pub struct WriteGuard<'a, T> where T: 'a {
    guard: RwLockWriteGuard<'a, T>,
    liveness: Arc<Liveness>
}
impl<'a, T> WriteGuard<'a, T> where T: 'a {
    fn new(guard: RwLockWriteGuard<'a, T>, liveness: &Arc<Liveness>) -> Self {
        liveness.is_mut.store(true, Ordering::Relaxed);
        WriteGuard {
            guard: guard,
            liveness: liveness.clone(),
        }
    }
}

impl<'a, T> Deref for WriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<'a, T> DerefMut for WriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl<'a, T> Drop for WriteGuard<'a, T> {
    fn drop(&mut self) {
        self.liveness.is_mut.store(false, Ordering::Relaxed)
    }
}

impl<T> MainLock<T> {
    pub fn new<F>(cb: F) -> Self
        where F: FnOnce(&Arc<Liveness>) -> T
    {
        let liveness = Arc::new(Liveness {
             is_alive: AtomicBool::new(true),
             is_mut: AtomicBool::new(false)
        });
        let value = cb(&liveness);
        MainLock {
            lock: RwLock::new(value),
            liveness: liveness
        }
    }

    pub fn read(&self) -> LockResult<ReadGuard<T>> {
        self.lock.read()
    }

    pub fn try_read(&self) ->  TryLockResult<ReadGuard<T>> {
        self.lock.try_read()
    }

    pub fn write(&self) -> LockResult<WriteGuard<T>> {
        match self.lock.write() {
            Ok(guard) => Ok(WriteGuard::new(guard, &self.liveness)),
            Err(poison) => Err(PoisonError::new(
                WriteGuard::new(poison.into_inner(), &self.liveness)
            ))
        }
    }

    pub fn try_write(&self) ->  TryLockResult<WriteGuard<T>> {
        use std::sync::TryLockError::*;
        match self.lock.try_write() {
            Ok(guard) => Ok(WriteGuard::new(guard, &self.liveness)),
            Err(WouldBlock) => Err(WouldBlock),
            Err(Poisoned(poison)) => Err(Poisoned(PoisonError::new(
                WriteGuard::new(poison.into_inner(), &self.liveness)
            )))
        }
    }

    pub fn liveness(&self) -> &Arc<Liveness> {
        &self.liveness
    }
}

/*
// The following test should not build. That's normal.
#[test]
fn test_should_not_build() {
    use std::collections::HashMap;
    struct State {
        liveness: Arc<Liveness>,
        data: HashMap<usize, usize>
    }

    let main = MainLock::new(|liveness| State {
        liveness: liveness.clone(),
        data: HashMap::new(),
    });

    {
        let data;
        {
            data = &main.read().unwrap().data
        }
    }
}
*/