# afptool-rs

一个用于解包瑞芯微 SSFW/RKFW/RKAF 固件映像，并打包 RKFW/RKAF 映像的 Rust 工具。

## 功能特性

- 解包多存储 SSFW 和 RKFW 固件文件
- 提取嵌入式 RKAF 更新映像
- 支持多种瑞芯微芯片系列（RK29xx、RK30xx、RK31xx、RK32xx、RK3368、RK3326、RK3566、PX30）
- 跨平台支持（Windows、macOS、Linux）

## 构建

### 标准构建
```bash
cargo build --release
```

### macOS 通用二进制文件
```bash
./build.sh
```

### GitHub Actions
本项目包含自动化 CI/CD 流程，通过 GitHub Actions 构建以下平台的二进制文件：
- Linux x86_64
- Linux ARM64 (aarch64)
- macOS x86_64
- macOS ARM64 (Apple Silicon)
- macOS 通用二进制文件
- Windows x86_64

当您推送版本标签（例如 `v1.0.0`）时，会自动创建发布版本。

## 使用方法

```bash
afptool-rs unpack <输入文件> <输出目录>
```

### 示例

**解包 SSFW 多存储固件：**
```bash
$ afptool-rs unpack update_spi_nvme.img ./out
SSFW signature detected
version: 0x00000010
merge version: 0x02000000
images: 2
image 0: offset=0x60, size=0x4dd24a, storage=spinor (mask=0x00000200, code=9)
image 1: offset=0x4dd2aa, size=0x71d4f24a, storage=pcie (mask=0x00000800, code=11)
```

每个内嵌 RKFW 会保存为 `firmware-<序号>-<存储类型>.img`，
`ssfw-manifest.txt` 记录原始偏移、长度、存储掩码和 RockUSB 存储选择码。
库 API 还提供 `SsfwImage`，可直接零拷贝解析内存映射的 SSFW 文件。

**解包 RKFW 固件：**
```bash
$ afptool-rs rk.img ./out
RKFW signature detected
version: 8.1.0
family: PX30
00000066-0004c1b3 BOOT                       (size: 311630)
0004c1b4-a2e8c9b7 embedded-update.img        (size: 2732853252)
```

**提取嵌入式 RKAF 更新映像：**
```bash
$ afptool-rs ./out/embedded-update.img ./out 
Filesize: 2732853252
manufacturer: RK3326
model: RK3326
00000800-000002eb ./out/package-file
00001000-0004c14e ./out/Image/MiniLoaderAll.bin
0004d800-0000031f ./out/Image/parameter.txt
0004e000-00400000 ./out/Image/trust.img
0084e000-0000c000 ./out/Image/misc.img
00a2e000-012b8814 ./out/Image/kernel.img
01ce7000-0016a40c ./out/Image/boot.img
01e51800-026a07c4 ./out/Image/recovery.img
628ec800-0c82b0a0 ./out/Image/oem.img
6f118000-33d28274 ./out/Image/update_back.img
```

## 支持的格式

- **SSFW**：由 `rkImageMaker -merge` 生成的瑞芯微多存储固件容器
- **RKFW**：瑞芯微固件包装格式
- **RKAF**：瑞芯微 Android 固件包格式

## 支持的芯片系列

| 芯片代码 | 系列 | 描述 |
|---|---|---|
| 0x19 | RV1109 / RV1126 | 视觉 AI 处理器 (NPU) |
| 0x30 | PX30 / RK3326 | 工业/车载 / 早期复古掌机常用 |
| 0x32 | RK3562 | 2023年后入门级平板/教育机 |
| 0x33 | RK3399 / RK3399Pro | 经典高性能六核 (含 Pro 专用 NPU 版) |
| 0x35 | RK3588 / RK3588S | 旗舰八核 (8K, 6TOPS NPU) |
| 0x36 | RK3326 | (部分版本与 0x30 兼容，特定版 ID) |
| 0x38 | RK3566 / RK3568 | 当前主流中端 AIoT |
| 0x39 | RK3528 | 4K 多媒体/电视盒子 |
| 0x41 | RK3368 | 早期八核 64 位系列 |
| 0x48 | RK3308 | 语音/音频专用 SoC |
| 0x50 | RK29xx | 极早期单核系列 |
| 0x51 | RV1108 | 早期视觉处理 SoC |
| 0x60 | RK30xx / RK3066 | 双核时代 |
| 0x70 | RK31xx / RK3188 | 早期四核 (28nm 经典款) |
| 0x80 | RK32xx / RK3288 | 32位时代的性能旗舰 |

## 测试

### 运行单元测试

```bash
cargo test
```

### 运行所有测试（包括集成测试）

```bash
cargo test -- --include-ignored
```

### 运行被忽略的集成测试

部分集成测试会调用完整的命令行可执行文件，因此通过 `#[ignore]`
标记避免在每次 `cargo test` 时运行。若需要验证这些场景，可以在有
模拟固件文件的情况下单独执行：

```bash
# tests/advanced_tests.rs 中的高级场景
cargo test advanced_tests -- --ignored

# tests/cli_tests.rs 中的 CLI 级断言
cargo test cli_tests -- --ignored
```

`tests/advanced_tests.rs` 与 `tests/cli_tests.rs` 中的辅助函数会在
`tests/data/integration` 目录下生成临时的 RKFW/RKAF 模拟文件，因此只
需确保输出目录可写，无需额外准备数据。

### 运行特定测试

```bash
cargo test test_update_header_from_bytes
```

### 测试覆盖范围

测试用例覆盖以下功能：

- 解析 RKFW 和 RKAF 文件头
- 解包 RKFW 和 RKAF 文件
- 命令行接口验证
- 错误处理

## 许可证

本项目采用 Apache License 2.0 许可证 - 详见 [LICENSE](LICENSE) 文件。
