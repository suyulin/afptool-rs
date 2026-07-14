use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::Path;
use anyhow::{anyhow, Result};
use chrono::NaiveDateTime;
use crate::{RKAF_SIGNATURE, RKFW_SIGNATURE, UpdateHeader, RKAFP_MAGIC, UPDATE_HEADER_SIZE};

pub fn unpack_file(file_path: &str, dst_path: &str) -> Result<()> {
    let mut file = File::open(file_path)?;
    let mut signature = [0u8; 4];
    file.read_exact(&mut signature)?;

    match &signature[..] {
        RKAF_SIGNATURE => unpack_rkafp(file_path, dst_path)?,
        RKFW_SIGNATURE => unpack_rkfw(file_path, dst_path)?,
        _ => {
            return Err(anyhow!("Unknown signature: {:?}", signature));
        }
    }
    Ok(())
}

/// Decode the modern chip identity field at RKFW offsets 0x15..0x19: the four
/// ASCII digits of the chip number stored last-digit-first (a vendor RK3588
/// image carries '8' '8' '5' '3'). Returns None for legacy one-byte
/// family-code encodings, which are resolved via the family table instead.
pub fn decode_chip_field(field: &[u8; 4]) -> Option<String> {
    if field.iter().all(|b| b.is_ascii_digit()) {
        let digits: String = field.iter().rev().map(|&b| b as char).collect();
        Some(format!("RK{}", digits))
    } else {
        None
    }
}

fn unpack_rkfw(file_path: &str, dst_path: &str) -> Result<()> {
    const HEADER_SIZE: usize = 0x66;

    let mut fp = File::open(file_path)?;
    let filesize = fp.metadata()?.len();
    let mut buf = [0u8; HEADER_SIZE];
    fp.read_exact(&mut buf)?;

    let mut chip: Option<&str> = None;

    println!("RKFW signature detected");

    let version_str = format!(
        "{}.{}.{}",
        buf[9],
        buf[8],
        ((buf[7] as u16) << 8) + buf[6] as u16
    );
    println!("version: {}", version_str);

    let code = u32::from_le_bytes([buf[0x0a], buf[0x0b], buf[0x0c], buf[0x0d]]);
    println!("code field: 0x{:08x}", code);

    let year = ((buf[0x0f] as u16) << 8) | (buf[0x0e] as u16);
    let month = buf[0x10];
    let day = buf[0x11];
    let hour = buf[0x12];
    let minute = buf[0x13];
    let second = buf[0x14];

    // The date is informational only; a malformed field must not stop extraction.
    let date = chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32);
    let time = chrono::NaiveTime::from_hms_opt(hour as u32, minute as u32, second as u32);
    match (date, time) {
        (Some(date), Some(time)) => {
            let unix_timestamp = NaiveDateTime::new(date, time).and_utc().timestamp();
            println!(
                "date: {}-{:02}-{:02} {:02}:{:02}:{:02} (Unix timestamp: {})",
                year, month, day, hour, minute, second, unix_timestamp
            );
        }
        _ => println!(
            "date: {}-{:02}-{:02} {:02}:{:02}:{:02} (invalid)",
            year, month, day, hour, minute, second
        ),
    }

    // Modern images carry the chip number as four ASCII digits; legacy images
    // carry a one-byte family code at 0x15.
    let chip_field: [u8; 4] = buf[0x15..0x19].try_into().unwrap();
    let decoded_chip = decode_chip_field(&chip_field);

    if decoded_chip.is_none() {
        match buf[0x15] {
            0x19 => chip = Some("RV1109/RV1126"),
            0x30 => chip = Some("PX30/RK3326"),
            0x32 => chip = Some("RK3562"),
            0x33 => chip = Some("RK3399/RK3399Pro"),
            0x35 => chip = Some("RK3588/RK3588S"),
            0x36 => chip = Some("RK3326"),
            0x38 => chip = Some("RK3566/RK3568"),
            0x39 => chip = Some("RK3528"),
            0x41 => chip = Some("RK3368"),
            0x48 => chip = Some("RK3308"),
            0x50 => chip = Some("RK29xx"),
            0x51 => chip = Some("RV1108"),
            0x60 => chip = Some("RK30xx/RK3066"),
            0x70 => chip = Some("RK31xx/RK3188"),
            0x80 => chip = Some("RK32xx/RK3288"),
            _ => println!(
                "You got a brand new chip ({:#x}), congratulations!!!",
                buf[0x15]
            ),
        }
    }

    let chip_name = decoded_chip.as_deref().or(chip).unwrap_or("unknown");
    println!("family: {}", chip_name);

    std::fs::create_dir_all(dst_path)?;

    // Keep the original header so pack-rkfw can use it as a template and
    // preserve fields that are otherwise lost (timestamp, code, unknown bytes).
    write_file(Path::new(&format!("{}/rkfw-header.bin", dst_path)), &buf)?;

    let ioff = get_u32_le(&buf[0x19..]) as u64;
    let isize = get_u32_le(&buf[0x1d..]) as u64;

    println!(
        "{:08x}-{:08x} {:26} (size: {})",
        ioff,
        ioff + isize - 1,
        "BOOT",
        isize
    );
    extract_file(&mut fp, ioff, isize, &format!("{}/BOOT", dst_path))?;

    let ioff = get_u32_le(&buf[0x21..]) as u64;
    let stored_size = get_u32_le(&buf[0x25..]);

    let mut magic = [0u8; 4];
    fp.seek(std::io::SeekFrom::Start(ioff))?;
    fp.read_exact(&mut magic)?;
    if &magic != b"RKAF" {
        return Err(anyhow!("cannot find embedded RKAF update.img"));
    }

    // The size field only holds the low 32 bits for >4 GiB updates, so derive
    // the true size from the file layout: everything between the update offset
    // and the trailing 32-char ASCII MD5 belongs to the update image.
    let mut data_end = filesize;
    if filesize >= ioff + 32 {
        let mut tail = [0u8; 32];
        fp.seek(std::io::SeekFrom::End(-32))?;
        fp.read_exact(&mut tail)?;
        if tail.iter().all(u8::is_ascii_hexdigit) {
            data_end = filesize - 32;
        }
    }
    let isize = crate::recover_true_size(stored_size, data_end.saturating_sub(ioff));
    if isize != stored_size as u64 {
        println!(
            "note: header size field is 0x{:08x}; recovered true update size {} bytes (>4 GiB, field wrapped)",
            stored_size, isize
        );
    }

    println!(
        "{:08x}-{:08x} {:26} (size: {})",
        ioff,
        ioff + isize - 1,
        "embedded-update.img",
        isize
    );
    extract_file(&mut fp, ioff, isize, &format!("{}/embedded-update.img", dst_path))?;
    Ok(())
}

