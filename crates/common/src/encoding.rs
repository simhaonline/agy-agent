use crate::error::CommonError;

pub fn encode_latin1(s: &str) -> Result<Vec<u8>, CommonError> {
    let mut bytes = Vec::with_capacity(s.len());
    for ch in s.chars() {
        if ch as u32 > 0xFF {
            return Err(CommonError::InvalidEncoding(format!(
                "Character '{}' (U+{:04X}) cannot be encoded in Latin-1",
                ch, ch as u32
            )));
        }
        bytes.push(ch as u8);
    }
    Ok(bytes)
}

pub fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

pub fn normalize_replacement(search_len: usize, replacement: &[u8]) -> Vec<u8> {
    if replacement.len() == search_len {
        replacement.to_vec()
    } else if replacement.len() < search_len {
        let mut out = Vec::with_capacity(search_len);
        out.extend_from_slice(replacement);
        out.resize(search_len, b' ');
        out
    } else {
        replacement[..search_len].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latin1_encode_decode() {
        let s = "Hello, world! © 2026";
        let encoded = encode_latin1(s).unwrap();
        let decoded = decode_latin1(&encoded);
        assert_eq!(s, decoded);
    }

    #[test]
    fn test_latin1_reject_unicode() {
        let s = "Hello 🚀";
        assert!(encode_latin1(s).is_err());
    }

    #[test]
    fn test_normalize_replacement() {
        assert_eq!(normalize_replacement(5, b"abc"), b"abc  ");
        assert_eq!(normalize_replacement(5, b"abcdefg"), b"abcde");
        assert_eq!(normalize_replacement(5, b"12345"), b"12345");
    }
}
