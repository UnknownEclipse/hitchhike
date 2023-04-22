use core::{
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

use atomic::Atomic;

use crate::{acquire, AcquireError, Acquired, Node, Pointer};

#[derive(Debug, Default)]
pub struct Stub {
    link: Link,
}

impl Stub {
    pub const fn new() -> Self {
        Self { link: Link::new() }
    }
}

pub struct MpscQueue<'stub, P> {
    raw: UnsafeMpscQueue,
    consuming: AtomicBool,
    _p: PhantomData<(&'stub Stub, P)>,
}

impl<'stub, P> MpscQueue<'stub, P>
where
    P: Pointer,
    P::Pointee: Node<Link>,
{
    pub fn with_stub(stub: &'stub mut Stub) -> Self {
        let stub = NonNull::from(&stub.link);

        Self {
            raw: unsafe { UnsafeMpscQueue::with_stub(stub) },
            consuming: AtomicBool::new(false),
            _p: PhantomData,
        }
    }

    pub fn push_acquired(&self, node: Acquired<P, Link>) {
        let link = node.into_link();
        unsafe { self.raw.push(link) };
    }

    pub fn push(&self, node: P) -> Result<(), AcquireError<P>> {
        let acq = acquire(node)?;
        self.push_acquired(acq);
        Ok(())
    }

    pub fn consumer(&self) -> Option<Consumer<'_, P>> {
        if self.consuming.swap(true, Ordering::Acquire) {
            None
        } else {
            Some(Consumer {
                queue: &self.raw,
                consuming: &self.consuming,
                _p: PhantomData,
            })
        }
    }
}

pub struct Consumer<'a, P> {
    queue: &'a UnsafeMpscQueue,
    consuming: &'a AtomicBool,
    _p: PhantomData<&'a P>,
}

impl<'a, P> Consumer<'a, P>
where
    P: Pointer,
    P::Pointee: Node<Link>,
{
    pub fn pop(&mut self) -> Option<P> {
        Some(self.pop_acquired()?.release())
    }

    pub fn pop_acquired(&mut self) -> Option<Acquired<P, Link>> {
        unsafe {
            let link = self.queue.pop()?;
            let acq = Acquired::from_link_unchecked(link);
            Some(acq)
        }
    }
}

impl<'a, P> Drop for Consumer<'a, P> {
    fn drop(&mut self) {
        self.consuming.store(false, Ordering::Release);
    }
}

pub struct UnsafeMpscQueue {
    head: Atomic<NonNull<Link>>,
    tail: Atomic<NonNull<Link>>,
    stub: NonNull<Link>,
}

impl UnsafeMpscQueue {
    pub const unsafe fn with_stub(stub: NonNull<Link>) -> Self {
        Self {
            head: Atomic::new(stub),
            tail: Atomic::new(stub),
            stub,
        }
    }

    pub unsafe fn push(&self, link: NonNull<Link>) {
        link.as_ref().next.store(None, Ordering::Relaxed);
        let prev = self.head.swap(link, Ordering::AcqRel);
        prev.as_ref().next.store(Some(link), Ordering::Release);
    }

    pub unsafe fn pop(&self) -> Option<NonNull<Link>> {
        let mut tail = self.tail.load(Ordering::Relaxed);
        let mut next = tail.as_ref().next.load(Ordering::Acquire);

        if tail == self.stub {
            if let Some(n) = next {
                self.tail.store(n, Ordering::Release);
                tail = n;
                next = n.as_ref().next.load(Ordering::Acquire);
            } else {
                return None;
            }
        }

        if let Some(n) = next {
            self.tail.store(n, Ordering::Release);
            return Some(tail);
        }

        let head = self.head.load(Ordering::Acquire);
        if tail != head {
            return None;
        }

        self.push(self.stub);
        next = tail.as_ref().next.load(Ordering::Acquire);
        let next = next?;
        self.tail.store(next, Ordering::Release);
        Some(tail)
    }
}

#[repr(transparent)]
#[derive(Debug, Default)]
pub struct Link {
    next: Atomic<Option<NonNull<Link>>>,
}

impl Link {
    pub const fn new() -> Self {
        Self {
            next: Atomic::new(None),
        }
    }
}

// impl<P> Node<Link> for P
// where
//     P: Node<DynLink<1>>,
// {
//     fn into_link(this: Self) -> NonNull<Link> {
//         <P as Node<DynLink<1>>>::into_link(this).cast()
//     }

//     unsafe fn from_link(link: NonNull<Link>) -> Self {
//         unsafe { <P as Node<DynLink<1>>>::from_link(link.cast()) }
//     }
// }
