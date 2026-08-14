use std::fs::File;
use std::io::{Read, Write, BufRead, BufReader, BufWriter, Seek};
use std::collections::HashMap;
use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, Timelike};
use crate::{UpdateHeader, RKFW_SIGNATURE, RKAF_SIGNATURE, UPDATE_HEADER_SIZE};

#[derive(Debug, Clone)]
struct PartitionMetadata {
    flash_size: u32,
    flash_offset: u32,
}

// RockChip CRC-32 table
const RKCRC32_TABLE: [u32; 256] = [
    0x00000000, 0x04c10db7, 0x09821b6e, 0x0d4316d9,
    0x130436dc, 0x17c53b6b, 0x1a862db2, 0x1e472005,
    0x26086db8, 0x22c9600f, 0x2f8a76d6, 0x2b4b7b61,
    0x350c5b64, 0x31cd56d3, 0x3c8e400a, 0x384f4dbd,
    0x4c10db70, 0x48d1d6c7, 0x4592c01e, 0x4153cda9,
    0x5f14edac, 0x5bd5e01b, 0x5696f6c2, 0x5257fb75,
    0x6a18b6c8, 0x6ed9bb7f, 0x639aada6, 0x675ba011,
    0x791c8014, 0x7ddd8da3, 0x709e9b7a, 0x745f96cd,
    0x9821b6e0, 0x9ce0bb57, 0x91a3ad8e, 0x9562a039,
    0x8b25803c, 0x8fe48d8b, 0x82a79b52, 0x866696e5,
    0xbe29db58, 0xbae8d6ef, 0xb7abc036, 0xb36acd81,
    0xad2ded84, 0xa9ece033, 0xa4aff6ea, 0xa06efb5d,
    0xd4316d90, 0xd0f06027, 0xddb376fe, 0xd9727b49,
    0xc7355b4c, 0xc3f456fb, 0xceb74022, 0xca764d95,
    0xf2390028, 0xf6f80d9f, 0xfbbb1b46, 0xff7a16f1,
    0xe13d36f4, 0xe5fc3b43, 0xe8bf2d9a, 0xec7e202d,
    0x34826077, 0x30436dc0, 0x3d007b19, 0x39c176ae,
    0x278656ab, 0x23475b1c, 0x2e044dc5, 0x2ac54072,
    0x128a0dcf, 0x164b0078, 0x1b0816a1, 0x1fc91b16,
    0x018e3b13, 0x054f36a4, 0x080c207d, 0x0ccd2dca,
    0x7892bb07, 0x7c53b6b0, 0x7110a069, 0x75d1adde,
    0x6b968ddb, 0x6f57806c, 0x621496b5, 0x66d59b02,
    0x5e9ad6bf, 0x5a5bdb08, 0x5718cdd1, 0x53d9c066,
    0x4d9ee063, 0x495fedd4, 0x441cfb0d, 0x40ddf6ba,
    0xaca3d697, 0xa862db20, 0xa521cdf9, 0xa1e0c04e,
    0xbfa7e04b, 0xbb66edfc, 0xb625fb25, 0xb2e4f692,
    0x8aabbb2f, 0x8e6ab698, 0x8329a041, 0x87e8adf6,
    0x99af8df3, 0x9d6e8044, 0x902d969d, 0x94ec9b2a,
    0xe0b30de7, 0xe4720050, 0xe9311689, 0xedf01b3e,
    0xf3b73b3b, 0xf776368c, 0xfa352055, 0xfef42de2,
    0xc6bb605f, 0xc27a6de8, 0xcf397b31, 0xcbf87686,
    0xd5bf5683, 0xd17e5b34, 0xdc3d4ded, 0xd8fc405a,
    0x6904c0ee, 0x6dc5cd59, 0x6086db80, 0x6447d637,
    0x7a00f632, 0x7ec1fb85, 0x7382ed5c, 0x7743e0eb,
    0x4f0cad56, 0x4bcda0e1, 0x468eb638, 0x424fbb8f,
    0x5c089b8a, 0x58c9963d, 0x558a80e4, 0x514b8d53,
    0x25141b9e, 0x21d51629, 0x2c9600f0, 0x28570d47,
    0x36102d42, 0x32d120f5, 0x3f92362c, 0x3b533b9b,
    0x031c7626, 0x07dd7b91, 0x0a9e6d48, 0x0e5f60ff,
    0x101840fa, 0x14d94d4d, 0x199a5b94, 0x1d5b5623,
    0xf125760e, 0xf5e47bb9, 0xf8a76d60, 0xfc6660d7,
    0xe22140d2, 0xe6e04d65, 0xeba35bbc, 0xef62560b,
    0xd72d1bb6, 0xd3ec1601, 0xdeaf00d8, 0xda6e0d6f,
    0xc4292d6a, 0xc0e820dd, 0xcdab3604, 0xc96a3bb3,
    0xbd35ad7e, 0xb9f4a0c9, 0xb4b7b610, 0xb076bba7,
    0xae319ba2, 0xaaf09615, 0xa7b380cc, 0xa3728d7b,
    0x9b3dc0c6, 0x9ffccd71, 0x92bfdba8, 0x967ed61f,
    0x8839f61a, 0x8cf8fbad, 0x81bbed74, 0x857ae0c3,
    0x5d86a099, 0x5947ad2e, 0x5404bbf7, 0x50c5b640,
    0x4e829645, 0x4a439bf2, 0x47008d2b, 0x43c1809c,
    0x7b8ecd21, 0x7f4fc096, 0x720cd64f, 0x76cddbf8,
    0x688afbfd, 0x6c4bf64a, 0x6108e093, 0x65c9ed24,
    0x11967be9, 0x1557765e, 0x18146087, 0x1cd56d30,
    0x02924d35, 0x06534082, 0x0b10565b, 0x0fd15bec,
    0x379e1651, 0x335f1be6, 0x3e1c0d3f, 0x3add0088,
    0x249a208d, 0x205b2d3a, 0x2d183be3, 0x29d93654,
    0xc5a71679, 0xc1661bce, 0xcc250d17, 0xc8e400a0,
    0xd6a320a5, 0xd2622d12, 0xdf213bcb, 0xdbe0367c,
    0xe3af7bc1, 0xe76e7676, 0xea2d60af, 0xeeec6d18,
    0xf0ab4d1d, 0xf46a40aa, 0xf9295673, 0xfde85bc4,
    0x89b7cd09, 0x8d76c0be, 0x8035d667, 0x84f4dbd0,
    0x9ab3fbd5, 0x9e72f662, 0x9331e0bb, 0x97f0ed0c,
    0xafbfa0b1, 0xab7ead06, 0xa63dbbdf, 0xa2fcb668,
    0xbcbb966d, 0xb87a9bda, 0xb5398d03, 0xb1f880b4,
];

