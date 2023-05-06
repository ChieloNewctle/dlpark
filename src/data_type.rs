use crate::dlpack::{DataType, DataTypeCode};

impl From<(DataTypeCode, u8, u16)> for DataType {
    fn from(value: (DataTypeCode, u8, u16)) -> Self {
        Self {
            code: value.0,
            bits: value.1,
            lanes: value.2,
        }
    }
}

impl Default for DataTypeCode {
    fn default() -> Self {
        DataTypeCode::Float
    }
}

impl Default for DataType {
    fn default() -> Self {
        Self {
            code: DataTypeCode::Float,
            bits: 32,
            lanes: 1,
        }
    }
}

impl DataType {
    pub const F32: Self = Self {
        code: DataTypeCode::Float,
        bits: 32,
        lanes: 1,
    };
    pub const U8: Self = Self {
        code: DataTypeCode::UInt,
        bits: 8,
        lanes: 1,
    };
}
