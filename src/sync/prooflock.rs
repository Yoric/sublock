//! A variant of RwLock with sublocks that can be opened at no cost by providing a proof that the
//! main lock is opened.

use std::cell::{ UnsafeCell };
use std::marker::PhantomData;
use std::sync::{ LockResult, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockResult };

/// A trait specifying that a structure supports immutable borrowing if some proof is provided.
pub trait ProofBorrow<P, T> {
    fn borrow<'a>(&'a self, proof: &P) -> &'a T;
}


/// A trait specifying that a structure supports mutable borrowing if some proof is provided.
pub trait ProofBorrowMut<P, T> {
    fn borrow_mut<'a>(&'a self, proof: &P) -> &'a mut T;
}

pub struct SubCell<T> {
    cell: UnsafeCell<T>,

    // The owner has type BigLock<_> and has a unique key equal to `owner_key`.
    owner_key: usize,
}

impl<T> SubCell<T> {
    pub fn new<'a>(proof: &ProofMut<'a>, value: T) -> Self {
        SubCell {
            cell: UnsafeCell::new(value),
            owner_key: proof.0,
        }
    }
}

impl<'b, T> ProofBorrow<Proof<'b>, T> for SubCell<T> {
    fn borrow<'a>(&'a self, proof: &Proof<'b>) -> &'a T {
        assert_eq!(self.owner_key, proof.0);
        unsafe { &*self.cell.get() }
    }
}

impl<'b, T> ProofBorrow<ProofMut<'b>, T> for SubCell<T> {
    fn borrow<'a>(&'a self, proof: &ProofMut<'b>) -> &'a T {
        assert_eq!(self.owner_key, proof.0);
        unsafe { &*self.cell.get() }
    }
}

impl<'b, T> ProofBorrowMut<ProofMut<'b>, T> for SubCell<T> {
    fn borrow_mut<'a>(&'a self, proof: &ProofMut<'b>) -> &'a mut T {
        assert_eq!(self.owner_key, proof.0);
        unsafe { &mut *self.cell.get() }
    }
}

/// With respect to Send and Sync, SubCell behaves as a RwLock.
unsafe impl<T> Send for SubCell<T> where T: Send + Sync { }

/// With respect to Send and Sync, SubCell behaves as a RwLock.
unsafe impl<T> Sync for SubCell<T> where T: Send + Sync { }

/// A proof that the MainLock is currently opened.
/// Its lifetime is limited by that of the ReadGuard that provided it.
pub struct Proof<'a>(usize, PhantomData<&'a()>);

/// A proof that the MainLock is currently opened mutably.
/// Its lifetime is limited by that of the WriteGuard that provided it.
pub struct ProofMut<'a>(usize, PhantomData<&'a()>);

pub type ReadGuard<'a, T> = (Proof<'a>, RwLockReadGuard<'a, T>);
pub type WriteGuard<'a, T> = (ProofMut<'a>, RwLockWriteGuard<'a, T>);

/// A variant of `RwLock` with sublocks that can be opened at no cost by providing a proof
/// that the main lock is opened.
///
/// ```
/// use sublock::sync::prooflock::*;
/// use std::collections::HashMap;
///
/// type State = HashMap<usize, SubCell<usize>>;
/// let data : MainLock<State> = MainLock::new(HashMap::new());
///
/// {
///     println!("* Attempt to read in the MainLock.");
///     let (_, guard) = data.read().unwrap();
///     assert_eq!(guard.len(), 0);
/// }
///
/// {
///     println!("* Attempt to write in the MainLock.");
///     let (proof, mut guard) = data.write().unwrap();
///     guard.insert(0, SubCell::new(&proof, 42));
///     assert_eq!(guard.len(), 1);
/// }
///
/// {
///     println!("* Attempt to read in a SubCell.");
///     let (proof, guard) = data.read().unwrap();
///     assert_eq!(guard.len(), 1);
///     let cell = guard.get(&0).unwrap();
///     assert_eq!(*cell.borrow(&proof), 42);
/// }
///
/// {
///     println!("* Attempt to read and write in a SubCell.");
///     let (proof, guard) = data.write().unwrap();
///     assert_eq!(guard.len(), 1);
///     let cell = guard.get(&0).unwrap();
///     assert_eq!(*cell.borrow(&proof), 42);
///
///     *cell.borrow_mut(&proof) = 99;
///     assert_eq!(*cell.borrow(&proof), 99);
/// }
///
/// {
///     println!("* Check that the SubCell changes are kept.");
///     let (proof, guard) = data.read().unwrap();
///     assert_eq!(guard.len(), 1);
///     let cell = guard.get(&0).unwrap();
///     assert_eq!(*cell.borrow(&proof), 99);
/// }
/// ```
pub struct MainLock<T> {
    lock: RwLock<T>,
    ownership: usize,
}
impl<T> MainLock<T> {
    pub fn new(value: T) -> Self {
        use std::mem;
        let ownership : usize = unsafe { mem::transmute(&value as *const T) };
        MainLock {
            lock: RwLock::new(value),
            ownership: ownership
        }
    }

    // As `RwLock.read`.
    pub fn read(&self) -> LockResult<ReadGuard<T>> {
        let proof = Proof(self.ownership, PhantomData);
        match self.lock.read() {
            Ok(ok) => Ok((proof, ok)),
            Err(err) => Err(PoisonError::new((proof, err.into_inner())))
        }
    }

    // As `RwLock.try_read`.
    pub fn try_read(&self) ->  TryLockResult<ReadGuard<T>> {
        use std::sync::TryLockError::*;
        let proof = Proof(self.ownership, PhantomData);
        match self.lock.try_read() {
            Ok(ok) => Ok((proof, ok)),
            Err(WouldBlock) => Err(WouldBlock),
            Err(Poisoned(err)) => Err(Poisoned(PoisonError::new((proof, err.into_inner()))))
        }
    }

    // As `RwLock.write`.
    pub fn write(&self) -> LockResult<WriteGuard<T>> {
        let proof = ProofMut(self.ownership, PhantomData);
        match self.lock.write() {
            Ok(ok) => Ok((proof, ok)),
            Err(err) => Err(PoisonError::new((proof, err.into_inner())))
        }
    }

    // As `RwLock.try_write`.
    pub fn try_write(&self) ->  TryLockResult<WriteGuard<T>> {
        use std::sync::TryLockError::*;
        let proof = ProofMut(self.ownership, PhantomData);
        match self.lock.try_write() {
            Ok(ok) => Ok((proof, ok)),
            Err(WouldBlock) => Err(WouldBlock),
            Err(Poisoned(err)) => Err(Poisoned(PoisonError::new((proof, err.into_inner()))))
        }
    }
}

