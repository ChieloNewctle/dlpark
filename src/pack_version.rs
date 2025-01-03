use crate::ffi::{DLPACK_MAJOR_VERSION, DLPACK_MINOR_VERSION, PackVersion};

impl Default for PackVersion {
    fn default() -> Self {
        Self {
            major: DLPACK_MAJOR_VERSION,
            minor: DLPACK_MINOR_VERSION,
        }
    }
}
