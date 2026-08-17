# Changelog

## v1.2.5 (2026-08-17)

- Fix RKAF repacking when the embedded `package-file` path differs from the member path stored in the container header.
- Preserve original RKAF member paths while accepting `package-file` paths as source-file fallbacks.
- Fix PARM checksums to use Rockchip `rkcrc32`, restoring byte-identical round trips for vendor firmware.
- Preserve structurally valid pre-wrapped PARM blobs byte-for-byte, including blobs produced by legacy tools.

## v1.2.4 (2026-08-14)

- Fix extraction of partitions larger than 4 GiB when later partition offsets wrap at the 32-bit boundary.
- Support repacking firmware with partitions located beyond the 4 GiB boundary.
- Accept vendor package-file comments encoded with non-UTF-8 legacy character sets.

## v1.2.3 (2026-05-10)

- Internal updates and bug fixes.


## v1.2.2 (2026-05-10)

- Internal updates and bug fixes.


## v1.2.1

- Fix: update integration tests to match clap v4 output format (#11)

## v1.2.0

This release includes updated Rockchip chip code mappings, bug fixes for firmware packing, and improved documentation.

### Features
- **Updated Chip Mappings**: The tool now recognizes a more comprehensive list of Rockchip SoCs, ensuring better compatibility and more accurate chip identification during both packing and unpacking. This includes new mappings for `RV1109/RV1126`, `RK3528`, `RK3308`, and many others.

### Bug Fixes
- **Packing/Unpacking Special Partitions**: Fixed a critical bug where the tool would fail when packing or unpacking firmware containing special partitions like `RESERVED` or `backup`. The tool now correctly handles these partitions, preventing crashes and ensuring that firmware images can be repacked successfully.

### Documentation
- **Updated READMEs**: Both `README.md` and `README_CN.md` have been updated with the latest list of supported chip families, providing users with up-to-date information on device compatibility.
