use anyhow::{bail, Result};
mod pack;
mod unpack;

pub use pack::{chip_name_to_code, encode_chip_field, pack_rkaf, pack_rkfw};
pub use unpack::{decode_chip_field, unpack_file};

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
pub const UPDATE_HEADER_SIZE: usize = 2048;
const UPDATE_PART_SIZE: usize = 112;
const MAX_FULL_PATH_LEN: usize = 60;
const MAX_MODEL_LEN: usize = 34;
const MAX_ID_LEN: usize = 30;
const MAX_MANUFACTURER_LEN: usize = 56;
pub const RKAF_SIGNATURE: &[u8] = b"RKAF";
pub const RKFW_SIGNATURE: &[u8] = b"RKFW";
pub const RKFP_SIGNATURE: &[u8] = b"RKFP";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UpdatePart {
    pub name: [u8; MAX_NAME_LEN],
    pub full_path: [u8; MAX_FULL_PATH_LEN],
    pub flash_size: u32,
    pub part_offset: u32,
    pub flash_offset: u32,
    pub padded_size: u32,
    pub part_byte_count: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
    /// Decode an RKAF header from its fixed-size little-endian on-disk format.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < UPDATE_HEADER_SIZE {
            bail!(
                "RKAF header requires at least {UPDATE_HEADER_SIZE} bytes, got {}",
                bytes.len()
            );
        }

        let bytes = &bytes[..UPDATE_HEADER_SIZE];
        let mut header = Self::default();
        header.magic.copy_from_slice(&bytes[0..4]);
        header.length = read_u32_le(bytes, 4);
        header.model.copy_from_slice(&bytes[8..42]);
        header.id.copy_from_slice(&bytes[42..72]);
        header.manufacturer.copy_from_slice(&bytes[72..128]);
        header.unknown1 = read_u32_le(bytes, 128);
        header.version = read_u32_le(bytes, 132);
        header.num_parts = read_u32_le(bytes, 136);

        if header.num_parts as usize > MAX_PARTS {
            bail!(
                "RKAF header contains {} partitions, maximum is {MAX_PARTS}",
                header.num_parts
            );
        }

        for (index, part) in header.parts.iter_mut().enumerate() {
            let start = 140 + index * UPDATE_PART_SIZE;
            *part = UpdatePart::decode(&bytes[start..start + UPDATE_PART_SIZE]);
        }
        header
            .reserved
            .copy_from_slice(&bytes[1932..UPDATE_HEADER_SIZE]);

        Ok(header)
    }

    /// Encode this RKAF header into its fixed-size little-endian on-disk format.
    pub fn encode(&self) -> Result<[u8; UPDATE_HEADER_SIZE]> {
        if self.num_parts as usize > MAX_PARTS {
            bail!(
                "cannot encode {} RKAF partitions, maximum is {MAX_PARTS}",
                self.num_parts
            );
        }

        let mut bytes = [0u8; UPDATE_HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        write_u32_le(&mut bytes, 4, self.length);
        bytes[8..42].copy_from_slice(&self.model);
        bytes[42..72].copy_from_slice(&self.id);
        bytes[72..128].copy_from_slice(&self.manufacturer);
        write_u32_le(&mut bytes, 128, self.unknown1);
        write_u32_le(&mut bytes, 132, self.version);
        write_u32_le(&mut bytes, 136, self.num_parts);

        for (index, part) in self.parts.iter().enumerate() {
            let start = 140 + index * UPDATE_PART_SIZE;
            part.encode_into(&mut bytes[start..start + UPDATE_PART_SIZE]);
        }
        bytes[1932..UPDATE_HEADER_SIZE].copy_from_slice(&self.reserved);

        Ok(bytes)
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

impl UpdatePart {
    fn decode(bytes: &[u8]) -> Self {
        let mut part = Self::default();
        part.name.copy_from_slice(&bytes[0..32]);
        part.full_path.copy_from_slice(&bytes[32..92]);
        part.flash_size = read_u32_le(bytes, 92);
        part.part_offset = read_u32_le(bytes, 96);
        part.flash_offset = read_u32_le(bytes, 100);
        part.padded_size = read_u32_le(bytes, 104);
        part.part_byte_count = read_u32_le(bytes, 108);
        part
    }

    fn encode_into(&self, bytes: &mut [u8]) {
        bytes[0..32].copy_from_slice(&self.name);
        bytes[32..92].copy_from_slice(&self.full_path);
        write_u32_le(bytes, 92, self.flash_size);
        write_u32_le(bytes, 96, self.part_offset);
        write_u32_le(bytes, 100, self.flash_offset);
        write_u32_le(bytes, 104, self.padded_size);
        write_u32_le(bytes, 108, self.part_byte_count);
    }
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
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
