use anyhow::{anyhow, bail, Result};

pub const SSFW_SIGNATURE: &[u8] = b"SSFW";
pub const SSFW_HEADER_SIZE: usize = 0x40;
pub const SSFW_ENTRY_SIZE: usize = 0x10;
pub const RKFW_HEADER_SIZE: usize = 0x66;
pub const RKFW_STORAGE_OFFSET: usize = 0x62;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsfwHeader {
    pub magic: [u8; 4],
    pub header_size: u32,
    pub version: u32,
    pub merge_version: u32,
    pub image_count: u32,
    pub reserved: [u8; 44],
}

impl Default for SsfwHeader {
    fn default() -> Self {
        Self {
            magic: *b"SSFW",
            header_size: SSFW_HEADER_SIZE as u32,
            version: 0x10,
            merge_version: 0,
            image_count: 0,
            reserved: [0; 44],
        }
    }
}

impl SsfwHeader {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < SSFW_HEADER_SIZE {
            bail!(
                "SSFW header requires at least {SSFW_HEADER_SIZE} bytes, got {}",
                bytes.len()
            );
        }

        let mut header = Self::default();
        header.magic.copy_from_slice(&bytes[0..4]);
        if header.magic != SSFW_SIGNATURE {
            bail!("invalid SSFW signature");
        }
        header.header_size = read_u32_le(bytes, 4)?;
        header.version = read_u32_le(bytes, 8)?;
        header.merge_version = read_u32_le(bytes, 12)?;
        header.image_count = read_u32_le(bytes, 16)?;
        header
            .reserved
            .copy_from_slice(&bytes[20..SSFW_HEADER_SIZE]);

        if header.header_size < SSFW_HEADER_SIZE as u32 {
            bail!(
                "SSFW header size is {}, minimum is {SSFW_HEADER_SIZE}",
                header.header_size
            );
        }

        Ok(header)
    }

    pub fn encode(&self) -> Result<[u8; SSFW_HEADER_SIZE]> {
        if self.magic != SSFW_SIGNATURE {
            bail!("invalid SSFW signature");
        }
        if self.header_size != SSFW_HEADER_SIZE as u32 {
            bail!(
                "cannot encode SSFW header size {}; only {SSFW_HEADER_SIZE} is supported",
                self.header_size
            );
        }

        let mut bytes = [0u8; SSFW_HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..8].copy_from_slice(&self.header_size.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.merge_version.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.image_count.to_le_bytes());
        bytes[20..SSFW_HEADER_SIZE].copy_from_slice(&self.reserved);
        Ok(bytes)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SsfwEntry {
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug)]
pub struct SsfwImage<'data> {
    data: &'data [u8],
    header: SsfwHeader,
    entries: Vec<SsfwEntry>,
}

#[derive(Copy, Clone, Debug)]
pub struct SsfwChild<'data> {
    pub entry: SsfwEntry,
    pub data: &'data [u8],
    pub storage_mask: u32,
    pub storage_code: Option<u8>,
}

impl<'data> SsfwImage<'data> {
    pub fn new(data: &'data [u8]) -> Result<Self> {
        let header = SsfwHeader::decode(data)?;
        let header_size = usize::try_from(header.header_size)
            .map_err(|_| anyhow!("SSFW header size does not fit in usize"))?;
        let table_size = (header.image_count as usize)
            .checked_mul(SSFW_ENTRY_SIZE)
            .ok_or_else(|| anyhow!("SSFW entry table size overflows usize"))?;
        let table_end = header_size
            .checked_add(table_size)
            .ok_or_else(|| anyhow!("SSFW entry table range overflows usize"))?;
        let table = data.get(header_size..table_end).ok_or_else(|| {
            anyhow!(
                "SSFW entry table ends at {}, beyond file size {}",
                table_end,
                data.len()
            )
        })?;
        let entries = table
            .chunks_exact(SSFW_ENTRY_SIZE)
            .map(SsfwEntry::decode)
            .collect::<Result<Vec<_>>>()?;
        validate_entries(&entries, table_end as u64, data.len() as u64)?;

        for (index, entry) in entries.iter().copied().enumerate() {
            let start = usize::try_from(entry.offset)
                .map_err(|_| anyhow!("SSFW image {index} offset does not fit in usize"))?;
            let end = usize::try_from(entry.end()?)
                .map_err(|_| anyhow!("SSFW image {index} end does not fit in usize"))?;
            let child = data
                .get(start..end)
                .ok_or_else(|| anyhow!("SSFW image {index} range is out of bounds"))?;
            rkfw_storage_mask(child)?;
        }

        Ok(Self {
            data,
            header,
            entries,
        })
    }

    pub fn header(&self) -> &SsfwHeader {
        &self.header
    }

