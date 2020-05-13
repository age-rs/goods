use {
    super::{handle::Handle, Error},
    crate::{
        asset::Asset,
        channel::{channel, Receiver, Sender},
    },
    alloc::{boxed::Box, sync::Arc, vec::Vec},
    core::{any::Any, marker::PhantomData},
};

pub(crate) struct ProcessSlot<A: Asset> {
    handle: Handle<A>,
    sender: Sender<Box<dyn AnyProcess<A::Context> + Send>>,
}

impl<A> ProcessSlot<A>
where
    A: Asset,
{
    pub(crate) fn set(self, result: Result<A::Repr, Error<A>>) {
        self.sender.send(Box::new(Process {
            result,
            handle: self.handle,
        }))
    }
}

pub(crate) trait AnyProcess<C> {
    fn run(self: Box<Self>, ctx: &mut C);
}

struct Process<A: Asset> {
    handle: Handle<A>,
    result: Result<A::Repr, Error<A>>,
}

impl<A> Process<A> where A: Asset {}

impl<A> AnyProcess<A::Context> for Process<A>
where
    A: Asset,
{
    fn run(self: Box<Self>, ctx: &mut A::Context) {
        let result = self
            .result
            .and_then(|asset| A::build(asset, ctx).map_err(|err| Error::Asset(Arc::new(err))));

        self.handle.set(result);
    }
}

struct Processes<C> {
    receiver: Receiver<Box<dyn AnyProcess<C> + Send>>,
    sender: Sender<Box<dyn AnyProcess<C> + Send>>,
}

impl<C> Processes<C> {
    fn new() -> Self {
        let (sender, receiver) = channel();
        Processes { sender, receiver }
    }

    fn run(&mut self) -> Vec<Box<dyn AnyProcess<C> + Send>> {
        self.receiver.recv_batch()
    }
}

pub(crate) struct AnyProcesses<K> {
    inner: Box<dyn Any + Send>,
    marker: PhantomData<fn(K)>,
}

impl<K> AnyProcesses<K>
where
    K: 'static,
{
    pub(crate) fn new<C: 'static>() -> Self {
        AnyProcesses {
            inner: Box::new(Processes::<C>::new()),
            marker: PhantomData,
        }
    }

    pub(crate) fn alloc<A>(&self) -> (Handle<A>, ProcessSlot<A>)
    where
        A: Asset,
    {
        let sender = Any::downcast_ref::<Processes<A::Context>>(&*self.inner)
            .unwrap()
            .sender
            .clone();
        let handle = Handle::new();
        let slot = ProcessSlot {
            handle: handle.clone(),
            sender,
        };
        (handle, slot)
    }

    pub(crate) fn run<C: 'static>(&mut self) -> Vec<Box<dyn AnyProcess<C> + Send>> {
        Any::downcast_mut::<Processes<C>>(&mut *self.inner)
            .unwrap()
            .run()
    }
}