use alloc::{Allocator, SliceWrapper, SliceWrapperMut};
use brotli::interface::Unfreezable;
use brotli::{InputPair, InputReference, InputReferenceMut, SliceOffset};
use core;

pub use mux::util::*;

#[derive(Copy, Clone, Default, Debug)]
pub struct SlicePlaceholder<T> {
    len: usize,
    ph: core::marker::PhantomData<T>,
}

impl<T> SlicePlaceholder<T> {
    pub fn new(len: usize) -> Self {
        SlicePlaceholder::<T> {
            len,
            ph: core::marker::PhantomData::<T>::default(),
        }
    }
}

impl<T> SliceWrapper<T> for SlicePlaceholder<T> {
    fn slice(&self) -> &[T] {
        &[]
    }
    fn len(&self) -> usize {
        self.len
    }
}

#[derive(Copy, Clone)]
pub struct SliceReference<'a, T: 'a> {
    data: &'a [T],
    start: usize,
    len: usize,
}

impl<'a, T: 'a> SliceReference<'a, T> {
    pub fn new(input: &'a [T], start: usize, len: usize) -> SliceReference<'a, T> {
        SliceReference::<T> {
            data: input.split_at(start).1.split_at(len).0,
            start: start,
            len: len,
        }
    }
    pub fn freeze_dry(&self) -> SliceReference<'static, T> {
        SliceReference::<T> {
            data: &[],
            start: self.start,
            len: self.len,
        }
    }
    pub fn freeze(old: SliceOffset) -> SliceReference<'static, T> {
        SliceReference::<T> {
            data: &[],
            start: old.offset(),
            len: old.len(),
        }
    }
    pub fn thaw(&self, slice: &'a [T]) -> SliceReference<'a, T> {
        SliceReference::<'a, T> {
            data: slice.split_at(self.start).1.split_at(self.len).0,
            start: self.start,
            len: self.len,
        }
    }
}

impl<'a, T> Default for SliceReference<'a, T> {
    fn default() -> SliceReference<'a, T> {
        SliceReference::<T> {
            data: &[],
            start: 0,
            len: 0,
        }
    }
}

impl<'a, T: 'a> SliceWrapper<T> for SliceReference<'a, T> {
    fn slice(&self) -> &[T] {
        self.data
    }
}

impl<'b> Unfreezable for SliceReference<'b, u8> {
    fn thaw<'a>(&self, data: &'a [u8]) -> InputReference<'a> {
        InputReference {
            data: data.split_at(self.start).1.split_at(self.len).0,
            orig_offset: self.start,
        }
    }
    fn thaw_mut<'a>(&self, data: &'a mut [u8]) -> InputReferenceMut<'a> {
        InputReferenceMut {
            data: data.split_at_mut(self.start).1.split_at_mut(self.len).0,
            orig_offset: self.start,
        }
    }
    fn thaw_pair<'a>(
        &self,
        pair: &InputPair<'a>,
    ) -> Result<InputReference<'a>, ()> {
        if self.start >= pair.1.orig_offset {
            return Ok(InputReference {
                data: pair.1
                    .data
                    .split_at(self.start - pair.1.orig_offset)
                    .1
                    .split_at(self.len)
                    .0,
                orig_offset: self.start,
            });
        }
        let offset = self.start - pair.0.orig_offset;
        if offset + self.len as usize <= pair.0.data.len() {
            // overlap
            Ok(InputReference {
                data: pair.0.data.split_at(offset).1.split_at(self.len).0,
                orig_offset: self.start,
            })
        } else {
            Err(())
        }
    }
}

pub struct AllocatedMemoryPrefix<T, AllocT: Allocator<T>>(pub AllocT::AllocatedMemory, pub u32);

impl<T, AllocT: Allocator<T>> core::ops::Index<usize> for AllocatedMemoryPrefix<T, AllocT> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.0.slice()[index]
    }
}

impl<T, AllocT: Allocator<T>> core::ops::IndexMut<usize> for AllocatedMemoryPrefix<T, AllocT> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.mem().slice_mut()[index]
    }
}

impl<T, AllocT: Allocator<T>> Default for AllocatedMemoryPrefix<T, AllocT> {
    fn default() -> Self {
        AllocatedMemoryPrefix(AllocT::AllocatedMemory::default(), 0u32)
    }
}

impl<T, AllocT: Allocator<T>> AllocatedMemoryPrefix<T, AllocT> {
    #[inline(always)]
    pub fn mem(&mut self) -> &mut AllocT::AllocatedMemory {
        &mut self.0
    }
    pub fn components(self) -> (AllocT::AllocatedMemory, usize) {
        (self.0, self.1 as usize)
    }
    #[inline(always)]
    pub fn max_len(&self) -> usize {
        self.0.len()
    }
}

impl<T, AllocT: Allocator<T>> SliceWrapperMut<T> for AllocatedMemoryPrefix<T, AllocT> {
    fn slice_mut(&mut self) -> &mut [T] {
        self.0.slice_mut().split_at_mut(self.1 as usize).0
    }
}

impl<T, AllocT: Allocator<T>> SliceWrapper<T> for AllocatedMemoryPrefix<T, AllocT> {
    fn slice(&self) -> &[T] {
        self.0.slice().split_at(self.1 as usize).0
    }
    fn len(&self) -> usize {
        self.1 as usize
    }
}

impl<T, AllocT: Allocator<T>> AllocatedMemoryPrefix<T, AllocT> {
    pub fn new(alloc_u8: &mut AllocT, len: usize) -> Self {
        AllocatedMemoryPrefix::<T, AllocT>(alloc_u8.alloc_cell(len), len as u32)
    }
    pub fn realloc(mem: AllocT::AllocatedMemory, len: usize) -> Self {
        debug_assert!(
            len <= mem.slice().len(),
            "Must realloc to a smaller size for AllocatedMemoryPrefix"
        );
        AllocatedMemoryPrefix::<T, AllocT>(mem, len as u32)
    }
}