fn rkcrc32(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        let index = ((crc >> 24) ^ (byte as u32)) as usize;
        crc = (crc << 8) ^ RKCRC32_TABLE[index & 0xFF];
    }
    crc
}

fn parm_crc32(data: &[u8]) -> u32 {
    const POLYNOMIAL: u32 = 0x04c11db7;

    let mut crc = 0u32;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x80000000 != 0 {
                (crc << 1) ^ POLYNOMIAL
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn is_valid_parm_blob(data: &[u8]) -> bool {
    const PARM_OVERHEAD: usize = 12;

    if data.len() < PARM_OVERHEAD || &data[..4] != b"PARM" {
        return false;
    }

    let content_len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    if content_len != data.len() - PARM_OVERHEAD {
        return false;
    }

    let content_end = 8 + content_len;
    let stored_crc = u32::from_le_bytes(data[content_end..content_end + 4].try_into().unwrap());
    parm_crc32(&data[8..content_end]) == stored_crc
}

fn parse_partition_metadata(input_dir: &str) -> Result<HashMap<String, PartitionMetadata>> {
    let metadata_path = format!("{}/partition-metadata.txt", input_dir);
    let mut metadata_map = HashMap::new();

    let file = match File::open(&metadata_path) {
        Ok(f) => f,
        Err(_) => return Ok(metadata_map),
    };

    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 7 {
            let name = parts[0].to_string();
            let flash_size = u32::from_str_radix(parts[2].trim_start_matches("0x"), 16)?;
            let flash_offset = u32::from_str_radix(parts[3].trim_start_matches("0x"), 16)?;

            metadata_map.insert(name, PartitionMetadata {
                flash_size,
                flash_offset,
            });
        }
    }

    Ok(metadata_map)
}

pub fn pack_rkfw(
    input_dir: &str,
    output_file: &str,
    chip: Option<&str>,
    version: Option<&str>,
    timestamp: Option<i64>,
    code_hex: Option<&str>,
) -> Result<()> {
    const HEADER_SIZE: usize = 0x66;

    let boot_path = format!("{}/BOOT", input_dir);
    let update_path = format!("{}/embedded-update.img", input_dir);
    let template_path = format!("{}/rkfw-header.bin", input_dir);

    // When repacking an unpacked image, start from the original header so
    // undocumented fields (e.g. bytes 0x36..0x39) survive the round-trip;
    // CLI flags act as overrides.
    let template = match std::fs::read(&template_path) {
        Ok(data) if data.len() >= HEADER_SIZE => Some(data),
        _ => None,
    };

    let mut header = match &template {
        Some(data) => data[..HEADER_SIZE].to_vec(),
        None => vec![0u8; HEADER_SIZE],
    };

    let missing = |flag: &str| {
        anyhow!(
            "--{} is required because no rkfw-header.bin template was found in {}",
            flag,
            input_dir
        )
    };

    header[0..4].copy_from_slice(RKFW_SIGNATURE);
    header[0x04] = HEADER_SIZE as u8;

    if let Some(version) = version {
        let version_parts: Vec<&str> = version.split('.').collect();
        if version_parts.len() != 3 {
            return Err(anyhow!("Version must be in format: major.minor.build (e.g., 8.1.0)"));
        }

        let major: u8 = version_parts[0].parse()
            .map_err(|_| anyhow!("Invalid major version"))?;
        let minor: u8 = version_parts[1].parse()
            .map_err(|_| anyhow!("Invalid minor version"))?;
        let build: u16 = version_parts[2].parse()
            .map_err(|_| anyhow!("Invalid build number"))?;

        header[6] = (build & 0xFF) as u8;
        header[7] = ((build >> 8) & 0xFF) as u8;
        header[8] = minor;
        header[9] = major;
    } else if template.is_none() {
        return Err(missing("version"));
    }

    if let Some(code_hex) = code_hex {
        let hex_str = code_hex.trim_start_matches("0x").trim_start_matches("0X");
        let code_value = u32::from_str_radix(hex_str, 16)
            .map_err(|_| anyhow!("Invalid hex value for code field: {}", hex_str))?;
        header[0x0a..0x0e].copy_from_slice(&code_value.to_le_bytes());
    } else if template.is_none() {
        return Err(missing("code"));
    }

    if let Some(timestamp) = timestamp {
        let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
            .ok_or_else(|| anyhow!("Invalid timestamp"))?
            .naive_utc();

        let year = datetime.year() as u16;
        header[0x0e] = (year & 0xFF) as u8;
        header[0x0f] = ((year >> 8) & 0xFF) as u8;
        header[0x10] = datetime.month() as u8;
        header[0x11] = datetime.day() as u8;
        header[0x12] = datetime.hour() as u8;
        header[0x13] = datetime.minute() as u8;
        header[0x14] = datetime.second() as u8;
    } else if template.is_none() {
        return Err(missing("timestamp"));
    }

    if let Some(chip) = chip {
        header[0x15..0x19].copy_from_slice(&encode_chip_field(chip)?);
    } else if template.is_none() {
        return Err(missing("chip"));
    }

    if template.is_none() {
        header[0x2d] = 0x01;
    }

    let mut boot_data = Vec::new();
    File::open(&boot_path)
        .map_err(|_| anyhow!("Cannot find BOOT file in {}", input_dir))?
        .read_to_end(&mut boot_data)?;

    let mut update_file = File::open(&update_path)
        .map_err(|_| anyhow!("Cannot find embedded-update.img file in {}", input_dir))?;
    let update_size = update_file.metadata()?.len();

    let mut update_magic = [0u8; 4];
    if update_size < 4 {
        return Err(anyhow!("embedded-update.img must be a valid RKAF file"));
    }
    update_file.read_exact(&mut update_magic)?;
    if &update_magic != b"RKAF" {
        return Err(anyhow!("embedded-update.img must be a valid RKAF file"));
    }
    update_file.seek(std::io::SeekFrom::Start(0))?;

    let boot_offset = HEADER_SIZE as u64;
    let boot_size = boot_data.len() as u64;
    let update_offset = boot_offset + boot_size;

    // These offsets have no verified >4 GiB on-disk encoding; refuse rather
    // than wrap silently. (In practice BOOT is well under 1 MiB.)
    if boot_size > u32::MAX as u64 || update_offset > u32::MAX as u64 {
        return Err(anyhow!("BOOT is too large ({} bytes)", boot_size));
    }

    put_u32_le(&mut header[0x19..], boot_offset as u32);
    put_u32_le(&mut header[0x1d..], boot_size as u32);
    put_u32_le(&mut header[0x21..], update_offset as u32);
    // The on-disk size field is 32-bit; vendor images store the low 32 bits
    // for >4 GiB updates (verified against a real RK3588 firmware image).
    put_u32_le(&mut header[0x25..], update_size as u32);
    if update_size > u32::MAX as u64 {
        println!(
            "note: update image is {} bytes (>4 GiB); header stores the low 32 bits (0x{:08x}) as vendor images do",
            update_size, update_size as u32
        );
    }

    // Stream the (potentially multi-GiB) update image through an incremental
    // MD5 instead of concatenating everything in memory.
    let mut md5_ctx = md5::Context::new();
    let mut out_file = BufWriter::new(File::create(output_file)?);

    md5_ctx.consume(&header);
    out_file.write_all(&header)?;
    md5_ctx.consume(&boot_data);
    out_file.write_all(&boot_data)?;

    let mut chunk = vec![0u8; 4 * 1024 * 1024];
    loop {
        let n = update_file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        md5_ctx.consume(&chunk[..n]);
        out_file.write_all(&chunk[..n])?;
    }

    let md5_hex = format!("{:x}", md5_ctx.finalize());
    out_file.write_all(md5_hex.as_bytes())?;
    out_file.flush()?;

    let total_size = update_offset + update_size + md5_hex.len() as u64;
    let chip_desc = chip
        .map(str::to_string)
        .or_else(|| crate::decode_chip_field(&header[0x15..0x19].try_into().unwrap()))
        .unwrap_or_else(|| "from template".to_string());

    println!("Successfully packed RKFW image:");
    println!("  Output: {}", output_file);
    println!("  Version: {}.{}.{}", header[9], header[8], ((header[7] as u16) << 8) | header[6] as u16);
    println!(
        "  Date: {}-{:02}-{:02} {:02}:{:02}:{:02}",
        ((header[0x0f] as u16) << 8) | header[0x0e] as u16,
        header[0x10], header[0x11], header[0x12], header[0x13], header[0x14]
    );
    println!("  Chip: {}", chip_desc);
    println!("  BOOT size: {} bytes", boot_size);
    println!("  Update image size: {} bytes", update_size);
    println!("  MD5: {}", md5_hex);
    println!("  Total size: {} bytes", total_size);

    Ok(())
}

/// Encode the chip identity field stored at RKFW header offsets 0x15..0x19.
///
/// Modern 4-digit chips (RK35xx/RK33xx/RV11xx, ...) store the four ASCII
/// digits of the chip number in reverse byte order: a vendor RK3588 image
/// carries '8' '8' '5' '3', which flashing tools read back as "3588". Legacy
/// families (RK29xx/RV1108/RK30xx/RK31xx/RK32xx) predate that scheme and
/// store a one-byte family code followed by up to three ASCII digits.
pub fn encode_chip_field(chip: &str) -> Result<[u8; 4]> {
    let family_code = chip_name_to_code(chip)?;
    let digits: Vec<u8> = chip
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c as u8)
        .collect();

    let legacy = matches!(family_code, 0x50 | 0x51 | 0x60 | 0x70 | 0x80);
    if !legacy && digits.len() == 4 {
        return Ok([digits[3], digits[2], digits[1], digits[0]]);
    }

    let mut field = [family_code, 0, 0, 0];
    if digits.len() >= 3 {
        field[1] = digits[2];
        field[2] = digits[1];
        field[3] = digits[0];
    }
    Ok(field)
}