fn extract_file(fp: &mut File, offset: u64, len: u64, full_path: &str) -> Result<()> {
    println!("{:08x}-{:08x} {}", offset, len, full_path);
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    let mut fp_out = File::create(full_path)?;

    fp.seek(std::io::SeekFrom::Start(offset))?;

    let mut remaining = len;

    while remaining > 0 {
        let read_len = std::cmp::min(remaining as usize, buffer.len());
        let read_bytes = fp.read(&mut buffer[..read_len])?;

        if read_bytes != read_len {
            return Err(anyhow!("Insufficient length in container image file"));
        }

        fp_out.write_all(&buffer[..read_len])?;

        remaining -= read_len as u64;
    }

    Ok(())
}

// Returns (data_offset, data_len) for the partition content, stripping the PARM
// wrapper (4-byte magic + 4-byte content length + content + 4-byte CRC) for
// partitions named "parameter", matching the behaviour of the reference rkafpack.c.
fn parm_content_range(name: &str, fp: &mut File, offset: u64, len: u64) -> Result<(u64, u64)> {
    const PARM_OVERHEAD: u64 = 12; // 4 magic + 4 length + 4 CRC
    if name != "parameter" || len < PARM_OVERHEAD {
        return Ok((offset, len));
    }
    let mut header = [0u8; 8];
    fp.seek(std::io::SeekFrom::Start(offset))?;
    fp.read_exact(&mut header)?;
    let content_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
    Ok((offset + 8, content_len))
}

