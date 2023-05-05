use pin_project::{pin_project, pinned_drop};
use std::{
    ffi::c_void,
    marker::{PhantomData, PhantomPinned},
    mem::transmute,
    pin::Pin,
    ptr::{self, NonNull},
    slice,
};

use crate::dlpack::{DLManagedTensor, DLTensor, Device};

#[derive(Debug)]
pub struct Tensor<'a> {
    pub inner: DLTensor,
    _marker: PhantomData<fn(&'a ()) -> &'a ()>,
}

impl<'a> From<DLTensor> for Tensor<'a> {
    fn from(value: DLTensor) -> Self {
        Self {
            inner: value,
            _marker: PhantomData,
        }
    }
}

impl<'a> From<Tensor<'a>> for DLTensor {
    fn from(value: Tensor<'a>) -> Self {
        value.inner
    }
}

impl<'a> Tensor<'a> {
    pub fn data(&self) -> *mut c_void {
        self.inner.data
    }

    pub fn shape(&self) -> &[i64] {
        unsafe { slice::from_raw_parts(self.inner.shape, self.ndim()) }
    }

    pub fn strides(&self) -> &[i64] {
        unsafe { slice::from_raw_parts(self.inner.strides, self.ndim()) }
    }

    pub fn ndim(&self) -> usize {
        self.inner.ndim as usize
    }

    pub fn byte_offset(&self) -> u64 {
        self.inner.byte_offset
    }

    pub fn into_inner(self) -> DLTensor {
        self.inner
    }

    pub fn into_ptr(self) -> *const DLTensor {
        &self.inner as *const _
    }

    pub unsafe fn from_raw(ptr: *mut DLTensor) -> Self {
        debug_assert!(!ptr.is_null());
        Self {
            inner: *ptr,
            _marker: PhantomData,
        }
    }

    pub fn device(&self) -> Device {
        self.inner.device
    }
}

// pub struct TensorMaker {

//     shape: Vec<i64>,
//     strides: Option<Vec<i64>>,
//     dtype: DataType,
// }

// impl TensorMaker {
//     pub fn new(data: *mut c_void, shape: &[i64]) -> Self {
//         Self {
//             shape: shape.to_vec(),
//             strides: None,
//             dtype: DataType::F32,
//         }
//     }

//     pub fn shape(&mut self) -> &mut Self {}
// }

pub struct ManagerContext<T> {
    pub ptr: Option<NonNull<*mut c_void>>,
    _type: PhantomData<T>,
    _pin: PhantomPinned,
}

impl<T> ManagerContext<T> {
    pub fn new(ptr: Option<NonNull<*mut c_void>>) -> Self {
        Self {
            ptr,
            _type: PhantomData,
            _pin: PhantomPinned,
        }
    }
}

#[pin_project(PinnedDrop)]
pub struct ManagedTensorProxy<T> {
    pub dl_tensor: DLTensor,
    #[pin]
    pub manager_ctx: ManagerContext<T>,
    pub deleter: Option<fn(&mut ManagedTensor<T>)>,
}

impl<T> From<DLManagedTensor> for ManagedTensorProxy<T> {
    fn from(mut value: DLManagedTensor) -> Self {
        let ptr = if value.manager_ctx.is_null() {
            None
        } else {
            Some(unsafe { NonNull::new_unchecked(&mut value.manager_ctx as *mut _) })
        };

        let manager_ctx = ManagerContext::new(ptr);
        let deleter = value.deleter.take().map(|del_fn| unsafe {
            transmute::<unsafe extern "C" fn(*mut DLManagedTensor), fn(&mut ManagedTensor<T>)>(
                del_fn,
            )
        });

        Self {
            dl_tensor: value.dl_tensor,
            manager_ctx,
            deleter,
        }
    }
}

impl<T> From<ManagedTensorProxy<T>> for DLManagedTensor {
    fn from(value: ManagedTensorProxy<T>) -> Self {
        let dl_tensor = value.dl_tensor;
        let manager_ctx = match value.manager_ctx.ptr {
            None => ptr::null_mut(),
            Some(non_null) => unsafe { *non_null.as_ptr() },
        };

        let deleter = value.deleter.map(|del_fn| unsafe {
            transmute::<fn(&mut ManagedTensor<T>), unsafe extern "C" fn(*mut DLManagedTensor)>(
                del_fn,
            )
        });
        DLManagedTensor {
            dl_tensor,
            manager_ctx,
            deleter,
        }
    }
}

impl<T> From<Pin<&mut ManagedTensorProxy<T>>> for DLManagedTensor {
    fn from(value: Pin<&mut ManagedTensorProxy<T>>) -> Self {
        let dl_tensor = value.dl_tensor;
        let manager_ctx = match value.manager_ctx.ptr {
            None => ptr::null_mut(),
            Some(non_null) => unsafe { *non_null.as_ptr() },
        };

        let deleter = value.deleter.map(|del_fn| unsafe {
            transmute::<fn(&mut ManagedTensor<T>), unsafe extern "C" fn(*mut DLManagedTensor)>(
                del_fn,
            )
        });
        DLManagedTensor {
            dl_tensor,
            manager_ctx,
            deleter,
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for ManagedTensorProxy<T> {
    fn drop(mut self: Pin<&mut Self>) {
        let mut dl_managed_tensor: DLManagedTensor = self.as_mut().into();
        if let Some(del_fn) = self.deleter {
            let c_del_fn =
                unsafe { transmute::<fn(&mut ManagedTensor<T>), fn(*mut DLManagedTensor)>(del_fn) };
            c_del_fn(&mut dl_managed_tensor as *mut _);
        }
    }
}

pub struct ManagedTensor<'a, T: 'a> {
    pub proxy: ManagedTensorProxy<T>,
    _marker: PhantomData<fn(&'a ()) -> &'a ()>, // invariant wrt 'tensor
}

impl<'a, T> From<DLManagedTensor> for ManagedTensor<'a, T> {
    fn from(value: DLManagedTensor) -> Self {
        let proxy = value.into();
        Self {
            proxy,
            _marker: PhantomData,
        }
    }
}

impl<'a, T> From<ManagedTensor<'a, T>> for DLManagedTensor {
    fn from(value: ManagedTensor<'a, T>) -> Self {
        value.proxy.into()
    }
}

impl<'a, T: 'a> ManagedTensor<'a, T> {
    pub fn new(tensor_ref: Tensor<'a>, manager_ctx: Option<NonNull<*mut c_void>>) -> Self {
        let manager_ctx = ManagerContext::new(manager_ctx);
        let proxy = ManagedTensorProxy {
            dl_tensor: tensor_ref.into_inner(),
            manager_ctx,
            deleter: None,
        };

        Self {
            proxy,
            _marker: PhantomData,
        }
    }
}
