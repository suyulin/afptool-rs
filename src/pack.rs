use std::fs::File;
use std::io::{Read, Write, BufRead, BufReader};
use std::collections::HashMap;
use anyhow::{anyhow, Result};
use chrono::{Datelike, Timelike};
use crate::{UpdateHeader, UpdatePart, MAX_NAME_LEN, MAX_FULL_PATH_LEN, RKFW_SIGNATURE, RKAF_SIGNATURE};

#[derive(Debug, Clone)]
struct PartitionMetadata {
    flash_size: u32,
    flash_offset: u32,
    padded_size: u32,
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
            let padded_size = u32::from_str_radix(parts[5].trim_start_matches("0x"), 16)?;

            metadata_map.insert(name, PartitionMetadata {
                flash_size,
                flash_offset,
                padded_size,
            });
        }
    }

    Ok(metadata_map)
}

pub fn pack_rkfw(input_dir: &str, output_file: &str, chip: &str, version: &str, timestamp: i64, code_hex: &str) -> Result<()> {
    let hex_str = code_hex.trim_start_matches("0x").trim_start_matches("0X");
    let code_value = u32::from_str_radix(hex_str, 16)
        .map_err(|_| anyhow!("Invalid hex value for code field: {}", hex_str))?;

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

    let chip_code = chip_name_to_code(chip)?;

    let boot_path = format!("{}/BOOT", input_dir);
    let update_path = format!("{}/embedded-update.img", input_dir);

    let mut boot_data = Vec::new();
    File::open(&boot_path)
        .map_err(|_| anyhow!("Cannot find BOOT file in {}", input_dir))?
        .read_to_end(&mut boot_data)?;

    let mut update_data = Vec::new();
    File::open(&update_path)
        .map_err(|_| anyhow!("Cannot find embedded-update.img file in {}", input_dir))?
        .read_to_end(&mut update_data)?;

    if update_data.len() < 4 || &update_data[0..4] != b"RKAF" {
        return Err(anyhow!("embedded-update.img must be a valid RKAF file"));
    }

    let header_size = 0x66;
    let boot_offset = header_size;
    let boot_size = boot_data.len() as u32;
    let update_offset = boot_offset + boot_size;
    let update_size = update_data.len() as u32;

    let mut header = vec![0u8; header_size as usize];

    header[0..4].copy_from_slice(RKFW_SIGNATURE);

    header[0x04] = header_size as u8;

    header[6] = (build & 0xFF) as u8;
    header[7] = ((build >> 8) & 0xFF) as u8;
    header[8] = minor;
    header[9] = major;

    let code_bytes = code_value.to_le_bytes();
    header[0x0a] = code_bytes[0];
    header[0x0b] = code_bytes[1];
    header[0x0c] = code_bytes[2];
    header[0x0d] = code_bytes[3];

    let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .naive_utc();

    let year = datetime.year() as u16;
    let month = datetime.month() as u8;
    let day = datetime.day() as u8;
    let hour = datetime.hour() as u8;
    let minute = datetime.minute() as u8;
    let second = datetime.second() as u8;

    header[0x0e] = (year & 0xFF) as u8;
    header[0x0f] = ((year >> 8) & 0xFF) as u8;
    header[0x10] = month;
    header[0x11] = day;
    header[0x12] = hour;
    header[0x13] = minute;
    header[0x14] = second;

    header[0x15] = chip_code;

    let chip_digits: Vec<u8> = chip.chars()
        .filter(|c| c.is_numeric())
        .map(|c| c as u8)
        .collect();

    if chip_digits.len() >= 3 {
        header[0x16] = chip_digits[2];
        header[0x17] = chip_digits[1];
        header[0x18] = chip_digits[0];
    }

    put_u32_le(&mut header[0x19..], boot_offset);
    put_u32_le(&mut header[0x1d..], boot_size);

    put_u32_le(&mut header[0x21..], update_offset);
    put_u32_le(&mut header[0x25..], update_size);

    // Padding
    header[0x2d] = 0x01;

    let mut file_data = Vec::new();
    file_data.extend_from_slice(&header);
    file_data.extend_from_slice(&boot_data);
    file_data.extend_from_slice(&update_data);

    let digest = md5::compute(&file_data);
    let md5_hex = format!("{:x}", digest);

    let mut out_file = File::create(output_file)?;
    out_file.write_all(&file_data)?;
    out_file.write_all(md5_hex.as_bytes())?;

    let total_size = file_data.len() + md5_hex.len();

    println!("Successfully packed RKFW image:");
    println!("  Output: {}", output_file);
    println!("  Version: {}.{}.{}", major, minor, build);
    println!("  Date: {}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hour, minute, second);
    println!("  Chip: {} (code: 0x{:02x})", chip, chip_code);
    println!("  BOOT size: {} bytes", boot_size);
    println!("  Update image size: {} bytes", update_size);
    println!("  MD5: {}", md5_hex);
    println!("  Total size: {} bytes", total_size);

    Ok(())
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

fn put_u32_le(slice: &mut [u8], value: u32) {
    let bytes = value.to_le_bytes();
    slice[0] = bytes[0];
    slice[1] = bytes[1];
    slice[2] = bytes[2];
    slice[3] = bytes[3];
}

pub fn pack_rkaf(input_dir: &str, output_file: &str, model: &str, manufacturer: &str) -> Result<()> {
    let package_file_path = format!("{}/package-file", input_dir);
    let package_file = File::open(&package_file_path)
        .map_err(|_| anyhow!("Cannot find package-file in {}", input_dir))?;

    let reader = BufReader::new(package_file);
    let mut file_list = Vec::new();

    for line in reader.lines() {
        let line = line?;
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
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.starts_with("MACHINE_ID:") {
                    machine_id = line.split(':').nth(1).unwrap_or("").trim().to_string();
                    break;
                }
            }
        }
    }

    let mut header = UpdateHeader::default();
    header.magic.copy_from_slice(RKAF_SIGNATURE);

    let model_str = if model.starts_with(' ') {
        model.to_string()
    } else {
        format!(" {}", model)
    };
    let model_bytes = model_str.as_bytes();
    let len = model_bytes.len().min(header.model.len() - 1);
    header.model[..len].copy_from_slice(&model_bytes[..len]);

    let manufacturer_str = if manufacturer.starts_with(' ') {
        manufacturer.to_string()
    } else {
        format!(" {}", manufacturer)
    };
    let manufacturer_bytes = manufacturer_str.as_bytes();
    let len = manufacturer_bytes.len().min(header.manufacturer.len() - 1);
    header.manufacturer[..len].copy_from_slice(&manufacturer_bytes[..len]);

    if !machine_id.is_empty() {
        let id_bytes = format!(" {}", machine_id);
        let id_bytes = id_bytes.as_bytes();
        let len = id_bytes.len().min(30 - 1); // MAX_ID_LEN is 30
        unsafe {
            let header_ptr = &header as *const UpdateHeader as *const u8;
            let id_offset = 42; // id field offset in UpdateHeader
            let id_ptr = header_ptr.add(id_offset) as *mut u8;
            std::ptr::copy_nonoverlapping(id_bytes.as_ptr(), id_ptr, len);
        }
    }

    header.num_parts = file_list.len() as u32;
    header.version = 0x01000000; // Version

    let partition_metadata = parse_partition_metadata(input_dir)?;
    if partition_metadata.is_empty() {
        return Err(anyhow!("Missing partition metadata"));
    }

    let header_size = std::mem::size_of::<UpdateHeader>();
    let sector_size = 2048;
    let mut current_offset = ((header_size + sector_size - 1) / sector_size) * sector_size;

    let mut file_data_map: HashMap<String, (Vec<u8>, u32, u32)> = HashMap::new();
    let mut file_data_list = Vec::new();

    for (i, (name, path)) in file_list.iter().enumerate() {
        let (file_offset, file_size, _padded_size) = if path == "SELF" || path == "RESERVED" {
            (0, 0, 0)
        } else if let Some((data, offset, padded)) = file_data_map.get(path) {
            // File already loaded, reuse offset
            (*offset, data.len() as u32, *padded)
        } else {
            // Load file for first time
            let file_path = format!("{}/{}", input_dir, path);
            let mut raw_data = Vec::new();

            File::open(&file_path)
                .map_err(|e| anyhow!("Cannot open {}: {}", file_path, e))?
                .read_to_end(&mut raw_data)?;

            // "parameter" partitions are stored PARM-wrapped in the image
            let file_data = if name == "parameter" {
                let content_len = raw_data.len() as u32;
                let mut wrapped = Vec::with_capacity(raw_data.len() + 12);
                wrapped.extend_from_slice(b"PARM");
                wrapped.extend_from_slice(&content_len.to_le_bytes());
                wrapped.extend_from_slice(&raw_data);
                let crc = rkcrc32(0, &raw_data);
                wrapped.extend_from_slice(&crc.to_le_bytes());
                wrapped
            } else {
                raw_data
            };

            let file_size = file_data.len() as u32;
            let padded_size = ((file_size + sector_size as u32 - 1) / sector_size as u32) * sector_size as u32;
            let file_offset = current_offset as u32;

            file_data_map.insert(path.clone(), (file_data.clone(), file_offset, padded_size));
            file_data_list.push((path.clone(), file_data));

            current_offset += padded_size as usize;

            (file_offset, file_size, padded_size)
        };

        let mut part = UpdatePart::default();

        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(MAX_NAME_LEN - 1);
        part.name[..len].copy_from_slice(&name_bytes[..len]);

        let path_bytes = path.as_bytes();
        let len = path_bytes.len().min(MAX_FULL_PATH_LEN - 1);
        part.full_path[..len].copy_from_slice(&path_bytes[..len]);

        if let Some(meta) = partition_metadata.get(name) {
            part.flash_size = meta.flash_size;
            part.flash_offset = meta.flash_offset;
            part.padded_size = meta.padded_size;
        } else {
            // Instead of returning an error, assume it's a special partition
            // with no data and use default values.
            part.flash_size = 0;
            part.flash_offset = 0;
            part.padded_size = 0;
        }

        part.part_offset = file_offset;
        part.part_byte_count = file_size;

        header.parts[i] = part;
    }

    header.length = current_offset as u32;

    let mut out_file = File::create(output_file)?;

    out_file.write_all(header.to_bytes())?;

    let header_padding = sector_size - header_size;
    out_file.write_all(&vec![0u8; header_padding])?;

    for (path, file_data) in file_data_list.iter() {
        out_file.write_all(file_data)?;

        // Pad file
        let (_data, _offset, padded_size) = file_data_map.get(path).unwrap();
        let padding_size = *padded_size as usize - file_data.len();
        if padding_size > 0 {
            out_file.write_all(&vec![0u8; padding_size])?;
        }
    }

    let file_content = std::fs::read(output_file)?;
    let checksum = rkcrc32(0, &file_content);

    let mut out_file = std::fs::OpenOptions::new()
        .append(true)
        .open(output_file)?;
    out_file.write_all(&checksum.to_le_bytes())?;

    let num_parts = header.num_parts;

    println!("Successfully packed RKAF image:");
    println!("  Output: {}", output_file);
    println!("  Model: {}", model);
    println!("  Manufacturer: {}", manufacturer);
    println!("  Parts: {}", num_parts);
    println!("  Total size: {} bytes", current_offset);

    Ok(())
}
