use core::marker::PhantomData;

#[derive(Clone, Copy, Default)]
pub(crate) struct Id<'id> {
    marker: PhantomData<*mut &'id ()>,
}
