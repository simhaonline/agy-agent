use common::error::CommonError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeSection {
    pub name: String,
    pub virtual_size: u32,
    pub virtual_address: u32,
    pub raw_data_size: u32,
    pub raw_data_pointer: u32,
    pub characteristics: u32,
    pub is_code: bool,
    pub is_writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeInspection {
    pub is_pe: bool,
    pub is_64bit: bool,
    pub machine: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub entry_point: u32,
    pub image_base: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub checksum: u32,
    pub subsystem: u16,
    pub has_authenticode_table: bool,
    pub authenticode_size: u32,
    pub sections: Vec<PeSection>,
}

impl PeInspection {
    pub fn parse(bytes: &[u8]) -> Result<Self, CommonError> {
        if bytes.len() < 64 {
            return Err(CommonError::Validation("Buffer too small for DOS header".into()));
        }

        // 1. DOS Header Check
        if bytes[0] != b'M' || bytes[1] != b'Z' {
            return Err(CommonError::Validation("Not a valid PE: MZ signature missing".into()));
        }

        let e_lfanew = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
        if e_lfanew + 4 > bytes.len() {
            return Err(CommonError::Validation("PE signature offset beyond file bounds".into()));
        }

        // 2. PE Signature Check ("PE\0\0")
        if &bytes[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
            return Err(CommonError::Validation("Not a valid PE: PE00 signature missing".into()));
        }

        let coff_offset = e_lfanew + 4;
        if coff_offset + 20 > bytes.len() {
            return Err(CommonError::Validation("Buffer truncated at COFF header".into()));
        }

        let machine = u16::from_le_bytes(bytes[coff_offset..coff_offset + 2].try_into().unwrap());
        let number_of_sections = u16::from_le_bytes(bytes[coff_offset + 2..coff_offset + 4].try_into().unwrap());
        let time_date_stamp = u32::from_le_bytes(bytes[coff_offset + 4..coff_offset + 8].try_into().unwrap());
        let size_of_optional_header = u16::from_le_bytes(bytes[coff_offset + 16..coff_offset + 18].try_into().unwrap()) as usize;

        let opt_offset = coff_offset + 20;
        if opt_offset + size_of_optional_header > bytes.len() {
            return Err(CommonError::Validation("Buffer truncated at Optional header".into()));
        }

        let opt_magic = u16::from_le_bytes(bytes[opt_offset..opt_offset + 2].try_into().unwrap());
        let is_64bit = opt_magic == 0x020B; // PE32+ (64-bit)

        let entry_point = u32::from_le_bytes(bytes[opt_offset + 16..opt_offset + 20].try_into().unwrap());
        let (image_base, size_of_image, size_of_headers, checksum, subsystem, security_rva, security_size) = if is_64bit {
            let base = u64::from_le_bytes(bytes[opt_offset + 24..opt_offset + 32].try_into().unwrap());
            let img_size = u32::from_le_bytes(bytes[opt_offset + 56..opt_offset + 60].try_into().unwrap());
            let hdr_size = u32::from_le_bytes(bytes[opt_offset + 60..opt_offset + 64].try_into().unwrap());
            let csum = u32::from_le_bytes(bytes[opt_offset + 64..opt_offset + 68].try_into().unwrap());
            let sub = u16::from_le_bytes(bytes[opt_offset + 68..opt_offset + 70].try_into().unwrap());
            let sec_rva = if size_of_optional_header >= 144 {
                u32::from_le_bytes(bytes[opt_offset + 128..opt_offset + 132].try_into().unwrap())
            } else { 0 };
            let sec_sz = if size_of_optional_header >= 144 {
                u32::from_le_bytes(bytes[opt_offset + 132..opt_offset + 136].try_into().unwrap())
            } else { 0 };
            (base, img_size, hdr_size, csum, sub, sec_rva, sec_sz)
        } else {
            let base = u32::from_le_bytes(bytes[opt_offset + 28..opt_offset + 32].try_into().unwrap()) as u64;
            let img_size = u32::from_le_bytes(bytes[opt_offset + 56..opt_offset + 60].try_into().unwrap());
            let hdr_size = u32::from_le_bytes(bytes[opt_offset + 60..opt_offset + 64].try_into().unwrap());
            let csum = u32::from_le_bytes(bytes[opt_offset + 64..opt_offset + 68].try_into().unwrap());
            let sub = u16::from_le_bytes(bytes[opt_offset + 68..opt_offset + 70].try_into().unwrap());
            let sec_rva = if size_of_optional_header >= 128 {
                u32::from_le_bytes(bytes[opt_offset + 128..opt_offset + 132].try_into().unwrap())
            } else { 0 };
            let sec_sz = if size_of_optional_header >= 128 {
                u32::from_le_bytes(bytes[opt_offset + 132..opt_offset + 136].try_into().unwrap())
            } else { 0 };
            (base, img_size, hdr_size, csum, sub, sec_rva, sec_sz)
        };

        // Section Headers (40 bytes each)
        let section_table_offset = opt_offset + size_of_optional_header;
        let mut sections = Vec::with_capacity(number_of_sections as usize);

        for i in 0..number_of_sections as usize {
            let sec_off = section_table_offset + i * 40;
            if sec_off + 40 > bytes.len() {
                break;
            }
            let name_bytes = &bytes[sec_off..sec_off + 8];
            let name_str = String::from_utf8_lossy(name_bytes)
                .trim_end_matches('\0')
                .to_string();

            let virtual_size = u32::from_le_bytes(bytes[sec_off + 8..sec_off + 12].try_into().unwrap());
            let virtual_address = u32::from_le_bytes(bytes[sec_off + 12..sec_off + 16].try_into().unwrap());
            let raw_data_size = u32::from_le_bytes(bytes[sec_off + 16..sec_off + 20].try_into().unwrap());
            let raw_data_pointer = u32::from_le_bytes(bytes[sec_off + 20..sec_off + 24].try_into().unwrap());
            let characteristics = u32::from_le_bytes(bytes[sec_off + 36..sec_off + 40].try_into().unwrap());

            let is_code = (characteristics & 0x00000020) != 0; // IMAGE_SCN_CNT_CODE
            let is_writable = (characteristics & 0x80000000) != 0; // IMAGE_SCN_MEM_WRITE

            sections.push(PeSection {
                name: name_str,
                virtual_size,
                virtual_address,
                raw_data_size,
                raw_data_pointer,
                characteristics,
                is_code,
                is_writable,
            });
        }

        Ok(PeInspection {
            is_pe: true,
            is_64bit,
            machine,
            number_of_sections,
            time_date_stamp,
            entry_point,
            image_base,
            size_of_image,
            size_of_headers,
            checksum,
            subsystem,
            has_authenticode_table: security_rva != 0 && security_size > 0,
            authenticode_size: security_size,
            sections,
        })
    }

    pub fn find_section_for_offset(&self, offset: usize) -> Option<&PeSection> {
        let off_u32 = offset as u32;
        self.sections.iter().find(|s| {
            off_u32 >= s.raw_data_pointer && off_u32 < s.raw_data_pointer + s.raw_data_size
        })
    }

    pub fn is_in_code_section(&self, offset: usize) -> bool {
        if let Some(sec) = self.find_section_for_offset(offset) {
            sec.is_code || sec.name == ".text"
        } else {
            false
        }
    }
}
