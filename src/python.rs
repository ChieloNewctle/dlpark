use std::ptr::NonNull;

use pyo3::{
    IntoPyObject, Python,
    ffi::{
        PyCapsule_GetPointer, PyCapsule_New, PyCapsule_SetName, PyErr_GetRaisedException,
        PyErr_Occurred, PyErr_SetRaisedException,
    },
    prelude::*,
};

use crate::{
    ffi,
    manager_ctx::ManagerCtx,
    tensor::{
        ManagedTensor,
        traits::{IntoDLPack, ToTensor},
    },
};

/// The producer must set the PyCapsule name to "dltensor" so that it can be
/// inspected by name, and set PyCapsule_Destructor that calls the deleter of
/// the ffi::DLManagedTensor when the "dltensor"-named capsule is no longer
/// needed.
const DLPACK_CAPSULE_NAME: &[u8] = b"dltensor\0";

/// The consumer must transer ownership of the DLManangedTensor from the capsule
/// to its own object. It does so by renaming the capsule to "used_dltensor"
/// to ensure that PyCapsule_Destructor will not get called
/// (ensured if PyCapsule_Destructor calls deleter only for capsules whose name
/// is "dltensor")
const DLPACK_CAPSULE_USED_NAME: &[u8] = b"used_dltensor\0";

fn dlpack_to_py_capsule(dlpack: NonNull<ffi::DLManagedTensor>) -> *mut pyo3::ffi::PyObject {
    unsafe {
        PyCapsule_New(
            dlpack.as_ptr().cast(),
            DLPACK_CAPSULE_NAME.as_ptr().cast(),
            Some(dlpack_capsule_deleter),
        )
    }
}

fn py_capsule_to_dlpack(capsule: *mut pyo3::ffi::PyObject) -> NonNull<ffi::DLManagedTensor> {
    unsafe {
        let ptr = PyCapsule_GetPointer(capsule, DLPACK_CAPSULE_NAME.as_ptr().cast()).cast();
        PyCapsule_SetName(capsule, DLPACK_CAPSULE_USED_NAME.as_ptr().cast());
        NonNull::new_unchecked(ptr)
    }
}

/// Refer to [dlpack python_spec](https://dmlc.github.io/dlpack/latest/python_spec.html#implementation)
unsafe extern "C" fn dlpack_capsule_deleter(capsule: *mut pyo3::ffi::PyObject) {
    if pyo3::ffi::PyCapsule_IsValid(capsule, DLPACK_CAPSULE_USED_NAME.as_ptr() as *const _) == 1 {
        return;
    }

    let exc = PyErr_GetRaisedException();

    let managed = PyCapsule_GetPointer(capsule, DLPACK_CAPSULE_NAME.as_ptr() as *const _)
        as *mut ffi::DLManagedTensor;

    if managed.is_null() {
        pyo3::ffi::PyErr_WriteUnraisable(capsule);
        PyErr_SetRaisedException(exc);
        return;
    }

    if let Some(del_fn) = (*managed).deleter {
        del_fn(managed);
        assert!(PyErr_Occurred().is_null());
    }

    PyErr_SetRaisedException(exc);
}

impl<'py, T> IntoPyObject<'py> for ManagerCtx<T>
where
    T: ToTensor,
{
    type Target = PyAny; // the Python type
    type Output = Bound<'py, Self::Target>; // in most cases this will be `Bound`
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let dlpack = self.into_dlpack();
        let capsule = dlpack_to_py_capsule(dlpack);
        Ok(unsafe { PyObject::from_owned_ptr(py, capsule) }.into_bound(py))
    }
}

impl ManagedTensor {
    /// Check this [pytorch src](https://github.com/pytorch/pytorch/blob/main/torch/csrc/utils/tensor_new.cpp#L1583)
    /// # Safety
    /// We use pyo3 ffi here.
    pub fn from_py_ptr(capsule: *mut pyo3::ffi::PyObject) -> Self {
        Self::new(py_capsule_to_dlpack(capsule))
    }
}

impl<'source> FromPyObject<'source> for ManagedTensor {
    fn extract_bound(ob: &Bound<'source, PyAny>) -> PyResult<Self> {
        Ok(ManagedTensor::from_py_ptr(ob.as_ptr()))
    }
}

impl<'py> IntoPyObject<'py> for ManagedTensor {
    type Target = PyAny; // the Python type
    type Output = Bound<'py, Self::Target>; // in most cases this will be `Bound`
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let capsule = dlpack_to_py_capsule(self.into_inner());
        Ok(unsafe { PyObject::from_owned_ptr(py, capsule) }.into_bound(py))
    }
}
