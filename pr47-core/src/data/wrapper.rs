use std::any::TypeId;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::addr_of;

use unchecked_unwrap::UncheckedUnwrap;

use crate::data::traits::StaticBase;
use crate::data::tyck::TyckInfo;
use crate::util::void::Void;
use crate::util::unsafe_from::UnsafeFrom;

pub const GC_MARKED_MASK: u8 = 0b1_00_00000;
pub const GC_INFO_MASK: u8   = 0b0_00_11111;

pub const GC_INFO_READ_MASK: u8   = 0b0_00_1_0_0_0_0;
pub const GC_INFO_WRITE_MASK: u8  = 0b0_00_0_1_0_0_0;
pub const GC_INFO_MOVE_MASK: u8   = 0b0_00_0_0_1_0_0;
pub const GC_INFO_DELETE_MASK: u8 = 0b0_00_0_0_0_1_0;
pub const GC_INFO_OWNED_MASK: u8  = 0b0_00_0_0_0_0_1;

#[repr(u8)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum GcInfo {
    // R = Read
    // W = Write
    // M = Move
    // D = Delete
    // O = Virtual Machine Owned
    //                    M    R W M D O
    Owned             = 0b0_00_1_1_1_1_1,
    SharedFromRust    = 0b0_00_1_0_0_0_0,
    MutSharedFromRust = 0b0_00_1_1_0_0_0,
    SharedToRust      = 0b0_00_1_0_0_0_1,
    MutSharedToRust   = 0b0_00_0_0_0_0_1,
    MovedToRust       = 0b0_00_0_0_0_1_0
}

impl GcInfo {
    #[inline(always)] pub fn is_readable(self) -> bool {
        (self as u8) & GC_INFO_READ_MASK != 0
    }

    #[inline(always)] pub fn is_writeable(self) -> bool {
        (self as u8) & GC_INFO_WRITE_MASK != 0
    }

    #[inline(always)] pub fn is_movable(self) -> bool {
        (self as u8) & GC_INFO_MOVE_MASK != 0
    }

    #[inline(always)] pub fn is_deletable(self) -> bool {
        (self as u8) & GC_INFO_DELETE_MASK != 0
    }

    #[inline(always)] pub fn is_owned(self) -> bool {
        (self as u8) & GC_INFO_OWNED_MASK != 0
    }
}

impl UnsafeFrom<u8> for GcInfo {
    unsafe fn unsafe_from(data: u8) -> Self {
        std::mem::transmute::<u8, Self>(data)
    }
}

#[repr(C)]
pub union WrapperData<T: 'static> {
    pub ptr: *mut T,
    pub owned: ManuallyDrop<MaybeUninit<T>>
}

#[repr(C, align(8))]
pub struct Wrapper<T: 'static> {
    /* +0 */ pub refcount: u32,
    /* +4 */ pub gc_info: u8,
    /* +5 */ pub data_offset: u8,

    pub data: WrapperData<T>
}

impl<T: 'static> Wrapper<T> {
    pub fn new_owned(data: T) -> Self {
        let mut ret: Wrapper<T> = Self {
            refcount: 0,
            gc_info: GcInfo::Owned as u8,
            data_offset: 0,
            data: WrapperData {
                owned: ManuallyDrop::new(MaybeUninit::new(data))
            }
        };
        ret.data_offset = (addr_of!(ret.data) as usize - addr_of!(ret) as usize) as u8;
        ret
    }

    pub fn new_ref(ptr: *const T) -> Self {
        let mut ret: Wrapper<T> = Self {
            refcount: 1,
            gc_info: GcInfo::SharedFromRust as u8,
            data_offset: 0,
            data: WrapperData {
                ptr: ptr as *mut T
            }
        };
        ret.data_offset = (addr_of!(ret.data) as usize - addr_of!(ret) as usize) as u8;
        ret
    }

    pub fn new_mut_ref(ptr: *mut T) -> Self {
        let mut ret: Wrapper<T> = Self {
            refcount: 1,
            gc_info: GcInfo::MutSharedFromRust as u8,
            data_offset: 0,
            data: WrapperData {
                ptr
            }
        };
        ret.data_offset = (addr_of!(ret.data) as usize - addr_of!(ret) as usize) as u8;
        ret
    }
}

