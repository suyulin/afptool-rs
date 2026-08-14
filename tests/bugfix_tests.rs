// Regression tests for the RK3588 firmware round-trip failures:
//   Bug 1 - chip identity mis-encoded for 4-digit chips (Check Chip Fail)
//   Bug 2 - u32 size/offset wrapping for >4 GiB content (silent truncation)
//   Bug 3 - RKFW header fields not preserved on round-trip
#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::{Seek, SeekFrom, Write};
    use afptool_rs::{
        decode_chip_field, encode_chip_field, pack_rkaf, pack_rkfw, recover_true_size,
        unpack_file,
    };
    use tempfile::TempDir;

    // ---------------- Bug 1: chip field encoding ----------------

    #[test]
    fn chip_field_rk3588_is_ascii_digits_reversed() {
        // Verified by hexdump of a vendor RK3588 image: 0x15..0x19 = '8' '8' '5' '3'
        assert_eq!(encode_chip_field("RK3588").unwrap(), [b'8', b'8', b'5', b'3']);
        assert_eq!(encode_chip_field("RK3588S").unwrap(), [b'8', b'8', b'5', b'3']);
    }

    #[test]
    fn chip_field_rk3562_matches_previous_encoding() {
        // Coincidence case: family code 0x32 == ASCII '2'. The README's chip must
        // round-trip identically after the fix.
        assert_eq!(encode_chip_field("RK3562").unwrap(), [0x32, b'6', b'5', b'3']);
    }

    #[test]
    fn chip_field_legacy_families_keep_family_code() {
        // Legacy chips predate the ASCII scheme; behavior must not change.
        assert_eq!(encode_chip_field("RK3288").unwrap(), [0x80, b'8', b'2', b'3']);
        assert_eq!(encode_chip_field("RK3066").unwrap(), [0x60, b'6', b'0', b'3']);
        assert_eq!(encode_chip_field("RK29XX").unwrap(), [0x50, 0, 0, 0]);
        assert_eq!(encode_chip_field("PX30").unwrap(), [0x30, 0, 0, 0]);
    }

    #[test]
    fn chip_field_decode() {
        assert_eq!(decode_chip_field(b"8853"), Some("RK3588".to_string()));
        assert_eq!(decode_chip_field(&[0x32, b'6', b'5', b'3']), Some("RK3562".to_string()));
        // Legacy family byte with zero padding is not an ASCII-digit field
        assert_eq!(decode_chip_field(&[0x30, 0, 0, 0]), None);
        assert_eq!(decode_chip_field(&[0x80, b'8', b'2', b'3']), None);
    }

    // ---------------- Bug 2: >4 GiB size recovery ----------------

    #[test]
    fn recover_true_size_no_wrap() {
        // Small partition: stored value is already the truth
        assert_eq!(recover_true_size(0x20, 906), 0x20);
        // Bound smaller than stored (corrupt input): trust the stored value
        assert_eq!(recover_true_size(100, 50), 100);
    }

    #[test]
    fn recover_true_size_userdata_from_real_firmware() {
        // Real numbers from the RK3588 vendor image: 4.5 GiB userdata whose
        // part_byte_count wrapped to exactly 512 MiB.
        assert_eq!(recover_true_size(536_870_912, 4_831_838_208), 4_831_838_208);
        // Same, with sector-padding slack above the true size
        assert_eq!(recover_true_size(536_870_912, 4_831_840_000), 4_831_838_208);
    }

    #[test]
    fn recover_true_size_rkfw_update_from_real_firmware() {
        // RKFW header stored 0xED85D004; true embedded update size is 8,279,937,028
        assert_eq!(recover_true_size(0xED85D004, 8_279_937_028), 8_279_937_028);
    }

    // ---------------- helpers ----------------

    /// Builds a minimal but complete RKFW image byte-for-byte, including the
    /// trailing 32-char ASCII MD5, with the vendor-observed unknown bytes at
    /// 0x36..0x39 so header fidelity can be asserted.
    fn build_reference_rkfw(boot: &[u8], update: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; 0x66];
        h[0..4].copy_from_slice(b"RKFW");
        h[0x04] = 0x66;
        // version 8.1.1
        h[6] = 0x01;
        h[8] = 0x01;
        h[9] = 0x08;
        // code 0x02000000 (LE)
        h[0x0d] = 0x02;
        // date 2026-04-29 23:04:41
        h[0x0e] = 0xea;
        h[0x0f] = 0x07;
        h[0x10] = 4;
        h[0x11] = 29;
        h[0x12] = 23;
        h[0x13] = 4;
        h[0x14] = 41;
        // chip RK3588 as ASCII digits reversed
        h[0x15..0x19].copy_from_slice(b"8853");
        // BOOT offset/size
        h[0x19..0x1d].copy_from_slice(&(0x66u32).to_le_bytes());
        h[0x1d..0x21].copy_from_slice(&(boot.len() as u32).to_le_bytes());
        // update offset/size
        h[0x21..0x25].copy_from_slice(&((0x66 + boot.len()) as u32).to_le_bytes());
        h[0x25..0x29].copy_from_slice(&(update.len() as u32).to_le_bytes());
        h[0x2d] = 0x01;
        // unknown-but-observed vendor bytes that must be preserved
        h[0x36..0x39].copy_from_slice(&[0x48, 0x49, 0x01]);

        let mut body = h;
        body.extend_from_slice(boot);
        body.extend_from_slice(update);
        let md5_hex = format!("{:x}", md5::compute(&body));
        body.extend_from_slice(md5_hex.as_bytes());
        body
    }

    fn mock_update_img() -> Vec<u8> {
        let mut update = b"RKAF".to_vec();
        update.extend_from_slice(&[0xAB; 28]);
        update
    }

    // ---------------- Bug 1 end-to-end: pack writes correct chip bytes ----------------

    #[test]
    fn rkfw_pack_writes_rk3588_chip_bytes() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("BOOT"), b"BOOTDATA12345678").unwrap();
        fs::write(dir.path().join("embedded-update.img"), mock_update_img()).unwrap();
        let out = dir.path().join("out.img");

        pack_rkfw(
            dir.path().to_str().unwrap(),
            out.to_str().unwrap(),
            Some("RK3588"),
            Some("1.0.0"),
            Some(1731031994),
            Some("0x02000000"),
        )
        .unwrap();

        let img = fs::read(&out).unwrap();
        assert_eq!(
            &img[0x15..0x19],
            b"8853",
            "chip field must be the four ASCII digits reversed (reads back as 3588)"
        );
    }

    // ---------------- Bug 3: header template preserved on round-trip ----------------

    #[test]
    fn rkfw_roundtrip_is_byte_identical_with_saved_header() {
        let boot = b"BOOTDATA12345678";
        let update = mock_update_img();
        let original = build_reference_rkfw(boot, &update);

        let dir = TempDir::new().unwrap();
        let src = dir.path().join("original.img");
        let unpacked = dir.path().join("unpacked");
        fs::write(&src, &original).unwrap();

        unpack_file(src.to_str().unwrap(), unpacked.to_str().unwrap()).unwrap();

        // unpack must persist the original header for later repacking
        let saved_header = fs::read(unpacked.join("rkfw-header.bin")).unwrap();
        assert_eq!(&saved_header[..], &original[..0x66]);
        assert_eq!(fs::read(unpacked.join("BOOT")).unwrap(), boot);
        assert_eq!(fs::read(unpacked.join("embedded-update.img")).unwrap(), update);

        // Repack with no CLI overrides: everything comes from the template
        let rebuilt_path = dir.path().join("rebuilt.img");
        pack_rkfw(
            unpacked.to_str().unwrap(),
            rebuilt_path.to_str().unwrap(),
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let rebuilt = fs::read(&rebuilt_path).unwrap();
        assert_eq!(
            rebuilt, original,
            "unchanged content must repack to a byte-identical image"
        );
    }

    #[test]
    fn rkfw_pack_cli_flags_override_template() {
        let boot = b"BOOTDATA12345678";
        let update = mock_update_img();
        let original = build_reference_rkfw(boot, &update);

        let dir = TempDir::new().unwrap();
        let src = dir.path().join("original.img");
        let unpacked = dir.path().join("unpacked");
        fs::write(&src, &original).unwrap();
        unpack_file(src.to_str().unwrap(), unpacked.to_str().unwrap()).unwrap();

        let rebuilt_path = dir.path().join("rebuilt.img");
        pack_rkfw(
            unpacked.to_str().unwrap(),
            rebuilt_path.to_str().unwrap(),
            None,
            Some("2.3.4"),
            None,
            None,
        )
        .unwrap();

        let rebuilt = fs::read(&rebuilt_path).unwrap();
        assert_eq!(rebuilt[9], 2, "major version overridden");
        assert_eq!(rebuilt[8], 3, "minor version overridden");
        assert_eq!(rebuilt[6], 4, "build overridden");
        // untouched template fields survive
        assert_eq!(&rebuilt[0x36..0x39], &[0x48, 0x49, 0x01]);
        assert_eq!(&rebuilt[0x15..0x19], b"8853");
    }

    #[test]
    fn rkfw_pack_without_template_requires_flags() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("BOOT"), b"BOOTDATA12345678").unwrap();
        fs::write(dir.path().join("embedded-update.img"), mock_update_img()).unwrap();
        let out = dir.path().join("out.img");

        let result = pack_rkfw(
            dir.path().to_str().unwrap(),
            out.to_str().unwrap(),
            None,
            Some("1.0.0"),
            Some(1731031994),
            Some("0x02000000"),
        );
        assert!(result.is_err(), "--chip is required when no rkfw-header.bin exists");
    }

    // ---------------- RKAF header fidelity (device did not boot) ----------------

    /// Vendor afptool drops RESERVED package-file lines from the header: the
    /// real RK3588 image lists "backup RESERVED" in package-file but its
    /// header has num_parts=9, not 10. A phantom all-zero entry must not be
    /// emitted.
    #[test]
    fn rkaf_reserved_entries_are_dropped_from_header() {
        let dir = TempDir::new().unwrap();
        let input = dir.path();
        fs::write(input.join("parameter.txt"), b"CMDLINE:x\n").unwrap();
        fs::write(input.join("userdata.img"), vec![0xAAu8; 100]).unwrap();
        fs::write(
            input.join("package-file"),
            b"# legacy comment: \xff\xfe\nparameter parameter.txt\nbackup RESERVED\nuserdata userdata.img\n",
        )
        .unwrap();
        fs::write(
            input.join("partition-metadata.txt"),
            "parameter,parameter.txt,0x00004000,0x00000000,0x00000800,0x00000001,0x00000016\n\
             userdata,userdata.img,0xffffffff,0x00a98000,0x00001000,0x00000001,0x00000064\n",
        )
        .unwrap();

        let out = dir.path().join("out.rkaf");
        pack_rkaf(input.to_str().unwrap(), out.to_str().unwrap(), "RK3588", "RK3588").unwrap();

        let img = fs::read(&out).unwrap();
        let num_parts = u32::from_le_bytes(img[136..140].try_into().unwrap());
        assert_eq!(num_parts, 2, "RESERVED entry must not be counted");

        // part slot 1 (140 + 112) must be userdata, not the phantom backup
        let name1 = &img[140 + 112..140 + 112 + 8];
        assert_eq!(name1, b"userdata");
        assert!(
            !img[..2048].windows(8).any(|w| w == b"RESERVED"),
            "no RESERVED entry may appear in the header"
        );
    }

    /// The vendor tool leaves undocumented bytes (e.g. 0x48 0x01) in the tails
    /// of string fields. unpack must save the original RKAF header and pack
    /// must use it as a template so those bytes survive a round-trip.
    #[test]
    fn rkaf_template_preserves_unknown_header_bytes() {
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("in");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("parameter.txt"), b"CMDLINE:x\n").unwrap();
        fs::write(input.join("userdata.img"), vec![0xAAu8; 100]).unwrap();
        fs::write(
            input.join("package-file"),
            "parameter parameter.txt\nuserdata userdata.img\n",
        )
        .unwrap();
        fs::write(
            input.join("partition-metadata.txt"),
            "parameter,parameter.txt,0x00004000,0x00000000,0x00000800,0x00000001,0x00000016\n\
             userdata,userdata.img,0xffffffff,0x00a98000,0x00001000,0x00000001,0x00000064\n",
        )
        .unwrap();

        let img1 = dir.path().join("v1.rkaf");
        pack_rkaf(input.to_str().unwrap(), img1.to_str().unwrap(), "RK3588", "RK3588").unwrap();

        let unpacked = dir.path().join("unpacked");
        unpack_file(img1.to_str().unwrap(), unpacked.to_str().unwrap()).unwrap();

        // unpack must persist the original RKAF header
        let template_path = unpacked.join("rkaf-header.bin");
        let mut template = fs::read(&template_path).expect("rkaf-header.bin must be saved");

        // Inject vendor-style garbage into string-field tails:
        // model field tail (abs offset 0x25) and part 1's full_path tail.
        template[0x25] = 0x48;
        template[0x26] = 0x01;
        let part1_path_tail = 140 + 112 + 32 + 59 - 1; // last byte of part 1 full_path
        template[part1_path_tail] = 0x48;
        fs::write(&template_path, &template).unwrap();

        // package-file is not a partition in this synthetic image; supply it
        fs::write(
            unpacked.join("package-file"),
            "parameter parameter.txt\nuserdata userdata.img\n",
        )
        .unwrap();

        let img2 = dir.path().join("v2.rkaf");
        pack_rkaf(unpacked.to_str().unwrap(), img2.to_str().unwrap(), "RK3588", "RK3588").unwrap();

        let img = fs::read(&img2).unwrap();
        assert_eq!(img[0x25], 0x48, "model-field tail byte must survive repack");
        assert_eq!(img[0x26], 0x01, "model-field tail byte must survive repack");
        assert_eq!(img[part1_path_tail], 0x48, "path-field tail byte must survive repack");
        // sanity: the real strings are still intact and NUL-terminated
        assert_eq!(&img[8..16], b" RK3588\0");
        assert_eq!(&img[140 + 112..140 + 112 + 9], b"userdata\0");
    }

    /// When packing from a saved header template, a MACHINE_ID that has been
    /// removed from parameter.txt must not leak through from the template:
    /// the logical id is cleared (NUL first byte) while tail bytes may remain.
    #[test]
    fn rkaf_template_id_cleared_when_machine_id_removed() {
        const ID_OFFSET: usize = 4 + 4 + 34; // magic + length + model

        let dir = TempDir::new().unwrap();
        let input = dir.path().join("in");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("parameter.txt"), b"MACHINE_ID: 007\nCMDLINE:x\n").unwrap();
        fs::write(input.join("userdata.img"), vec![0xAAu8; 100]).unwrap();
        fs::write(
            input.join("package-file"),
            "parameter parameter.txt\nuserdata userdata.img\n",
        )
        .unwrap();
        fs::write(
            input.join("partition-metadata.txt"),
            "parameter,parameter.txt,0x00004000,0x00000000,0x00000800,0x00000001,0x00000027\n\
             userdata,userdata.img,0xffffffff,0x00a98000,0x00001000,0x00000001,0x00000064\n",
        )
        .unwrap();

        let img1 = dir.path().join("v1.rkaf");
        pack_rkaf(input.to_str().unwrap(), img1.to_str().unwrap(), "RK3588", "RK3588").unwrap();
        assert_eq!(
            &fs::read(&img1).unwrap()[ID_OFFSET..ID_OFFSET + 5],
            b" 007\0",
            "id must be set while MACHINE_ID exists"
        );

        // Unpack (saves rkaf-header.bin with id " 007"), then remove MACHINE_ID
        let unpacked = dir.path().join("unpacked");
        unpack_file(img1.to_str().unwrap(), unpacked.to_str().unwrap()).unwrap();
        fs::write(unpacked.join("parameter.txt"), b"CMDLINE:x\n").unwrap();
        fs::write(
            unpacked.join("package-file"),
            "parameter parameter.txt\nuserdata userdata.img\n",
        )
        .unwrap();

        let img2 = dir.path().join("v2.rkaf");
        pack_rkaf(unpacked.to_str().unwrap(), img2.to_str().unwrap(), "RK3588", "RK3588").unwrap();

        let img = fs::read(&img2).unwrap();
        assert_eq!(
            img[ID_OFFSET], 0,
            "logical id must be cleared when parameter.txt has no MACHINE_ID"
        );
    }

    /// padded_size is stored in 2048-byte sectors and encodes the TRUE
    /// partition length (the vendor's 4.5 GiB userdata carries 0x240000
    /// sectors while part_byte_count wraps). It must be computed from the
    /// actual file being packed — not copied from partition-metadata.txt,
    /// which goes stale the moment a partition is swapped for one of a
    /// different size.
    #[test]
    fn rkaf_padded_size_computed_from_actual_file_not_stale_metadata() {
        let dir = TempDir::new().unwrap();
        let input = dir.path();
        // 5000 bytes -> ceil(5000/2048) = 3 sectors
        fs::write(input.join("userdata.img"), vec![0xAAu8; 5000]).unwrap();
        fs::write(input.join("package-file"), "userdata userdata.img\n").unwrap();
        // metadata claims 1 sector (stale: recorded when the partition was smaller)
        fs::write(
            input.join("partition-metadata.txt"),
            "userdata,userdata.img,0xffffffff,0x00a98000,0x00000800,0x00000001,0x00000400\n",
        )
        .unwrap();

        let out = dir.path().join("out.rkaf");
        pack_rkaf(input.to_str().unwrap(), out.to_str().unwrap(), "RK3588", "RK3588").unwrap();

        let img = fs::read(&out).unwrap();
        // part 0 numeric fields start at 140 + 92; padded_size is the 4th u32
        let padded = u32::from_le_bytes(img[140 + 92 + 12..140 + 92 + 16].try_into().unwrap());
        assert_eq!(padded, 3, "padded_size must reflect the actual file (3 sectors), not stale metadata");
    }

    // ---------------- Bug 2 end-to-end: 5 GiB member round-trip ----------------
    // Expensive (~11 GB of temp disk I/O); run with: cargo test -- --ignored

    #[test]
    #[ignore]
    fn rkaf_5gib_member_roundtrips_losslessly() {
        const FIVE_GIB: u64 = 5 * 1024 * 1024 * 1024;

        let dir = TempDir::new().unwrap();
        let input = dir.path().join("in");
        fs::create_dir_all(&input).unwrap();

        // Sparse 5 GiB file with markers at both ends so truncation is detectable
        let big = input.join("userdata.img");
        let mut f = File::create(&big).unwrap();
        f.write_all(b"HEAD").unwrap();
        f.seek(SeekFrom::Start(FIVE_GIB - 4)).unwrap();
        f.write_all(b"TAIL").unwrap();
        drop(f);
        assert_eq!(fs::metadata(&big).unwrap().len(), FIVE_GIB);

        fs::write(input.join("recovery.img"), b"TAILPART").unwrap();
        fs::write(
            input.join("package-file"),
            "userdata userdata.img\nrecovery recovery.img\n",
        )
        .unwrap();
        // flash_size in sectors (5 GiB / 512 = 0xA00000), padded/byte-count wrapped low-32.
        // The following recovery partition also has a wrapped physical offset.
        let wrapped = (FIVE_GIB & 0xFFFF_FFFF) as u32;
        fs::write(
            input.join("partition-metadata.txt"),
            format!(
                "userdata,userdata.img,{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}\n\
                 recovery,recovery.img,{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}\n",
                0x00A0_0000u32,
                0x0000_8000u32,
                2048u32,
                wrapped,
                wrapped,
                0x0000_1000u32,
                0x0000_4000u32,
                0x4000_0800u32,
                1u32,
                8u32,
            ),
        )
        .unwrap();

        let packed = dir.path().join("update.img");
        pack_rkaf(
            input.to_str().unwrap(),
            packed.to_str().unwrap(),
            "RK3588",
            "RK3588",
        )
        .unwrap();

        // Header sector + 5 GiB data + one padded recovery sector + CRC.
        assert_eq!(
            fs::metadata(&packed).unwrap().len(),
            2048 + FIVE_GIB + 2048 + 4
        );

        let out = dir.path().join("out");
        unpack_file(packed.to_str().unwrap(), out.to_str().unwrap()).unwrap();

        let extracted = out.join("userdata.img");
        assert_eq!(
            fs::metadata(&extracted).unwrap().len(),
            FIVE_GIB,
            "extraction must recover the full 5 GiB, not the wrapped 1 GiB"
        );
        let mut f = File::open(&extracted).unwrap();
        let mut head = [0u8; 4];
        std::io::Read::read_exact(&mut f, &mut head).unwrap();
        assert_eq!(&head, b"HEAD");
        f.seek(SeekFrom::Start(FIVE_GIB - 4)).unwrap();
        let mut tail = [0u8; 4];
        std::io::Read::read_exact(&mut f, &mut tail).unwrap();
        assert_eq!(&tail, b"TAIL");
        assert_eq!(fs::read(out.join("recovery.img")).unwrap(), b"TAILPART");
    }
}
