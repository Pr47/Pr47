#[cfg(debug_assertions)]
use std::any::TypeId;
use std::iter::Iterator;
use std::ptr::NonNull;

use crate::data::tyck::ContainerTyckInfo;
use crate::util::mem::FatPointer;

pub const CONTAINER_MASK: u8 = 0b00000_010;

#[cfg(debug_assertions)]
pub type MoveOutCkFn = unsafe fn(this: *mut (), out: *mut (), type_id: TypeId);
#[cfg(not(debug_assertions))]
pub type MoveOutFn = unsafe fn(this: *mut (), out: *mut ());

pub type ChildrenFn = unsafe fn(this: *const ()) -> Box<dyn Iterator<Item=FatPointer>>;

pub type DropFn = unsafe fn(this: *mut());

pub struct ContainerVT {
    pub tyck_info: NonNull<ContainerTyckInfo>,
    pub type_name: String,
    #[cfg(debug_assertions)]
    pub move_out_fn: MoveOutCkFn,
    #[cfg(not(debug_assertions))]
    pub move_out_fn: MoveOutFn,
    pub children_fn: ChildrenFn,
    pub drop_fn: DropFn
}

impl ContainerVT {
    #[cfg(debug_assertions)]
    pub fn new(
        tyck_info: NonNull<ContainerTyckInfo>,
        type_name: impl ToString,
        move_out_fn: MoveOutCkFn,
        children_fn: ChildrenFn,
        drop_fn: DropFn
    ) -> Self {
        Self {
            tyck_info,
            type_name: type_name.to_string(),
            move_out_fn,
            children_fn,
            drop_fn
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn new(
        tyck_info: NonNull<ContainerTyckInfo>,
        type_name: impl ToString,
        move_out_fn: MoveOutFn,
        children_fn: ChildrenFn,
        drop_fn: DropFn
    ) -> Self {
        Self {
            tyck_info,
            type_name: type_name.to_string(),
            move_out_fn,
            children_fn,
            drop_fn
        }
    }
}

#[derive(Clone, Copy)]
pub struct ContainerPtr {
    pub data_ptr: *mut u8,
    pub vt: *mut ContainerVT
}
