use core::cell::UnsafeCell;

/// A dynamic link that can be used by multiple kinds of intrusive collections.
/// The `N` parameter represents the number of pointers that may be stored.
#[repr(transparent)]
#[derive(Debug)]
pub struct DynLink<const N: usize> {
    words: UnsafeCell<[usize; N]>,
}

impl<const N: usize> DynLink<N> {
    pub const fn new() -> Self {
        Self {
            words: UnsafeCell::new([0; N]),
        }
    }
}

impl<const N: usize> Default for DynLink<N> {
    fn default() -> Self {
        Self::new()
    }
}
