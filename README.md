# afptool-rs

A Rust tool for unpacking and packing RockChip firmware images (RKFW and RKAF formats).

## Features

- Unpack RKFW firmware files
- Extract embedded RKAF update images
- Pack RKFW/RKAF files
- Support for various RockChip chip families (RK29xx, RK30xx, RK31xx, RK32xx, RK3368, RK3326, RK3566, RK3562, PX30)
- Cross-platform support (Windows, macOS, Linux)

## Build

### Standard build
```bash
cargo build --release
```

### Universal macOS binary
```bash
./build.sh
```

### GitHub Actions
This project includes automated CI/CD with GitHub Actions that builds binaries for:
- Linux x86_64
- Linux ARM64 (aarch64)
- macOS x86_64
- macOS ARM64 (Apple Silicon)
- macOS Universal Binary
- Windows x86_64

Releases are automatically created when you push a version tag (e.g., `v1.0.0`).

## Usage

### Unpacking

```bash
afptool-rs unpack <input_file> <output_directory>
```

**Unpack RKFW firmware:**
```bash
$ afptool-rs unpack rk.img ./out
RKFW signature detected
version: 1.0.0
code: 0x02000000
date: 2025-11-06 13:33:14 (Unix timestamp: 1762435994)
family: RK3562
00000066-00072a25 BOOT                       (size: 469440)
00072a26-10495a29 embedded-update.img        (size: 272773124)
```

**Extract embedded RKAF update image:**
```bash
$ afptool-rs unpack ./out/embedded-update.img ./out
Filesize: 272773124
manufacturer:  RK3562
model:  RK3562
00000800-000000fb ./out/package-file
00001000-0000021d ./out/parameter.txt
00001800-000729c0 ./out/MiniLoaderAll.bin
00074800-00400000 ./out/uboot.img
00074800-00400000 ./out/uboot.img
00474800-0000c000 ./out/misc.img
00480800-01084800 ./out/boot.img
00480800-01084800 ./out/boot.img
01505000-0df1e000 ./out/rootfs.img
01505000-0df1e000 ./out/rootfs.img
0f423000-00800000 ./out/oem.img
0fc23000-00800000 ./out/userdata.img

Partition metadata saved to: ./out/partition-metadata.txt
```

### Run Ignored Integration Suites

Some integration-style tests exercise the full CLI binary and are marked with
`#[ignore]` to avoid running them on every `cargo test`. You can run these
targeted suites when you have the mocked firmware files available:

```bash
# Advanced scenarios from tests/advanced_tests.rs
cargo test advanced_tests -- --ignored

# CLI level assertions from tests/cli_tests.rs
cargo test cli_tests -- --ignored
```

The helpers in `tests/advanced_tests.rs` and `tests/cli_tests.rs` will generate
temporary RKFW/RKAF mock artifacts under `tests/data/integration`, so no
additional setup is required beyond ensuring the output directory is writable.

### Packing

**Pack RKFW firmware:**
```bash
afptool-rs pack-rkfw <input_directory> <output_file> [--chip <chip>] [--version <version>] [--timestamp <unix_timestamp>] [--code <code_field_hex>]
```

When the input directory contains an `rkfw-header.bin` (saved automatically by
`unpack`), the original header is used as a template and all flags become
optional overrides — repacking unchanged content reproduces the original image
byte-for-byte. Without a template, all four flags are required.

Example (repacking an unpacked image — flags come from the saved header):
```bash
$ afptool-rs pack-rkfw ./out ./repacked.img
```

Example (building from scratch):
```bash
$ afptool-rs pack-rkfw ./out ./repacked.img --chip RK3562 --version 1.0.0 --timestamp 1762435994 --code 0x02000000
Successfully packed RKFW image:
  Output: ./repacked.img
  Version: 1.0.0
  Date: 2025-11-06 13:33:14
  Chip: RK3562
  BOOT size: 469440 bytes
  Update image size: 272773124 bytes
  MD5: 9574d7cdf6f6a45bfaaad62f171fd185
  Total size: 273242698 bytes
```

**Pack RKAF update image:**
```bash
afptool-rs pack-rkaf <input_directory> <output_file> --model <model> --manufacturer <manufacturer>
```

When the input directory contains an `rkaf-header.bin` (saved automatically by
`unpack`), it is used as a header template so vendor-specific header bytes
survive the round-trip. Entries whose path is `RESERVED` are listed in
`package-file` but get no header entry, matching the official tool's behavior.

Example:
```bash
$ afptool-rs pack-rkaf ./out repacked-update.img --model RK3562 --manufacturer RK3562
Successfully packed RKAF image:
  Output: repacked-update.img
  Model: RK3562
  Manufacturer: RK3562
  Parts: 12
  Total size: 272773120 bytes
```

**Notes:**
- When unpacking RKAF, partition metadata is saved to `partition-metadata.txt`. You need correct metadata to pack RKAF
- Use `--timestamp` parameter to preserve the original build timestamp for byte-perfect matching

## Supported Formats

- **RKFW**: RockChip firmware wrapper format
- **RKAF**: RockChip Android firmware package format

## Supported Chip Families

| Chip Code | Family | Description |
|---|---|---|
| 0x19 | RV1109 / RV1126 | Visual AI Processor (NPU) |
| 0x30 | PX30 / RK3326 | Industrial/Vehicle / Commonly used in early retro handhelds |
| 0x32 | RK3562 | Entry-level tablets/educational devices post-2023 |
| 0x33 | RK3399 / RK3399Pro | Classic high-performance hexa-core (including Pro version with dedicated NPU) |
| 0x35 | RK3588 / RK3588S | Flagship octa-core (8K, 6TOPS NPU) |
| 0x36 | RK3326 | (Some versions compatible with 0x30, specific ID) |
| 0x38 | RK3566 / RK3568 | Current mainstream mid-range AIoT |
| 0x39 | RK3528 | 4K multimedia/TV box |
| 0x41 | RK3368 | Early octa-core 64-bit series |
| 0x48 | RK3308 | Voice/Audio dedicated SoC |
| 0x50 | RK29xx | Very early single-core series |
| 0x51 | RV1108 | Early visual processing SoC |
| 0x60 | RK30xx / RK3066 | Dual-core era |
| 0x70 | RK31xx / RK3188 | Early quad-core (28nm classic) |
| 0x80 | RK32xx / RK3288 | 32-bit era performance flagship |

## Testing

### Run Unit Tests

```bash
cargo test
```

### Run All Tests (Including Integration Tests)

```bash
cargo test -- --include-ignored
```

### Run Specific Test

```bash
cargo test test_update_header_from_bytes
```

### Test Coverage

The test cases cover the following functionality:

- Parsing RKFW and RKAF file headers
- Unpacking RKFW and RKAF files
- Command-line interface validation
- Error handling

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.