    pub fn entries(&self) -> &[SsfwEntry] {
        &self.entries
    }

    pub fn child(&self, index: usize) -> Result<Option<SsfwChild<'data>>> {
        let Some(entry) = self.entries.get(index).copied() else {
            return Ok(None);
        };
        let start = usize::try_from(entry.offset)
            .map_err(|_| anyhow!("SSFW image {index} offset does not fit in usize"))?;
        let end = usize::try_from(entry.end()?)
            .map_err(|_| anyhow!("SSFW image {index} end does not fit in usize"))?;
        let data = self
            .data
            .get(start..end)
            .ok_or_else(|| anyhow!("SSFW image {index} range is out of bounds"))?;
        let mask = rkfw_storage_mask(data)?;
        Ok(Some(SsfwChild {
            entry,
            data,
            storage_mask: mask,
            storage_code: storage_code(mask),
        }))
    }

    pub fn children(&self) -> impl Iterator<Item = Result<SsfwChild<'data>>> + '_ {
        (0..self.entries.len()).map(|index| {
            self.child(index)?
                .ok_or_else(|| anyhow!("SSFW image {index} disappeared"))
        })
    }
}

impl SsfwEntry {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < SSFW_ENTRY_SIZE {
            bail!(
                "SSFW entry requires {SSFW_ENTRY_SIZE} bytes, got {}",
                bytes.len()
            );
        }
        Ok(Self {
            offset: read_u64_le(bytes, 0)?,
            size: read_u64_le(bytes, 8)?,
        })
    }

    pub fn encode(self) -> [u8; SSFW_ENTRY_SIZE] {
        let mut bytes = [0u8; SSFW_ENTRY_SIZE];
        bytes[0..8].copy_from_slice(&self.offset.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.size.to_le_bytes());
        bytes
    }

    pub fn end(self) -> Result<u64> {
        self.offset
            .checked_add(self.size)
            .ok_or_else(|| anyhow!("SSFW image range overflows u64"))
    }
}

/// Decode the storage bit mask written by rkImageMaker at the end of a
/// 0x66-byte RKFW header.
pub fn rkfw_storage_mask(rkfw_header: &[u8]) -> Result<u32> {
    if rkfw_header.len() < RKFW_HEADER_SIZE {
        bail!(
            "RKFW header requires {RKFW_HEADER_SIZE} bytes, got {}",
            rkfw_header.len()
        );
    }
    if &rkfw_header[0..4] != b"RKFW" {
        bail!("SSFW child does not have an RKFW signature");
    }
    read_u32_le(rkfw_header, RKFW_STORAGE_OFFSET)
}

/// Convert a one-hot RKFW storage mask to the RockUSB storage selector used by
/// the CHANGE_STORAGE command. For example, 0x200 becomes 9 (SPI NOR) and
/// 0x800 becomes 11 (PCIe/NVMe).
pub fn storage_code(storage_mask: u32) -> Option<u8> {
    (storage_mask.count_ones() == 1).then(|| storage_mask.trailing_zeros() as u8)
}

pub fn storage_name(code: u8) -> &'static str {
    match code {
        0 => "flash",
        1 => "emmc",
        2 => "sd",
        9 => "spinor",
        10 => "spinand",
        11 => "pcie",
        _ => "unknown",
    }
}

pub(crate) fn validate_entries(
    entries: &[SsfwEntry],
    table_end: u64,
    file_size: u64,
) -> Result<()> {
    let mut ranges = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().copied().enumerate() {
        let end = entry.end()?;
        if entry.offset < table_end {
            bail!(
                "SSFW image {index} starts at {}, inside the header or entry table ending at {}",
                entry.offset,
                table_end
            );
        }
        if end > file_size {
            bail!(
                "SSFW image {index} range {}..{} exceeds file size {}",
                entry.offset,
                end,
                file_size
            );
        }
        if entry.size < RKFW_HEADER_SIZE as u64 {
            bail!(
                "SSFW image {index} is too small for an RKFW header: {} bytes",
                entry.size
            );
        }
        ranges.push((entry.offset, end, index));
    }

    ranges.sort_unstable_by_key(|&(start, _, _)| start);
    for pair in ranges.windows(2) {
        let (_, previous_end, previous_index) = pair[0];
        let (next_start, _, next_index) = pair[1];
        if previous_end > next_start {
            bail!("SSFW images {previous_index} and {next_index} overlap");
        }
    }
    Ok(())
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow!("u32 field at offset {offset:#x} is out of range"))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

fn read_u64_le(bytes: &[u8], offset: usize) -> Result<u64> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| anyhow!("u64 field at offset {offset:#x} is out of range"))?;
    Ok(u64::from_le_bytes(value.try_into().unwrap()))
}
