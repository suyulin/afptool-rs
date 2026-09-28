use std::fs;

use afptool_rs::{
    rkfw_storage_mask, storage_code, storage_name, unpack_file, SsfwEntry, SsfwHeader, SsfwImage,
    RKFW_HEADER_SIZE, RKFW_STORAGE_OFFSET, SSFW_ENTRY_SIZE, SSFW_HEADER_SIZE, SSFW_SIGNATURE,
};
use tempfile::TempDir;

fn mock_rkfw(size: usize, storage_mask: u32, fill: u8) -> Vec<u8> {
    assert!(size >= RKFW_HEADER_SIZE);
    let mut image = vec![fill; size];
    image[0..4].copy_from_slice(b"RKFW");
    image[4..6].copy_from_slice(&(RKFW_HEADER_SIZE as u16).to_le_bytes());
    image[RKFW_STORAGE_OFFSET..RKFW_STORAGE_OFFSET + 4]
        .copy_from_slice(&storage_mask.to_le_bytes());
    image
}

fn mock_ssfw() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let first = mock_rkfw(128, 0x200, 0xa5);
    let second = mock_rkfw(144, 0x800, 0x5a);
    let first_offset = (SSFW_HEADER_SIZE + 2 * SSFW_ENTRY_SIZE) as u64;
    let second_offset = first_offset + first.len() as u64;

    let header = SsfwHeader {
        merge_version: 0x0200_0000,
        image_count: 2,
        ..SsfwHeader::default()
    };
    let entries = [
        SsfwEntry {
            offset: first_offset,
            size: first.len() as u64,
        },
        SsfwEntry {
            offset: second_offset,
            size: second.len() as u64,
        },
    ];

    let mut image = header.encode().unwrap().to_vec();
    for entry in entries {
        image.extend_from_slice(&entry.encode());
    }
    image.extend_from_slice(&first);
    image.extend_from_slice(&second);
    (image, first, second)
}

#[test]
fn ssfw_header_and_entry_round_trip() {
    let header = SsfwHeader {
        merge_version: 0x0200_0000,
        image_count: 2,
        reserved: [0x5a; 44],
        ..SsfwHeader::default()
    };
    let encoded = header.encode().unwrap();
    assert_eq!(&encoded[0..4], SSFW_SIGNATURE);
    assert_eq!(SsfwHeader::decode(&encoded).unwrap(), header);

    let entry = SsfwEntry {
        offset: 0x60,
        size: 0x71d4_f24a,
    };
    assert_eq!(SsfwEntry::decode(&entry.encode()).unwrap(), entry);
    assert_eq!(entry.end().unwrap(), 0x71d4_f2aa);
}

#[test]
fn storage_mask_maps_to_rockusb_selector() {
    let spinor = mock_rkfw(RKFW_HEADER_SIZE, 0x200, 0);
    let pcie = mock_rkfw(RKFW_HEADER_SIZE, 0x800, 0);

    assert_eq!(rkfw_storage_mask(&spinor).unwrap(), 0x200);
    assert_eq!(storage_code(0x200), Some(9));
    assert_eq!(storage_name(9), "spinor");
    assert_eq!(rkfw_storage_mask(&pcie).unwrap(), 0x800);
    assert_eq!(storage_code(0x800), Some(11));
    assert_eq!(storage_name(11), "pcie");
    assert_eq!(storage_code(0), None);
    assert_eq!(storage_code(0x600), None);
}

#[test]
fn in_memory_parser_exposes_children_without_copying() {
    let (image, first, second) = mock_ssfw();
    let parsed = SsfwImage::new(&image).unwrap();

    assert_eq!(parsed.header().image_count, 2);
    assert_eq!(parsed.entries().len(), 2);
    let children = parsed.children().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(children[0].data, first);
    assert_eq!(children[0].storage_mask, 0x200);
    assert_eq!(children[0].storage_code, Some(9));
    assert_eq!(children[1].data, second);
    assert_eq!(children[1].storage_mask, 0x800);
    assert_eq!(children[1].storage_code, Some(11));
}

#[test]
fn decodes_observed_vendor_ssfw_index() {
    let bytes = [
        0x53, 0x53, 0x46, 0x57, 0x40, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4a, 0xd2, 0x4d,
        0x00, 0x00, 0x00, 0x00, 0x00, 0xaa, 0xd2, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4a, 0xf2,
        0xd4, 0x71, 0x00, 0x00, 0x00, 0x00,
    ];
    let header = SsfwHeader::decode(&bytes).unwrap();
    assert_eq!(header.header_size, 0x40);
    assert_eq!(header.version, 0x10);
    assert_eq!(header.merge_version, 0x0200_0000);
    assert_eq!(header.image_count, 2);
    assert_eq!(SsfwEntry::decode(&bytes[0x40..0x50]).unwrap().offset, 0x60);
    assert_eq!(
        SsfwEntry::decode(&bytes[0x50..0x60]).unwrap(),
        SsfwEntry {
            offset: 0x4dd2aa,
            size: 0x71d4f24a,
        }
    );
}

#[test]
fn unpack_ssfw_extracts_each_rkfw_and_manifest() {
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("merged.img");
    let output = temp.path().join("out");
    let (image, first, second) = mock_ssfw();
    fs::write(&input, image).unwrap();

    unpack_file(input.to_str().unwrap(), output.to_str().unwrap()).unwrap();

    assert_eq!(
        fs::read(output.join("firmware-00-spinor.img")).unwrap(),
        first
    );
    assert_eq!(
        fs::read(output.join("firmware-01-pcie.img")).unwrap(),
        second
    );
    assert_eq!(
        fs::read(output.join("ssfw-header.bin")).unwrap().len(),
        SSFW_HEADER_SIZE
    );

    let manifest = fs::read_to_string(output.join("ssfw-manifest.txt")).unwrap();
    assert!(manifest.contains("0,0x60,0x80,0x00000200,9,firmware-00-spinor.img"));
    assert!(manifest.contains("1,0xe0,0x90,0x00000800,11,firmware-01-pcie.img"));
}

#[test]
fn unpack_ssfw_rejects_child_outside_container() {
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("bad.img");
    let output = temp.path().join("out");
    let (mut image, _, _) = mock_ssfw();

    let impossible_size = (image.len() as u64 + 1).to_le_bytes();
    image[SSFW_HEADER_SIZE + 8..SSFW_HEADER_SIZE + 16].copy_from_slice(&impossible_size);
    fs::write(&input, image).unwrap();

    let error = unpack_file(input.to_str().unwrap(), output.to_str().unwrap()).unwrap_err();
    assert!(error.to_string().contains("exceeds file size"));
}

#[test]
fn unpack_ssfw_rejects_overlapping_children() {
    let temp = TempDir::new().unwrap();
    let input = temp.path().join("overlap.img");
    let output = temp.path().join("out");
    let (mut image, _, _) = mock_ssfw();

    let first_offset = (SSFW_HEADER_SIZE + 2 * SSFW_ENTRY_SIZE) as u64;
    let second_entry_offset = SSFW_HEADER_SIZE + SSFW_ENTRY_SIZE;
    image[second_entry_offset..second_entry_offset + 8]
        .copy_from_slice(&(first_offset + 1).to_le_bytes());
    fs::write(&input, image).unwrap();

    let error = unpack_file(input.to_str().unwrap(), output.to_str().unwrap()).unwrap_err();
    assert!(error.to_string().contains("overlap"));
}
