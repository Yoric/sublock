//! A variant of RefCell with subcells that can be opened at no cost by providing a proof that the
//! main cell is opened.

use std::cell::{ BorrowError, BorrowMutError, Ref, RefCell, RefMut, UnsafeCell };
use std::marker::PhantomData;

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

    // The owner has type MainCell<_> and has a unique key equal to `owner_key`.
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

/// A proof that the MainCell is currently opened.
/// Its lifetime is limited by that of the ReadGuard that provided it.
pub struct Proof<'a>(usize, PhantomData<&'a()>);

/// A proof that the MainCell is currently opened mutably.
/// Its lifetime is limited by that of the WriteGuard that provided it.
pub struct ProofMut<'a>(usize, PhantomData<&'a()>);

pub type ReadGuard<'a, T> = (Proof<'a>, Ref<'a, T>);
pub type WriteGuard<'a, T> = (ProofMut<'a>, RefMut<'a, T>);

/// A variant of `RefCell` with subcells that can be opened at no cost by providing a proof
/// that the main cell is opened.
///
/// ```
/// use sublock::cell::proofcell::*;
/// use std::collections::HashMap;
///
/// type State = HashMap<usize, SubCell<usize>>;
/// let data : MainCell<State> = MainCell::new(HashMap::new());
///
/// {
///     println!("* Attempt to read in the MainCell.");
///     let (_, guard) = data.try_borrow().unwrap();
///     assert_eq!(guard.len(), 0);
/// }
///
/// {
///     println!("* Attempt to write in the MainCell.");
///     let (proof, mut guard) = data.try_borrow_mut().unwrap();
///     guard.insert(0, SubCell::new(&proof, 42));
///     assert_eq!(guard.len(), 1);
/// }
///
/// {
///     println!("* Attempt to read in a SubCell.");
///     let (proof, guard) = data.try_borrow().unwrap();
///     assert_eq!(guard.len(), 1);
///     let cell = guard.get(&0).unwrap();
///     assert_eq!(*cell.borrow(&proof), 42);
/// }
///
/// {
///     println!("* Attempt to read and write in a SubCell.");
///     let (proof, guard) = data.try_borrow_mut().unwrap();
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
///     let (proof, guard) = data.try_borrow().unwrap();
///     assert_eq!(guard.len(), 1);
///     let cell = guard.get(&0).unwrap();
///     assert_eq!(*cell.borrow(&proof), 99);
/// }
/// ```
pub struct MainCell<T> {
    cell: RefCell<T>,
    ownership: usize,
}
impl<T> MainCell<T> {
    pub fn new(value: T) -> Self {
        use std::mem;
        let ownership : usize = unsafe { mem::transmute(&value as *const T) };
        MainCell {
            cell: RefCell::new(value),
            ownership: ownership
        }
    }

    pub fn try_borrow(&self) -> Result<ReadGuard<T>, BorrowError> {
        let proof = Proof(self.ownership, PhantomData);
        match self.cell.try_borrow() {
            Ok(ok) => Ok((proof, ok)),
            Err(err) => Err(err)
        }
    }

    pub fn try_borrow_mut(&self) -> Result<WriteGuard<T>, BorrowMutError> {
        let proof = ProofMut(self.ownership, PhantomData);
        match self.cell.try_borrow_mut() {
            Ok(ok) => Ok((proof, ok)),
            Err(err) => Err(err)
        }
    }
}