fn unpack_rkafp(file_path: &str, dst_path: &str) -> Result<()> {
    let mut fp = File::open(file_path)?;
    let mut buf = [0u8; UPDATE_HEADER_SIZE];
    fp.read_exact(&mut buf)?;
    let header = UpdateHeader::decode(&buf)?;
    let magic_str = std::str::from_utf8(&header.magic)?;
    if magic_str != RKAFP_MAGIC {
        return Err(anyhow!("Invalid header magic id"));
    }

    let filesize = fp.metadata()?.len();
    println!("Filesize: {}", filesize);

    // The length field only stores the low 32 bits, so compare modulo 2^32.
    let container_end = filesize.saturating_sub(4); // trailing CRC
    if (container_end as u32) != header.length {
        eprintln!("update_header.length cannot be correct, cannot check CRC");
    } else if container_end > u32::MAX as u64 {
        println!(
            "note: container is {} bytes (>4 GiB); header length field holds the low 32 bits",
            container_end
        );
    }

    // Offsets of all data-bearing partitions, used to bound each partition's
    // true size when its 32-bit byte count has wrapped.
    let mut data_offsets: Vec<u64> = (0..header.num_parts as usize)
        .map(|i| header.parts[i].part_offset as u64)
        .filter(|&off| off > 0)
        .collect();
    data_offsets.sort_unstable();
    data_offsets.dedup();
    std::fs::create_dir_all(format!("{}/Image", dst_path))?;
    // 安全地从null-terminated字符串中提取文本
    let manufacturer = std::ffi::CStr::from_bytes_until_nul(&header.manufacturer)
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|_| "unknown".into());
    let model = std::ffi::CStr::from_bytes_until_nul(&header.model)
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|_| "unknown".into());

    println!("manufacturer: {}", manufacturer);
    println!("model: {}", model);

    // Keep the original header so pack-rkaf can use it as a template and
    // preserve undocumented bytes (the vendor tool leaves data in the tails
    // of string fields).
    write_file(Path::new(&format!("{}/rkaf-header.bin", dst_path)), &buf)?;

    // Save partition metadata for repacking
    let metadata_path = format!("{}/partition-metadata.txt", dst_path);
    let mut metadata_file = File::create(&metadata_path)?;

    for i in 0..header.num_parts {
        let part = &header.parts[i as usize];
        // 安全地提取路径字符串
        if let Ok(cstr_path) = std::ffi::CStr::from_bytes_until_nul(&part.full_path) {
            let part_full_path = cstr_path.to_string_lossy();
            let part_name = if let Ok(cstr_name) = std::ffi::CStr::from_bytes_until_nul(&part.name) {
                cstr_name.to_string_lossy().to_string()
            } else {
                String::new()
            };

            let flash_size = part.flash_size;
            let flash_offset = part.flash_offset;
            let part_offset = part.part_offset;
            let padded_size = part.padded_size;
            let part_byte_count = part.part_byte_count;

            writeln!(
                metadata_file,
                "{},{},{:#010x},{:#010x},{:#010x},{:#010x},{:#010x}",
                part_name,
                part_full_path,
                flash_size,
                flash_offset,
                part_offset,
                padded_size,
                part_byte_count
            )?;

            if part_full_path == "SELF" || part_full_path == "RESERVED" {
                continue;
            }

            let file_to_extract = format!("{}/{}", dst_path, part_full_path);

            // part_byte_count only stores the low 32 bits; recover the true
            // size using the next partition's offset (or the container end)
            // as an upper bound.
            let offset = part.part_offset as u64;
            let bound = data_offsets
                .iter()
                .find(|&&o| o > offset)
                .copied()
                .unwrap_or(container_end);
            let true_count = crate::recover_true_size(
                part.part_byte_count,
                bound.saturating_sub(offset),
            );
            if true_count != part.part_byte_count as u64 {
                println!(
                    "note: partition '{}' byte count field is 0x{:08x}; recovered true size {} bytes (>4 GiB, field wrapped)",
                    part_name, part_byte_count, true_count
                );
            }

            let (data_offset, data_len) =
                parm_content_range(&part_name, &mut fp, offset, true_count)?;
            extract_file(&mut fp, data_offset, data_len, &file_to_extract)?;
        }
    }

    println!("\nPartition metadata saved to: {}", metadata_path);

    Ok(())
}

fn get_u32_le(slice: &[u8]) -> u32 {
    u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
}

fn write_file(path: &Path, buffer: &[u8]) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(buffer)?;
    Ok(())
}
