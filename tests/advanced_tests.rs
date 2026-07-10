#[cfg(test)]
mod advanced_tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    
    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::TempDir;

    fn create_mock_rkfw_file(path: &Path) -> std::io::Result<()> {
        let mut data = vec![0u8; 1024];
        
        // 写入 RKFW 签名
        data[0] = b'R';
        data[1] = b'K';
        data[2] = b'F';
        data[3] = b'W';
        
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
        
        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }
    
    fn create_mock_rkaf_file(path: &Path) -> std::io::Result<()> {
        let mut data = vec![0u8; 2048];
        
        // 写入 RKAF 签名
        data[0] = b'R';
        data[1] = b'K';
        data[2] = b'A';
        data[3] = b'F';
        
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
        
        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    #[test]
    fn test_version() {
        let mut cmd = Command::cargo_bin("afptool-rs").unwrap();
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_missing_args() {
        let mut cmd = Command::cargo_bin("afptool-rs").unwrap();
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Usage:"));
    }

    #[test]
    #[ignore] // 默认忽略此测试，因为它需要构建可执行文件
    fn test_unpack_rkfw() -> Result<(), Box<dyn std::error::Error>> {
        // 创建临时目录
        let temp_dir = TempDir::new()?;
        let input_file = temp_dir.path().join("test.rkfw");
        let output_dir = temp_dir.path().join("output");
        fs::create_dir(&output_dir)?;
        
        // 创建测试 RKFW 文件
        create_mock_rkfw_file(&input_file)?;
        
        // 运行命令
        let mut cmd = Command::cargo_bin("afptool-rs")?;
        cmd.arg("unpack")
           .arg(input_file.to_str().unwrap())
           .arg(output_dir.to_str().unwrap());

        cmd.assert()
           .success()
           .stdout(predicate::str::contains("RKFW signature detected"))
           .stdout(predicate::str::contains("version: 8.1.0"))
           .stdout(predicate::str::contains("family: PX30"));
        
        // 验证文件是否被正确提取
        assert!(output_dir.join("BOOT").exists());
        assert!(output_dir.join("embedded-update.img").exists());
        
        Ok(())
    }
    
    #[test]
    #[ignore] // 默认忽略此测试，因为它需要构建可执行文件
    fn test_unpack_rkaf() -> Result<(), Box<dyn std::error::Error>> {
        // 创建临时目录
        let temp_dir = TempDir::new()?;
        let input_file = temp_dir.path().join("test.rkaf");
        let output_dir = temp_dir.path().join("output");
        fs::create_dir(&output_dir)?;
        
        // 创建测试 RKAF 文件
        create_mock_rkaf_file(&input_file)?;
        
        // 运行命令
        let mut cmd = Command::cargo_bin("afptool-rs")?;
        cmd.arg("unpack")
           .arg(input_file.to_str().unwrap())
           .arg(output_dir.to_str().unwrap());

        // 执行命令并检查输出
        cmd.assert()
           .success()
           .stdout(predicate::str::contains("Filesize:"))
           .stdout(predicate::str::contains("manufacturer: RK3326"))
           .stdout(predicate::str::contains("model: RK3326"));
        
        Ok(())
    }
    
    #[test]
    #[ignore] // 默认忽略此测试，因为它需要有效的输入文件
    fn test_invalid_file() {
        // 创建临时目录
        let temp_dir = TempDir::new().unwrap();
        let input_file = temp_dir.path().join("invalid.bin");
        let output_dir = temp_dir.path().join("output");
        fs::create_dir(&output_dir).unwrap();
        
        // 创建无效的测试文件
        let data = vec![0u8; 10];
        let mut file = File::create(&input_file).unwrap();
        file.write_all(&data).unwrap();
        
        // 运行命令
        let mut cmd = Command::cargo_bin("afptool-rs").unwrap();
        cmd.arg("unpack")
           .arg(input_file.to_str().unwrap())
           .arg(output_dir.to_str().unwrap());

        // 应该失败并输出错误信息
        cmd.assert()
           .failure()
           .stderr(predicate::str::contains("Unknown signature"));
    }
}
