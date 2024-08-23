/// Utility functions to decode URIs.

const PERCENT_CODE: u8 = '%' as u8; // ASCII code for '%'


/// Decodes a percent-encoded URI into its corresponding UTF-8 string.
pub fn decode_uri(encoded_uri: &str) -> Result<String, std::string::FromUtf8Error> {

    // Result accumulator
    let mut decoded_bytes = Vec::<u8>::with_capacity(encoded_uri.len());

    // Iterator at '%' boundaries
    let mut chunks = encoded_uri.split('%');

    // First chunk goes directly to the result:
    if let Some(prefix) = chunks.next() {
        decoded_bytes.extend_from_slice(prefix.as_bytes());
    }

    // For the rest of the chunks, attempt to decode a byte by interpreting
    // the first two characters as hex digits, and use that character and the rest of
    // the string. Otherwise re-insert a '%' and the full string.
    for chunk in chunks {
        let (decoded_char, remainder_str) = shift_encoded_hex(chunk);
        decoded_bytes.push(decoded_char);
        decoded_bytes.extend_from_slice(remainder_str.as_bytes());
    }

    String::from_utf8(decoded_bytes)
}


/// If the first two characters of `string` are hex digits, return their numerical value
/// and the rest of the string; otherwise, return the char code for '%' and the full original
/// string.
fn shift_encoded_hex(string: &str) -> (u8, &str) {

    // For a valid encoding, exactly two single-byte characters (later validated as hex digits)
    // are expected
    if !string.is_char_boundary(2) {
        return (PERCENT_CODE, string);
    }
    let (code_str, suffix) = string.split_at(2);

    if let Ok(decoded_byte) = u8::from_str_radix(code_str, 16) {
        (decoded_byte, suffix)
    }
    else {
        (PERCENT_CODE, string)
    }
}


#[cfg(test)]
mod tests {
    use crate::uri::decode_uri;
    
    macro_rules! check_decode {
        ($encoded:literal, $decoded:literal) => {
            assert_eq!( decode_uri($encoded), Ok(String::from($decoded)) );
        };
    }

    #[test]
    fn test_decode_invariants() {
        check_decode!("",               "");
        check_decode!("foo",            "foo");
        check_decode!("%",              "%");
        check_decode!("%xyz",           "%xyz");
        check_decode!("address%",       "address%");
        check_decode!("20% discount",   "20% discount");
        check_decode!("100%!",          "100%!");
    }

    #[test]
    fn test_decode_single_byte() {
        check_decode!("%20",                " ");
        check_decode!("two%20words",        "two words");
        check_decode!("two%20%20spaces",    "two  spaces");
        check_decode!("%40label",           "@label");
        check_decode!("label%40",           "label@");
    }

    #[test]
    fn test_decode_multi_byte() {
        check_decode!("%C2%A3",             "£");
        check_decode!("Price: %C2%A357",    "Price: £57");
        check_decode!("%E2%82%AC",          "€");
        check_decode!("Price: %E2%82%AC79", "Price: €79");
        check_decode!("Currencies:%20$%E2%82%AC%C2%A3", "Currencies: $€£");
    }
}