pub fn chip_name_to_code(chip: &str) -> Result<u8> {
    match chip.to_uppercase().as_str() {
        "RV1109" | "RV1126" => Ok(0x19),
        "PX30" => Ok(0x30),
        "RK3562" => Ok(0x32),
        "RK3399" | "RK3399PRO" => Ok(0x33),
        "RK3588" | "RK3588S" => Ok(0x35),
        "RK3326" => Ok(0x36),
        "RK3566" | "RK3568" => Ok(0x38),
        "RK3528" => Ok(0x39),
        "RK3368" => Ok(0x41),
        "RK3308" => Ok(0x48),
        "RK29XX" | "RK29" | "RK2918" | "RK2908" => Ok(0x50),
        "RV1108" => Ok(0x51),
        "RK30XX" | "RK30" | "RK3066" | "RK3026" => Ok(0x60),
        "RK31XX" | "RK31" | "RK3188" | "PX1" | "PX3" | "PX4" => Ok(0x70),
        "RK32XX" | "RK32" | "RK3288" => Ok(0x80),
        _ => Err(anyhow!("Unsupported chip family: {}", chip)),
    }
}

/// Write a NUL-terminated string into a fixed-size header field WITHOUT
/// zeroing the rest of the field. Vendor headers carry undocumented bytes in
/// the tails of string fields; when packing from a saved header template
/// those bytes must survive. Readers stop at the first NUL, so the string
/// stays well-formed either way.
fn write_cstr_preserving_tail(field: &mut [u8], value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(field.len() - 1);
    field[..len].copy_from_slice(&bytes[..len]);
    field[len] = 0;
}

