use afptool_rs::{UpdateHeader, UpdatePart, MAX_PARTS, RKAF_SIGNATURE, UPDATE_HEADER_SIZE};

const PARTS_OFFSET: usize = 140;
const PART_SIZE: usize = 112;

#[test]
fn decodes_little_endian_numeric_fields() {
    let mut bytes = [0u8; UPDATE_HEADER_SIZE];
    bytes[0..4].copy_from_slice(RKAF_SIGNATURE);
    bytes[4..8].copy_from_slice(&[0x78, 0x56, 0x34, 0x12]);
    bytes[128..132].copy_from_slice(&[0x04, 0x03, 0x02, 0x01]);
    bytes[132..136].copy_from_slice(&[0x44, 0x33, 0x22, 0x11]);
    bytes[136..140].copy_from_slice(&1u32.to_le_bytes());

    let numeric = PARTS_OFFSET + 92;
    bytes[numeric..numeric + 4].copy_from_slice(&[0xef, 0xcd, 0xab, 0x89]);
    bytes[numeric + 4..numeric + 8].copy_from_slice(&[0x10, 0x32, 0x54, 0x76]);
    bytes[numeric + 8..numeric + 12].copy_from_slice(&[0x98, 0xba, 0xdc, 0xfe]);
    bytes[numeric + 12..numeric + 16].copy_from_slice(&[0x0d, 0x0c, 0x0b, 0x0a]);
    bytes[numeric + 16..numeric + 20].copy_from_slice(&[0x40, 0x30, 0x20, 0x10]);

    let header = UpdateHeader::decode(&bytes).unwrap();
    assert_eq!(header.length, 0x1234_5678);
    assert_eq!(header.unknown1, 0x0102_0304);
    assert_eq!(header.version, 0x1122_3344);
    assert_eq!(header.num_parts, 1);
    assert_eq!(header.parts[0].flash_size, 0x89ab_cdef);
    assert_eq!(header.parts[0].part_offset, 0x7654_3210);
    assert_eq!(header.parts[0].flash_offset, 0xfedc_ba98);
    assert_eq!(header.parts[0].padded_size, 0x0a0b_0c0d);
    assert_eq!(header.parts[0].part_byte_count, 0x1020_3040);
}

#[test]
fn encodes_little_endian_numeric_fields() {
    let mut header = UpdateHeader::default();
    header.magic = *b"RKAF";
    header.length = 0x1234_5678;
    header.unknown1 = 0x0102_0304;
    header.version = 0x1122_3344;
    header.num_parts = 1;
    header.parts[0] = UpdatePart {
        flash_size: 0x89ab_cdef,
        part_offset: 0x7654_3210,
        flash_offset: 0xfedc_ba98,
        padded_size: 0x0a0b_0c0d,
        part_byte_count: 0x1020_3040,
        ..Default::default()
    };

    let bytes = header.encode().unwrap();
    assert_eq!(&bytes[4..8], &[0x78, 0x56, 0x34, 0x12]);
    assert_eq!(&bytes[128..132], &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(&bytes[132..136], &[0x44, 0x33, 0x22, 0x11]);
    assert_eq!(&bytes[136..140], &[1, 0, 0, 0]);

    let numeric = PARTS_OFFSET + 92;
    assert_eq!(&bytes[numeric..numeric + 4], &[0xef, 0xcd, 0xab, 0x89]);
    assert_eq!(&bytes[numeric + 4..numeric + 8], &[0x10, 0x32, 0x54, 0x76]);
    assert_eq!(&bytes[numeric + 8..numeric + 12], &[0x98, 0xba, 0xdc, 0xfe]);
    assert_eq!(
        &bytes[numeric + 12..numeric + 16],
        &[0x0d, 0x0c, 0x0b, 0x0a]
    );
    assert_eq!(
        &bytes[numeric + 16..numeric + 20],
        &[0x40, 0x30, 0x20, 0x10]
    );
}

#[test]
fn decode_encode_preserves_every_header_byte() {
    let mut bytes = std::array::from_fn(|index| (index as u8).wrapping_mul(37));
    bytes[136..140].copy_from_slice(&(MAX_PARTS as u32).to_le_bytes());

    let header = UpdateHeader::decode(&bytes).unwrap();
    assert_eq!(header.encode().unwrap(), bytes);
}

#[test]
fn rejects_short_headers_and_invalid_partition_counts() {
    let short = [0u8; UPDATE_HEADER_SIZE - 1];
    let error = UpdateHeader::decode(&short).unwrap_err().to_string();
    assert!(error.contains("requires at least 2048 bytes"));

    let mut bytes = [0u8; UPDATE_HEADER_SIZE];
    bytes[136..140].copy_from_slice(&((MAX_PARTS + 1) as u32).to_le_bytes());
    let error = UpdateHeader::decode(&bytes).unwrap_err().to_string();
    assert!(error.contains("maximum is 16"));

    let mut header = UpdateHeader::default();
    header.num_parts = (MAX_PARTS + 1) as u32;
    let error = header.encode().unwrap_err().to_string();
    assert!(error.contains("maximum is 16"));
}

#[test]
fn accepts_trailing_container_bytes_without_serializing_them() {
    let mut bytes = vec![0u8; UPDATE_HEADER_SIZE + PART_SIZE];
    bytes[..4].copy_from_slice(RKAF_SIGNATURE);
    bytes[UPDATE_HEADER_SIZE..].fill(0xa5);

    let header = UpdateHeader::decode(&bytes).unwrap();
    assert_eq!(header.encode().unwrap(), bytes[..UPDATE_HEADER_SIZE]);
}