pub trait DynBase {
    fn dyn_type_id(&self) -> TypeId;

    fn dyn_type_name(&self) -> String;

    fn dyn_tyck(&self, tyck_info: &TyckInfo) -> bool;

    #[cfg(debug_assertions)]
    unsafe fn move_out_ck(&mut self, out: *mut (), type_id: TypeId);

    #[cfg(not(debug_assertions))]
    unsafe fn move_out(&mut self, out: *mut ());
}

impl<T: 'static> DynBase for Wrapper<T> where Void: StaticBase<T> {
    fn dyn_type_id(&self) -> TypeId {
        <Void as StaticBase<T>>::type_id()
    }

    fn dyn_type_name(&self) -> String {
        <Void as StaticBase<T>>::type_name()
    }

    fn dyn_tyck(&self, tyck_info: &TyckInfo) -> bool {
        <Void as StaticBase<T>>::tyck(tyck_info)
    }

    #[cfg(debug_assertions)]
    unsafe fn move_out_ck(&mut self, out: *mut (), type_id: TypeId) {
        debug_assert_eq!(self.dyn_type_id(), type_id);
        debug_assert!(GcInfo::unsafe_from(self.gc_info).is_movable());
        let dest: &mut MaybeUninit<T> = (out as *mut MaybeUninit<T>).as_mut().unchecked_unwrap();
        *dest.as_mut_ptr() = ManuallyDrop::take(&mut self.data.owned).assume_init();
        self.gc_info = GcInfo::MovedToRust as u8;
    }

    #[cfg(not(debug_assertions))]
    unsafe fn move_out(&mut self, out: *mut ()) {
        let dest: &mut MaybeUninit<T>
            = (out as *mut MaybeUninit<T>).as_mut().unchecked_unwrap();
        *dest.as_mut_ptr() = ManuallyDrop::take(&mut self.data.owned).assume_init();
        self.gc_info = GcInfo::MovedToRust as u8;
    }
}

#[cfg(test)]
mod test {
    use std::ptr::{addr_of, null_mut};
    use crate::data::wrapper::{Wrapper, WrapperData};

    #[allow(dead_code)]
    struct TestStruct {
        field1: i32,
        field2: i64,
        field3: std::string::String
    }

    #[allow(dead_code)]
    #[repr(align(16))]
    struct TestStruct2();

    #[test]
    fn test_mem_layout() {
        let w: Wrapper<TestStruct> = Wrapper {
            refcount: 42,
            gc_info: 0,
            data_offset: 0,
            data: WrapperData {
                ptr: null_mut()
            }
        };

        assert_eq!(addr_of!(w.refcount) as usize - addr_of!(w) as usize, 0);
        assert_eq!(addr_of!(w.gc_info) as usize - addr_of!(w) as usize, 4);
        assert_eq!(addr_of!(w.data_offset) as usize - addr_of!(w) as usize, 5);

        let w: Wrapper<()> = Wrapper {
            refcount: 42,
            gc_info: 0,
            data_offset: 0,
            data: WrapperData {
                ptr: null_mut()
            }
        };

        assert_eq!(addr_of!(w.refcount) as usize - addr_of!(w) as usize, 0);
        assert_eq!(addr_of!(w.gc_info) as usize - addr_of!(w) as usize, 4);
        assert_eq!(addr_of!(w.data_offset) as usize - addr_of!(w) as usize, 5);

        let w: Wrapper<TestStruct2> = Wrapper {
            refcount: 42,
            gc_info: 0,
            data_offset: 0,
            data: WrapperData {
                ptr: null_mut()
            }
        };

        assert_eq!(addr_of!(w.refcount) as usize - addr_of!(w) as usize, 0);
        assert_eq!(addr_of!(w.gc_info) as usize - addr_of!(w) as usize, 4);
        assert_eq!(addr_of!(w.data_offset) as usize - addr_of!(w) as usize, 5);
    }
}
