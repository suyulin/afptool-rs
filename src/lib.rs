use std::mem;
mod pack;
mod unpack;

pub use pack::{pack_rkfw, pack_rkaf, chip_name_to_code, encode_chip_field};
pub use unpack::{unpack_file, decode_chip_field};

/// Recover a true (possibly >4 GiB) length from an on-disk u32 field that
/// only stores the low 32 bits. `available` is an upper bound derived from
/// the actual file layout (e.g. distance to the next partition or to the end
/// of the container); the true length is the largest value congruent to
/// `stored` modulo 2^32 that still fits within it.
pub fn recover_true_size(stored: u32, available: u64) -> u64 {
    let stored = stored as u64;
    if available <= stored {
        return stored;
    }
    stored + (((available - stored) >> 32) << 32)
}

pub const RKAFP_MAGIC: &str = "RKAF";
pub const PARM_MAGIC: &str = "PARM";
pub const MAX_PARTS: usize = 16;
pub const MAX_NAME_LEN: usize = 32;
const MAX_FULL_PATH_LEN: usize = 60;
const MAX_MODEL_LEN: usize = 34;
const MAX_ID_LEN: usize = 30;
const MAX_MANUFACTURER_LEN: usize = 56;
pub const RKAF_SIGNATURE: &[u8] = b"RKAF";
pub const RKFW_SIGNATURE: &[u8] = b"RKFW";
pub const RKFP_SIGNATURE: &[u8] = b"RKFP";

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct UpdatePart {
    pub name: [u8; MAX_NAME_LEN],
    pub full_path: [u8; MAX_FULL_PATH_LEN],
    pub flash_size: u32,
    pub part_offset: u32,
    pub flash_offset: u32,
    pub padded_size: u32,
    pub part_byte_count: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct UpdateHeader {
    pub magic: [u8; 4],
    pub length: u32,
    pub model: [u8; MAX_MODEL_LEN],
    pub id: [u8; MAX_ID_LEN],
    pub manufacturer: [u8; MAX_MANUFACTURER_LEN],
    pub unknown1: u32,
    pub version: u32,
    pub num_parts: u32,
    pub parts: [UpdatePart; MAX_PARTS],
    reserved: [u8; 116],
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct ParamHeader {
    magic: [u8; 4],
    length: u32,
}

impl Default for UpdateHeader {
    fn default() -> Self {
        Self {
            magic: [0u8; 4],
            length: 0,
            model: [0u8; MAX_MODEL_LEN],
            id: [0u8; MAX_ID_LEN],
            manufacturer: [0u8; MAX_MANUFACTURER_LEN],
            unknown1: 0,
            version: 0,
            num_parts: 0,
            parts: [UpdatePart::default(); MAX_PARTS],
            reserved: [0u8; 116],
        }
    }
}

impl UpdateHeader {
    pub fn from_bytes(bytes: &[u8]) -> &UpdateHeader {
        unsafe { &*(bytes.as_ptr() as *const UpdateHeader) }
    }

    pub fn to_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<UpdateHeader>()) }
    }
}

impl Default for UpdatePart {
    fn default() -> Self {
        Self {
            name: [0u8; MAX_NAME_LEN],
            full_path: [0u8; MAX_FULL_PATH_LEN],
            flash_size: 0,
            part_offset: 0,
            flash_offset: 0,
            padded_size: 0,
            part_byte_count: 0,
        }
    }
}

pub fn info_and_fatal(is_fatal: bool, message: String) {
    if is_fatal {
        eprint!("rkunpack: fatal: ");
    } else {
        eprint!("rkunpack: info: ");
    }
    eprintln!("{}", message);
    if is_fatal {
        std::process::exit(1);
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        info_and_fatal(false, format!($($arg)*));
    };
}

#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => {
        info_and_fatal(true, format!($($arg)*));
    };
}

/// # Safety
///
/// The returned slice exposes the raw bytes of `p`, including any padding
/// bytes, which may be uninitialized. Only call this on `#[repr(C, packed)]`
/// types with no padding (such as the header structs in this crate).
pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    core::slice::from_raw_parts(
        (p as *const T) as *const u8,
        mem::size_of::<T>(),
    )
}
