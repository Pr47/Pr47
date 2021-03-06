use std::future::Future;
use std::pin::Pin;

use crate::data::Value;
use crate::data::exception::Exception;
use crate::data::traits::StaticBase;
use crate::data::wrapper::OwnershipInfo;
use crate::ffi::Signature;
use crate::util::serializer::Serializer;
use crate::util::void::Void;

pub enum AsyncOwnInfoGuard {
    DoNothing,
    SetOwnInfo(Value, OwnershipInfo)
}

pub trait AsyncVMContext: 'static + Sized + Send + Sync {
    type SharedData;

    fn serializer(&self) -> &Serializer<Self::SharedData>;
}

pub type AsyncReturnType = Result<Box<[Value]>, Exception>;

pub struct Promise {
    pub fut: Pin<Box<dyn Future<Output = AsyncReturnType> + Send + 'static>>,
    pub guards: Box<[AsyncOwnInfoGuard]>
}

// TODO should we make it a `StaticBase`?
impl StaticBase<Promise> for Void {}

pub trait AsyncFunction: 'static {
    fn signature(&self) -> Signature;

    fn call_tyck(
        &self,
        context: &impl AsyncVMContext,
        args: &[Value]
    ) -> Promise;

    unsafe fn call_rtlc(
        &self,
        context: &impl AsyncVMContext,
        args: &[Value]
    ) -> Promise;
}

