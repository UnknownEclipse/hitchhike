#![no_std]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, sync::Arc};
use core::{
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    ptr::{self, NonNull},
};

#[doc(hidden)]
pub use memoffset as __memoffset;

pub mod dyn_link;
pub mod mpsc_queue;

pub trait Node<Link> {
    fn acquire(node: &Self) -> bool;
    unsafe fn release(node: &Self);
    unsafe fn as_link(node: NonNull<Self>) -> NonNull<Link>;
    unsafe fn as_node(link: NonNull<Link>) -> NonNull<Self>;
}

pub trait Pointer: Deref<Target = Self::Pointee> {
    type Pointee;

    fn into_raw(self) -> NonNull<Self::Pointee>;
    unsafe fn from_raw(ptr: NonNull<Self::Pointee>) -> Self;
}

impl<T> Pointer for Box<T> {
    type Pointee = T;

    #[inline]
    fn into_raw(self) -> NonNull<Self::Pointee> {
        unsafe { NonNull::new_unchecked(Box::into_raw(self)) }
    }

    #[inline]
    unsafe fn from_raw(ptr: NonNull<Self::Pointee>) -> Self {
        unsafe { Box::from_raw(ptr.as_ptr()) }
    }
}

impl<T> Pointer for Rc<T> {
    type Pointee = T;

    #[inline]
    fn into_raw(self) -> NonNull<Self::Pointee> {
        unsafe { NonNull::new_unchecked(Rc::into_raw(self).cast_mut()) }
    }

    #[inline]
    unsafe fn from_raw(ptr: NonNull<Self::Pointee>) -> Self {
        unsafe { Rc::from_raw(ptr.as_ptr()) }
    }
}

impl<T> Pointer for Arc<T> {
    type Pointee = T;

    #[inline]
    fn into_raw(self) -> NonNull<Self::Pointee> {
        unsafe { NonNull::new_unchecked(Arc::into_raw(self).cast_mut()) }
    }

    #[inline]
    unsafe fn from_raw(ptr: NonNull<Self::Pointee>) -> Self {
        unsafe { Arc::from_raw(ptr.as_ptr()) }
    }
}

pub struct UnsafeRef<T> {
    ptr: NonNull<T>,
}

impl<T> Deref for UnsafeRef<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

unsafe impl<T> Send for UnsafeRef<T> where for<'a> &'a T: Send {}
unsafe impl<T> Sync for UnsafeRef<T> where for<'a> &'a T: Sync {}

#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $ty:path, $field:ident) => {{
        let field_offset = $crate::__memoffset::offset_of!($ty, $field);
        let byte_ptr = $ptr as *const u8;
        let parent_byte_ptr = byte_ptr.sub(field_offset);
        parent_byte_ptr as *mut $ty
    }};
}

#[macro_export]
macro_rules! container_of_nonnull {
    ($ptr:expr, $ty:path, $field:ident) => {{
        let ptr = $ptr.as_ptr();
        let ptr = $crate::container_of(ptr, $ty, $field);
        unsafe { NonNull::new_unchecked(ptr) }
    }};
}

#[derive(Debug)]
pub struct AcquireError<P>(pub P);

#[derive(Debug)]
pub struct Acquired<P, L>(P, PhantomData<L>)
where
    P: Pointer,
    P::Pointee: Node<L>;

impl<P, L> Acquired<P, L>
where
    P: Pointer,
    P::Pointee: Node<L>,
{
    pub unsafe fn from_link_unchecked(link: NonNull<L>) -> Self {
        let raw = <P::Pointee>::as_node(link);
        unsafe { Self(P::from_raw(raw), PhantomData) }
    }

    pub unsafe fn new_unchecked(node: P) -> Self {
        Self(node, PhantomData)
    }

    pub fn release(self) -> P {
        let node = self.leak();
        unsafe { <P::Pointee>::release(&node) };
        node
    }
}

impl<P, L> Acquired<P, L>
where
    P: Pointer,
    P::Pointee: Node<L>,
{
    pub fn into_link(self) -> NonNull<L> {
        let raw = self.leak().into_raw();
        unsafe { <P::Pointee>::as_link(raw) }
    }

    fn leak(self) -> P {
        unsafe { ptr::read(&ManuallyDrop::new(self).0) }
    }
}

impl<P, L> Drop for Acquired<P, L>
where
    P: Pointer,
    P::Pointee: Node<L>,
{
    fn drop(&mut self) {
        todo!()
    }
}

pub fn acquire<P, L>(node: P) -> Result<Acquired<P, L>, AcquireError<P>>
where
    P: Pointer,
    P::Pointee: Node<L>,
{
    if <P::Pointee>::acquire(&node) {
        Ok(Acquired(node, PhantomData))
    } else {
        Err(AcquireError(node))
    }
}
