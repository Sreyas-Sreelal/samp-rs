use crate::cell::{AmxPrimitive, Buffer, Cell, Ref};
use crate::consts::{AmxExecIdx, AmxFlags};
use crate::error::{AmxError, AmxResult};
use crate::exports::*;
use crate::raw::types::{AMX, AMX_HEADER};

use std::ffi::CString;

macro_rules! amx_try {
    ($call:expr) => {
        let result = $call;

        if result > 0 {
            return Err(result.into());
        }
    };
}

/// A wrapper around a raw pointer to an AMX and exported functions.
#[derive(Debug)]
pub struct Amx {
    ptr: *mut AMX,
    fn_table: usize,
}

impl Amx {
    pub fn new(ptr: *mut AMX, fn_table: usize) -> Amx {
        Amx { ptr, fn_table }
    }

    pub(crate) fn allot<T: Sized + AmxPrimitive>(&self, cells: usize) -> AmxResult<Ref<T>> {
        let allot = Allot::from_table(self.fn_table);

        let mut amx_addr = 0;
        let mut phys_addr = 0;

        amx_try!(allot(self.ptr, cells as i32, &mut amx_addr, &mut phys_addr));

        unsafe { Ok(Ref::new(amx_addr, phys_addr as *mut T)) }
    }

    // TODO: return any type that can be converted to an amx cell
    pub fn exec(&self, index: AmxExecIdx) -> AmxResult<i32> {
        let exec = Exec::from_table(self.fn_table);
        let mut retval = 0;

        amx_try!(exec(self.ptr, &mut retval, index.into()));

        Ok(retval)
    }

    pub fn find_native(&self, name: &str) -> AmxResult<i32> {
        let find_native = FindNative::from_table(self.fn_table);
        let mut index = -1;
        let c_str = CString::new(name).map_err(|_| AmxError::NotFound)?;

        amx_try!(find_native(self.ptr, c_str.as_ptr(), &mut index));

        Ok(index)
    }

    pub fn find_public(&self, name: &str) -> AmxResult<AmxExecIdx> {
        let find_public = FindPublic::from_table(self.fn_table);
        let mut index = -1;
        let c_str = CString::new(name).map_err(|_| AmxError::NotFound)?;

        amx_try!(find_public(self.ptr, c_str.as_ptr(), &mut index));

        Ok(AmxExecIdx::from(index))
    }

    pub fn flags(&self) -> AmxResult<AmxFlags> {
        let flags = Flags::from_table(self.fn_table);
        let mut value: u16 = 0;

        amx_try!(flags(self.ptr, &mut value));

        Ok(AmxFlags::from_bits_truncate(value))
    }

    pub fn get_ref<T: Sized + AmxPrimitive>(&self, address: i32) -> AmxResult<Ref<T>> {
        let amx = self.amx();
        let header = self.header();

        let data = if amx.data.is_null() {
            unsafe { amx.base.offset(header.dat as isize) }
        } else {
            amx.data
        };

        if address >= amx.hea && address < amx.stk || address < 0 || address >= amx.stp {
            return Err(AmxError::MemoryAccess);
        }

        let ptr = unsafe { data.offset(address as isize) };

        unsafe { Ok(Ref::new(address, ptr as *mut T)) }
    }

    #[inline(always)]
    pub(crate) fn release(&self, address: i32) -> AmxResult<()> {
        let amx = self.amx();

        if amx.hea > address {
            amx.hea = address;
        }

        Ok(())
    }

    pub fn push<'a, T: Cell<'a>>(&'a self, value: T) -> AmxResult<()> {
        let push = Push::from_table(self.fn_table);

        amx_try!(push(self.ptr, value.as_cell()));

        return Ok(());
    }

    pub fn allocator(&self) -> Allocator {
        Allocator::new(self)
    }

    pub fn amx(&self) -> &mut AMX {
        unsafe { &mut *self.ptr }
    }

    pub fn header(&self) -> &mut AMX_HEADER {
        unsafe { &mut *((*self.ptr).base as *mut AMX_HEADER) }
    }
}

/// AMX memory allocator (on the heap) that frees captured memory after drop.
pub struct Allocator<'amx> {
    amx: &'amx Amx,
    release_addr: i32,
}

impl<'amx> Allocator<'amx> {
    pub(crate) fn new(amx: &'amx Amx) -> Allocator<'amx> {
        Allocator {
            amx,
            release_addr: amx.amx().hea,
        }
    }

    /// Allocate memory for a primitive value.
    ///
    /// #Example
    /// ```
    /// let allocator = amx.allocator();
    /// ```
    pub fn allot<T: Sized + AmxPrimitive>(&self, init_value: T) -> AmxResult<Ref<T>> {
        let mut cell = self.amx.allot(1)?;
        *cell = init_value;
        return Ok(cell);
    }

    /// Allocate custom sized buffer on the heap.
    pub fn allot_buffer(&self, size: usize) -> AmxResult<Buffer> {
        let buffer = self.amx.allot(size)?;
        return Ok(Buffer::new(buffer, size));
    }

    pub fn allot_array<T>(&self, array: &[T]) -> AmxResult<Buffer>
    where
        T: Cell<'amx> + AmxPrimitive,
    {
        let mut buffer = self.allot_buffer(array.len())?;

        let slice = buffer.as_mut_slice();

        for (idx, item) in array.iter().enumerate() {
            slice[idx] = item.as_cell();
        }

        return Ok(buffer);
    }

    pub fn allot_string(&self, string: &str) -> AmxResult<Buffer> {
        let mut buffer = self.allot_buffer(string.bytes().len())?;
        buffer.set_string(string);
        return Ok(buffer);
    }
}

impl Drop for Allocator<'_> {
    fn drop(&mut self) {
        // AMX::release never fails
        self.amx.release(self.release_addr).unwrap();
    }
}
