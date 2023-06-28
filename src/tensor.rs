pub mod impls;
pub mod traits;

use std::ptr::NonNull;

use crate::ffi;

use self::traits::{FromDLPack, TensorView, ToDLPack, ToTensor};
use crate::manager_ctx::ManagerCtx;

/// Safe wrapper for DLManagedTensor
/// Will call deleter when dropped.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct ManagedTensor(NonNull<ffi::DLManagedTensor>);

impl Drop for ManagedTensor {
    fn drop(&mut self) {
        // TODO: we should add a flag for buggy numpy dlpack deleter
        unsafe {
            if let Some(deleter) = self.0.as_ref().deleter {
                deleter(self.0.as_ptr());
            }
        }
    }
}

impl ManagedTensor {
    pub fn new(src: NonNull<ffi::DLManagedTensor>) -> Self {
        Self(src)
    }

    pub fn as_slice<A>(&self) -> &[A] {
        unsafe { std::slice::from_raw_parts(self.data_ptr().cast(), self.num_elements()) }
    }

    /// Get raw pointer.
    pub fn as_ptr(&self) -> *mut ffi::DLManagedTensor {
        self.0.as_ptr()
    }

    /// Get DLPack ptr.
    pub fn into_inner(self) -> NonNull<ffi::DLManagedTensor> {
        self.0
    }

    pub fn dl_tensor(&self) -> &ffi::DLTensor {
        unsafe { &self.0.as_ref().dl_tensor }
    }
}

impl TensorView for ManagedTensor {
    fn data_ptr(&self) -> *mut std::ffi::c_void {
        self.dl_tensor().data_ptr()
    }

    fn byte_offset(&self) -> u64 {
        self.dl_tensor().byte_offset()
    }

    fn device(&self) -> ffi::Device {
        self.dl_tensor().device()
    }

    fn dtype(&self) -> ffi::DataType {
        self.dl_tensor().dtype()
    }

    fn shape(&self) -> &[i64] {
        self.dl_tensor().shape()
    }

    fn strides(&self) -> Option<&[i64]> {
        self.dl_tensor().strides()
    }

    fn ndim(&self) -> usize {
        self.dl_tensor().ndim()
    }
}

impl<T> From<ManagerCtx<T>> for ManagedTensor
where
    T: ToTensor,
{
    fn from(value: ManagerCtx<T>) -> Self {
        Self(value.to_dlpack())
    }
}

impl FromDLPack for ManagedTensor {
    fn from_dlpack(src: NonNull<ffi::DLManagedTensor>) -> Self {
        Self(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn from_vec_f32() {
        let v: Vec<f32> = (0..10).map(|x| x as f32).collect();
        let tensor = ManagerCtx::new(v);
        assert_eq!(tensor.shape(), &[10]);
        assert_eq!(tensor.ndim(), 1);
        assert_eq!(tensor.device(), Device::CPU);
        assert_eq!(tensor.strides(), None);
        assert_eq!(tensor.byte_offset(), 0);
        assert_eq!(tensor.dtype(), DataType::F32);
    }
}
