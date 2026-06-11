#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use afptool_rs::{pack_rkaf, unpack_file, RKAF_SIGNATURE, RKFW_SIGNATURE, UpdateHeader, UpdatePart};
    use tempfile::TempDir;

    // 创建模拟的 RKFW 文件用于测试
    fn create_mock_rkfw() -> Vec<u8> {
        let mut data = vec![0u8; 1024];
        
        // 写入 RKFW 签名
        data[0..4].copy_from_slice(RKFW_SIGNATURE);
        
        // 设置版本信息 (8.1.0)
        data[6] = 0;
        data[7] = 0;
        data[8] = 1;
        data[9] = 8;
        
        // 设置芯片类型 (PX30)
        data[0x15] = 0x30;
        
        // 设置引导信息偏移量和大小
        data[0x19] = 0x66;
        data[0x1a] = 0;
        data[0x1b] = 0;
        data[0x1c] = 0;
        
        data[0x1d] = 0x10;
        data[0x1e] = 0;
        data[0x1f] = 0;
        data[0x20] = 0;
        
        // 设置嵌入式更新映像偏移量和大小
        data[0x21] = 0x76;
        data[0x22] = 0;
        data[0x23] = 0;
        data[0x24] = 0;
        
        data[0x25] = 0x20;
        data[0x26] = 0;
        data[0x27] = 0;
        data[0x28] = 0;
        
        // 写入 BOOT 标记
        data[0x66] = b'B';
        data[0x67] = b'O';
        data[0x68] = b'O';
        data[0x69] = b'T';
        
        // 写入 RKAF 标记（模拟嵌入式更新映像）
        data[0x76] = b'R';
        data[0x77] = b'K';
        data[0x78] = b'A';
        data[0x79] = b'F';
        
        data
    }
    
    // 创建模拟的 RKAF 文件用于测试
    fn create_mock_rkaf() -> Vec<u8> {
        let mut data = vec![0u8; 2048];
        
        // 写入 RKAF 签名
        data[0..4].copy_from_slice(RKAF_SIGNATURE);
        
        // 设置长度（文件大小）
        data[4] = 0x00;
        data[5] = 0x08;
        data[6] = 0x00;
        data[7] = 0x00;
        
        // 设置厂商信息 (RK3326)
        let manufacturer = b"RK3326";
        let offset = 4 + 4 + 34 + 30; // magic + length + model + id
        data[offset..offset + manufacturer.len()].copy_from_slice(manufacturer);
        
        // 设置模型信息 (RK3326)
        let model = b"RK3326";
        let offset = 4 + 4; // magic + length
        data[offset..offset + model.len()].copy_from_slice(model);
        
        // 设置分区数量 (0表示没有分区，这样可以避免解包时的错误)
        let num_parts_offset = 4 + 4 + 34 + 30 + 56 + 4 + 4;
        data[num_parts_offset] = 0;
        data[num_parts_offset + 1] = 0;
        data[num_parts_offset + 2] = 0;
        data[num_parts_offset + 3] = 0;
        
        data
    }

    #[test]
    fn test_update_header_from_bytes() {
        let mock_rkaf = create_mock_rkaf();
        let header = UpdateHeader::from_bytes(&mock_rkaf);
        
        assert_eq!(&header.magic, RKAF_SIGNATURE);
        // 使用临时变量避免 packed struct 对齐问题
        let length = header.length;
        assert_eq!(length, 0x800);
        
        // 检查厂商信息
        let manufacturer = b"RK3326\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(&header.manufacturer[..], manufacturer);
        
        // 检查型号信息
        let model = b"RK3326\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(&header.model[..], model);
        
        // 检查分区数量（使用临时变量避免 packed struct 对齐问题）
        let num_parts = header.num_parts;
        assert_eq!(num_parts, 0);
    }
    
    #[test]
    fn test_update_header_to_bytes() {
        let mut header = UpdateHeader::default();
        header.magic.copy_from_slice(RKAF_SIGNATURE);
        header.length = 0x800;
        header.num_parts = 0;
        
        let manufacturer = b"RK3326";
        header.manufacturer[..manufacturer.len()].copy_from_slice(manufacturer);
        
        let model = b"RK3326";
        header.model[..model.len()].copy_from_slice(model);
        
        let bytes = header.to_bytes();
        assert_eq!(&bytes[0..4], RKAF_SIGNATURE);
        
        // 检查长度
        assert_eq!(bytes[4], 0x00);
        assert_eq!(bytes[5], 0x08);
        assert_eq!(bytes[6], 0x00);
        assert_eq!(bytes[7], 0x00);
        
        // 检查分区数量
        let num_parts_offset = 4 + 4 + 34 + 30 + 56 + 4 + 4;
        assert_eq!(bytes[num_parts_offset], 0);
        assert_eq!(bytes[num_parts_offset + 1], 0);
        assert_eq!(bytes[num_parts_offset + 2], 0);
        assert_eq!(bytes[num_parts_offset + 3], 0);
    }

    #[test]
    fn test_create_mock_files() {
        // 创建测试目录
        let test_dir = Path::new("tests/data/temp");
        if !test_dir.exists() {
            fs::create_dir_all(test_dir).unwrap();
        }
        
        // 创建并写入模拟的 RKFW 文件
        let rkfw_data = create_mock_rkfw();
        let rkfw_path = test_dir.join("mock.rkfw");
        let mut file = File::create(&rkfw_path).unwrap();
        file.write_all(&rkfw_data).unwrap();
        
        assert!(rkfw_path.exists());
        assert_eq!(fs::metadata(&rkfw_path).unwrap().len(), rkfw_data.len() as u64);
        
        // 创建并写入模拟的 RKAF 文件
        let rkaf_data = create_mock_rkaf();
        let rkaf_path = test_dir.join("mock.rkaf");
        let mut file = File::create(&rkaf_path).unwrap();
        file.write_all(&rkaf_data).unwrap();
        
        assert!(rkaf_path.exists());
        assert_eq!(fs::metadata(&rkaf_path).unwrap().len(), rkaf_data.len() as u64);
        
        // 清理测试文件
        fs::remove_file(&rkfw_path).unwrap();
        fs::remove_file(&rkaf_path).unwrap();
    }

    #[test]
    fn test_parm_header_stripped_on_extract() {
        use std::mem;

        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("test.rkaf");
        let output_dir = temp_dir.path().join("out");
        fs::create_dir_all(&output_dir).unwrap();

        let content = b"CMDLINE:console=ttyFIQ0,1500000\n";

        // Build PARM-wrapped data: magic + content_len (LE u32) + content + CRC (4 bytes)
        let mut parm_data: Vec<u8> = Vec::new();
        parm_data.extend_from_slice(b"PARM");
        parm_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        parm_data.extend_from_slice(content);
        parm_data.extend_from_slice(&[0u8; 4]); // CRC

        let header_size = mem::size_of::<UpdateHeader>();

        let mut header = UpdateHeader::default();
        header.magic.copy_from_slice(RKAF_SIGNATURE);
        header.num_parts = 1;
        // header.length must equal filesize - 4 (the RKAF trailing CRC)
        header.length = (header_size + parm_data.len()) as u32;

        let mut part = UpdatePart::default();
        part.name[..9].copy_from_slice(b"parameter");
        part.full_path[..13].copy_from_slice(b"parameter.txt");
        part.part_offset = header_size as u32;
        part.part_byte_count = parm_data.len() as u32;
        header.parts[0] = part;

        let mut file_data: Vec<u8> = header.to_bytes().to_vec();
        file_data.extend_from_slice(&parm_data);
        file_data.extend_from_slice(&[0u8; 4]); // RKAF trailing CRC

        fs::write(&input_path, &file_data).unwrap();

        unpack_file(
            input_path.to_str().unwrap(),
            output_dir.to_str().unwrap(),
        ).unwrap();

        let extracted = fs::read(output_dir.join("parameter.txt")).unwrap();
        assert_eq!(extracted, content, "extracted content should match without PARM wrapper");
    }

    #[test]
    fn test_parameter_parm_wrapped_on_pack() {
        let temp_dir = TempDir::new().unwrap();
        let input_dir = temp_dir.path();
        let output_file = temp_dir.path().join("out.rkaf");

        let content = b"CMDLINE:console=ttyFIQ0,1500000\n";

        fs::write(input_dir.join("parameter.txt"), content).unwrap();
        fs::write(input_dir.join("package-file"), "parameter parameter.txt\n").unwrap();

        // partition-metadata.txt provides flash_size/flash_offset/padded_size;
        // part_byte_count in metadata is not used by pack (recomputed from file)
        let wrapped_size = content.len() as u32 + 12;
        let padded = ((wrapped_size + 2047) / 2048) * 2048;
        fs::write(
            input_dir.join("partition-metadata.txt"),
            format!("parameter,parameter.txt,{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}\n",
                0u32, 0u32, 2048u32, padded, wrapped_size),
        ).unwrap();

        pack_rkaf(
            input_dir.to_str().unwrap(),
            output_file.to_str().unwrap(),
            "RK3562",
            "RK3562",
        ).unwrap();

        let rkaf = fs::read(&output_file).unwrap();
        // UpdateHeader occupies exactly one 2048-byte sector; parameter data starts there
        let param_data = &rkaf[2048..];
        assert_eq!(&param_data[..4], b"PARM", "parameter partition must be PARM-wrapped");
        let stored_len = u32::from_le_bytes(param_data[4..8].try_into().unwrap());
        assert_eq!(stored_len as usize, content.len(), "PARM length field must hold bare content length");
        assert_eq!(&param_data[8..8 + content.len()], content, "PARM content must match bare input");
    }

    #[test]
    fn test_parameter_roundtrip() {
        use std::mem;

        let temp_dir = TempDir::new().unwrap();
        let content = b"CMDLINE:console=ttyFIQ0,1500000\n";

        // --- Build initial RKAF with PARM-wrapped parameter ---
        let input_rkaf = temp_dir.path().join("original.rkaf");
        let unpack_dir = temp_dir.path().join("unpacked");
        let repack_file = temp_dir.path().join("repacked.rkaf");
        let verify_dir = temp_dir.path().join("verified");

        let mut parm_data: Vec<u8> = Vec::new();
        parm_data.extend_from_slice(b"PARM");
        parm_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        parm_data.extend_from_slice(content);
        parm_data.extend_from_slice(&[0u8; 4]); // CRC

        let header_size = mem::size_of::<UpdateHeader>();
        let mut header = UpdateHeader::default();
        header.magic.copy_from_slice(RKAF_SIGNATURE);
        header.num_parts = 1;
        header.length = (header_size + parm_data.len()) as u32;

        let mut part = UpdatePart::default();
        part.name[..9].copy_from_slice(b"parameter");
        part.full_path[..13].copy_from_slice(b"parameter.txt");
        part.part_offset = header_size as u32;
        part.part_byte_count = parm_data.len() as u32;
        header.parts[0] = part;

        let mut file_data: Vec<u8> = header.to_bytes().to_vec();
        file_data.extend_from_slice(&parm_data);
        file_data.extend_from_slice(&[0u8; 4]);
        fs::write(&input_rkaf, &file_data).unwrap();

        // --- Unpack ---
        fs::create_dir_all(&unpack_dir).unwrap();
        unpack_file(input_rkaf.to_str().unwrap(), unpack_dir.to_str().unwrap()).unwrap();

        // Bare content on disk
        let bare = fs::read(unpack_dir.join("parameter.txt")).unwrap();
        assert_eq!(bare.as_slice(), content);

        // --- Repack ---
        // package-file was not in the original image, create it manually
        fs::write(unpack_dir.join("package-file"), "parameter parameter.txt\n").unwrap();

        pack_rkaf(
            unpack_dir.to_str().unwrap(),
            repack_file.to_str().unwrap(),
            "RK3562",
            "RK3562",
        ).unwrap();

        // --- Verify repacked image has PARM wrapper ---
        let repacked = fs::read(&repack_file).unwrap();
        let param_data = &repacked[2048..];
        assert_eq!(&param_data[..4], b"PARM");
        assert_eq!(&param_data[8..8 + content.len()], content);

        // --- Unpack repacked image and verify content survives ---
        fs::create_dir_all(&verify_dir).unwrap();
        unpack_file(repack_file.to_str().unwrap(), verify_dir.to_str().unwrap()).unwrap();
        let final_content = fs::read(verify_dir.join("parameter.txt")).unwrap();
        assert_eq!(final_content.as_slice(), content);
    }
}