fn put_u32_le(slice: &mut [u8], value: u32) {
    let bytes = value.to_le_bytes();
    slice[0] = bytes[0];
    slice[1] = bytes[1];
    slice[2] = bytes[2];
    slice[3] = bytes[3];
}

pub fn pack_rkaf(input_dir: &str, output_file: &str, model: &str, manufacturer: &str) -> Result<()> {
    let package_file_path = format!("{}/package-file", input_dir);
    let package_file = std::fs::read(&package_file_path)
        .map_err(|_| anyhow!("Cannot find package-file in {}", input_dir))?;
    let mut file_list = Vec::new();

    // Vendor package files can contain comments in legacy encodings such as
    // GBK. File entries themselves are ASCII, so parse lines lossily while
    // preserving the original package-file bytes written into the image.
    for raw_line in package_file.split(|&byte| byte == b'\n') {
        let line = String::from_utf8_lossy(raw_line);
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            file_list.push((parts[0].to_string(), parts[1].to_string()));
        }
    }

    if file_list.is_empty() {
        return Err(anyhow!("No files found in package-file"));
    }

    let mut machine_id = String::new();
    if let Ok(param_file) = File::open(format!("{}/parameter.txt", input_dir)) {
        let reader = BufReader::new(param_file);
        for line in reader.lines().map_while(Result::ok) {
            if line.starts_with("MACHINE_ID:") {
                machine_id = line.split(':').nth(1).unwrap_or("").trim().to_string();
                break;
            }
        }
    }

    // When repacking an unpacked image, start from the original header so
    // undocumented bytes (the vendor tool leaves data in the tails of string
    // fields) survive; the fields below are overwritten with computed values.
    let template_path = format!("{}/rkaf-header.bin", input_dir);
    let mut header = match std::fs::read(&template_path) {
        Ok(data) if data.len() >= UPDATE_HEADER_SIZE => UpdateHeader::decode(&data)
            .with_context(|| format!("Invalid RKAF header template: {template_path}"))?,
        _ => UpdateHeader {
            version: 0x01000000,
            ..Default::default()
        },
    };
    header.magic.copy_from_slice(RKAF_SIGNATURE);

    let model_str = if model.starts_with(' ') {
        model.to_string()
    } else {
        format!(" {}", model)
    };
    write_cstr_preserving_tail(&mut header.model, &model_str);

    let manufacturer_str = if manufacturer.starts_with(' ') {
        manufacturer.to_string()
    } else {
        format!(" {}", manufacturer)
    };
    write_cstr_preserving_tail(&mut header.manufacturer, &manufacturer_str);

    if !machine_id.is_empty() {
        write_cstr_preserving_tail(&mut header.id, &format!(" {}", machine_id));
    } else {
        // No MACHINE_ID in parameter.txt: clear the logical id so a template
        // value cannot leak through. Bytes after the NUL are left untouched
        // for vendor-tail fidelity; readers stop at the first NUL.
        header.id[0] = 0;
    }

    let partition_metadata = parse_partition_metadata(input_dir)?;
    if partition_metadata.is_empty() {
        return Err(anyhow!("Missing partition metadata"));
    }

    let header_size = UPDATE_HEADER_SIZE as u64;
    let sector_size: u64 = 2048;
    let data_start = header_size.div_ceil(sector_size) * sector_size;
    let mut current_offset = data_start;

    // Layout pass: sizes come from file metadata (or the small PARM-wrapped
    // parameter blob built in memory); multi-GiB partitions are never loaded.
    // (path, pre-built data for "parameter" partitions, true size, padded size)
    let mut write_list: Vec<(String, Option<Vec<u8>>, u64, u64)> = Vec::new();
    let mut layout_map: HashMap<String, (u64, u64)> = HashMap::new();

    let mut emitted = 0usize;
    for (name, path) in file_list.iter() {
        // Vendor afptool keeps RESERVED lines in package-file but does not
        // emit a header entry for them (verified against a real RK3588 image:
        // 10 package-file lines, num_parts = 9).
        if path == "RESERVED" {
            continue;
        }

        let (file_offset, file_size) = if path == "SELF" {
            (0u64, 0u64)
        } else if let Some(&(offset, size)) = layout_map.get(path) {
            // File already laid out, reuse offset
            (offset, size)
        } else {
            let file_path = format!("{}/{}", input_dir, path);

            // "parameter" partitions are stored PARM-wrapped in the image
            let (file_size, prebuilt) = if name == "parameter" {
                let raw_data = std::fs::read(&file_path)
                    .map_err(|e| anyhow!("Cannot open {}: {}", file_path, e))?;
                let wrapped = if is_valid_parm_blob(&raw_data) {
                    raw_data
                } else {
                    let content_len = raw_data.len() as u32;
                    let mut wrapped = Vec::with_capacity(raw_data.len() + 12);
                    wrapped.extend_from_slice(b"PARM");
                    wrapped.extend_from_slice(&content_len.to_le_bytes());
                    wrapped.extend_from_slice(&raw_data);
                    let crc = parm_crc32(&raw_data);
                    wrapped.extend_from_slice(&crc.to_le_bytes());
                    wrapped
                };
                (wrapped.len() as u64, Some(wrapped))
            } else {
                let meta = std::fs::metadata(&file_path)
                    .map_err(|e| anyhow!("Cannot open {}: {}", file_path, e))?;
                (meta.len(), None)
            };

            let padded_size = file_size.div_ceil(sector_size) * sector_size;
            let file_offset = current_offset;

            layout_map.insert(path.clone(), (file_offset, file_size));
            write_list.push((path.clone(), prebuilt, file_size, padded_size));

            current_offset += padded_size;

            (file_offset, file_size)
        };

        if emitted >= crate::MAX_PARTS {
            return Err(anyhow!(
                "Too many partitions in package-file (maximum is {})",
                crate::MAX_PARTS
            ));
        }

        // Mutate the slot in place so template bytes in the field tails survive.
        let part = &mut header.parts[emitted];
        write_cstr_preserving_tail(&mut part.name, name);
        write_cstr_preserving_tail(&mut part.full_path, path);

        if let Some(meta) = partition_metadata.get(name) {
            part.flash_size = meta.flash_size;
            part.flash_offset = meta.flash_offset;
        } else {
            // Instead of returning an error, assume it's a special partition
            // with no data and use default values.
            part.flash_size = 0;
            part.flash_offset = 0;
        }

        // padded_size is in 2048-byte sectors and encodes the TRUE length
        // (the vendor's 4.5 GiB userdata stores 0x240000 sectors while
        // part_byte_count wraps at 4 GiB). Compute it from the file actually
        // being packed — metadata from a previous unpack goes stale as soon
        // as a partition is swapped for one of a different size.
        part.padded_size = file_size.div_ceil(sector_size) as u32;

        // Vendor images store offsets beyond 4 GiB modulo 2^32, just like
        // partition byte counts. The unpacker reconstructs the physical
        // offset from package order and the preceding padded allocation.
        part.part_offset = file_offset as u32;
        if file_offset > u32::MAX as u64 {
            println!(
                "note: partition '{}' starts at byte {} (>4 GiB); header stores the low 32 bits (0x{:08x}) as vendor images do",
                name, file_offset, file_offset as u32
            );
        }
        // The on-disk field is 32-bit; vendor images store the low 32 bits
        // for >4 GiB partitions (verified against a real RK3588 firmware).
        part.part_byte_count = file_size as u32;
        if file_size > u32::MAX as u64 {
            println!(
                "note: partition '{}' is {} bytes (>4 GiB); header stores the low 32 bits (0x{:08x}) as vendor images do",
                name, file_size, file_size as u32
            );
        }

        emitted += 1;
    }

    header.num_parts = emitted as u32;
    header.length = current_offset as u32;
    if current_offset > u32::MAX as u64 {
        println!(
            "note: image is {} bytes (>4 GiB); header length stores the low 32 bits (0x{:08x}) as vendor images do",
            current_offset, current_offset as u32
        );
    }

    // Write pass: stream every file through an incremental RockChip CRC so
    // the image is never held in memory and never re-read for the checksum.
    let mut out_file = BufWriter::new(File::create(output_file)?);
    let mut checksum: u32 = 0;
    let emit = |out: &mut BufWriter<File>, crc: &mut u32, data: &[u8]| -> Result<()> {
        out.write_all(data)?;
        *crc = rkcrc32(*crc, data);
        Ok(())
    };

    let encoded_header = header.encode()?;
    emit(&mut out_file, &mut checksum, &encoded_header)?;
    if data_start > header_size {
        emit(&mut out_file, &mut checksum, &vec![0u8; (data_start - header_size) as usize])?;
    }

    let mut chunk = vec![0u8; 4 * 1024 * 1024];
    for (path, prebuilt, file_size, padded_size) in &write_list {
        match prebuilt {
            Some(data) => emit(&mut out_file, &mut checksum, data)?,
            None => {
                let file_path = format!("{}/{}", input_dir, path);
                let mut fp = File::open(&file_path)
                    .map_err(|e| anyhow!("Cannot open {}: {}", file_path, e))?;
                let mut written: u64 = 0;
                loop {
                    let n = fp.read(&mut chunk)?;
                    if n == 0 {
                        break;
                    }
                    emit(&mut out_file, &mut checksum, &chunk[..n])?;
                    written += n as u64;
                }
                if written != *file_size {
                    return Err(anyhow!(
                        "{} changed size while packing ({} bytes read, {} expected)",
                        file_path, written, file_size
                    ));
                }
            }
        }

        let padding = (padded_size - file_size) as usize;
        if padding > 0 {
            emit(&mut out_file, &mut checksum, &vec![0u8; padding])?;
        }
    }

    out_file.write_all(&checksum.to_le_bytes())?;
    out_file.flush()?;

    let num_parts = header.num_parts;

    println!("Successfully packed RKAF image:");
    println!("  Output: {}", output_file);
    println!("  Model: {}", model);
    println!("  Manufacturer: {}", manufacturer);
    println!("  Parts: {}", num_parts);
    println!("  Total size: {} bytes", current_offset);

    Ok(())
}
