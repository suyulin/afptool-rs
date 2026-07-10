#[cfg(test)]
mod integration_tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use assert_cmd::Command;
    use predicates::prelude::*;
    
    fn setup_test_files() -> (String, String) {
        // 创建测试目录
        let test_data_dir = Path::new("tests/data/integration");
        let output_dir = test_data_dir.join("output");
        
        if !test_data_dir.exists() {
            fs::create_dir_all(test_data_dir).unwrap();
        }
        
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).unwrap();
        }
        fs::create_dir_all(&output_dir).unwrap();
        
        // 创建一个简单的模拟 RKFW 文件用于测试
        let rkfw_data = create_mock_rkfw_file();
        let rkfw_path = test_data_dir.join("mock.rkfw");
        let mut file = File::create(&rkfw_path).unwrap();
        file.write_all(&rkfw_data).unwrap();
        
        (rkfw_path.to_string_lossy().to_string(), output_dir.to_string_lossy().to_string())
    }
    
    fn create_mock_rkfw_file() -> Vec<u8> {
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
        
        data
    }
    
    #[test]
    fn test_cli_help() {
        let mut cmd = Command::cargo_bin("afptool-rs").unwrap();
        cmd.arg("--help");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("afptool-rs"))
            .stdout(predicate::str::contains("A Rust tool for packing and unpacking RockChip firmware images"));
    }

    #[test]
    fn test_cli_unpack_help() {
        let mut cmd = Command::cargo_bin("afptool-rs").unwrap();
        cmd.arg("unpack").arg("--help");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Path to the firmware file"))
            .stdout(predicate::str::contains("Directory where extracted files will be saved"));
    }
    
    #[test]
    #[ignore] // 默认忽略此测试，因为它需要构建可执行文件
    fn test_cli_unpack_rkfw() -> Result<(), Box<dyn std::error::Error>> {
        // 设置测试文件
        let (input_path, output_dir) = setup_test_files();
        
        // 运行命令
        let mut cmd = Command::cargo_bin("afptool-rs")?;
        cmd.arg("unpack").arg(&input_path).arg(&output_dir);
        
        // 执行命令并检查输出
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("RKFW signature detected"))
            .stdout(predicate::str::contains("version: 8.1.0"))
            .stdout(predicate::str::contains("family: PX30"));
        
        // 检查文件是否被正确提取
        let boot_file = Path::new(&output_dir).join("BOOT");
        let update_file = Path::new(&output_dir).join("embedded-update.img");
        
        assert!(boot_file.exists(), "BOOT file was not extracted");
        assert!(update_file.exists(), "embedded-update.img was not extracted");
        
        // 清理测试文件
        fs::remove_file(input_path).unwrap();
        fs::remove_dir_all(output_dir).unwrap();
        
        Ok(())
    }
}
