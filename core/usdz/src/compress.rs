// SPDX-License-Identifier: MIT
//! The compression of a binary layer's structural sections and integer
//! arrays: LZ4 blocks in USD's chunked framing ("fast compression") and
//! USD's integer coding, both directions (the reader checks the writer).

/// The LZ4 framing USD uses: a chunk count byte (0: one chunk), then the
/// LZ4 block. One chunk holds up to about 2 GB, more than any layer here.
pub fn fast_compress(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() / 2 + 16);
    out.push(0);
    lz4_compress(input, &mut out);
    out
}

/// The bytes of [`fast_compress`]'s framing; at most `max_output` of them.
pub fn fast_decompress(input: &[u8], max_output: usize) -> Result<Vec<u8>, String> {
    let (&chunks, mut rest) = input
        .split_first()
        .ok_or("compressed data without its chunk count")?;
    if chunks == 0 {
        return lz4_decompress(rest, max_output);
    }
    let mut out = Vec::new();
    for _ in 0..chunks {
        let size = rest
            .get(..4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or("a compressed chunk without its size")?;
        let size = usize::try_from(size).map_err(|_| "a negative chunk size")?;
        let chunk = rest
            .get(4..4 + size)
            .ok_or("a compressed chunk cut short")?;
        out.extend(lz4_decompress(chunk, max_output - out.len())?);
        rest = &rest[4 + size..];
    }
    Ok(out)
}

/// Hash table size (log 2) of the compressor.
const HASH_BITS: u32 = 14;
/// The shortest match LZ4 encodes.
const MIN_MATCH: usize = 4;
/// LZ4's end rules: the last 5 bytes are literals, and the last match
/// starts at least 12 bytes before the end.
const LAST_LITERALS: usize = 5;
const MATCH_FIND_LIMIT: usize = 12;

fn read_u32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

/// A greedy LZ4 block compressor (one hash probe per position).
pub fn lz4_compress(input: &[u8], out: &mut Vec<u8>) {
    let n = input.len();
    let mut anchor = 0;
    if n > MATCH_FIND_LIMIT {
        let mut table = vec![0u32; 1 << HASH_BITS];
        let hash = |v: u32| (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize;
        let mut i = 0;
        while i + MATCH_FIND_LIMIT < n {
            let sequence = read_u32(input, i);
            let slot = hash(sequence);
            // Positions + 1, so that 0 is an empty slot.
            let candidate = table[slot] as usize;
            table[slot] = (i + 1) as u32;
            if candidate > 0 {
                let candidate = candidate - 1;
                if i - candidate <= 0xffff && read_u32(input, candidate) == sequence {
                    let mut length = MIN_MATCH;
                    while i + length < n - LAST_LITERALS
                        && input[candidate + length] == input[i + length]
                    {
                        length += 1;
                    }
                    sequence_out(out, &input[anchor..i], Some((i - candidate, length)));
                    i += length;
                    anchor = i;
                    continue;
                }
            }
            i += 1;
        }
    }
    sequence_out(out, &input[anchor..], None);
}

/// One LZ4 sequence: literals, then a match (offset, length) unless it is
/// the last one.
fn sequence_out(out: &mut Vec<u8>, literals: &[u8], matched: Option<(usize, usize)>) {
    let extra = |out: &mut Vec<u8>, mut rest: usize| {
        while rest >= 255 {
            out.push(255);
            rest -= 255;
        }
        out.push(rest as u8);
    };
    let literal_code = literals.len().min(15);
    let match_code = matched.map_or(0, |(_, length)| (length - MIN_MATCH).min(15));
    out.push(((literal_code << 4) | match_code) as u8);
    if literal_code == 15 {
        extra(out, literals.len() - 15);
    }
    out.extend_from_slice(literals);
    if let Some((offset, length)) = matched {
        out.extend_from_slice(&(offset as u16).to_le_bytes());
        if match_code == 15 {
            extra(out, length - MIN_MATCH - 15);
        }
    }
}

/// Decompresses one LZ4 block of at most `max_output` bytes.
pub fn lz4_decompress(input: &[u8], max_output: usize) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    let byte = |i: &mut usize| -> Result<u8, String> {
        let b = *input.get(*i).ok_or("LZ4 block cut short")?;
        *i += 1;
        Ok(b)
    };
    let length = |i: &mut usize, code: usize| -> Result<usize, String> {
        let mut length = code;
        if code == 15 {
            loop {
                let b = byte(i)?;
                length += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(length)
    };
    while i < input.len() {
        let token = byte(&mut i)? as usize;
        let literals = length(&mut i, token >> 4)?;
        let end = i + literals;
        out.extend_from_slice(input.get(i..end).ok_or("LZ4 literals cut short")?);
        i = end;
        if i == input.len() {
            break;
        }
        let offset = u16::from_le_bytes([byte(&mut i)?, byte(&mut i)?]) as usize;
        if offset == 0 || offset > out.len() {
            return Err(format!("LZ4 match offset {offset} out of range"));
        }
        let count = length(&mut i, token & 15)? + MIN_MATCH;
        let start = out.len() - offset;
        for k in 0..count {
            let b = out[start + k];
            out.push(b);
        }
        if out.len() > max_output {
            return Err("LZ4 block larger than expected".to_owned());
        }
    }
    if out.len() > max_output {
        return Err("LZ4 block larger than expected".to_owned());
    }
    Ok(out)
}

/// USD's integer coding of 32-bit integers, compressed with
/// [`fast_compress`]: the most common difference between neighbours, then
/// a 2-bit code per integer (the common difference, or one of 1, 2 or 4
/// bytes) and the differences that are not the common one.
pub fn compress_ints(values: &[i32]) -> Vec<u8> {
    fast_compress(&encode_ints(values))
}

fn encode_ints(values: &[i32]) -> Vec<u8> {
    if values.is_empty() {
        return Vec::new();
    }
    // The most common difference; the largest of equally common ones.
    let mut counts = std::collections::HashMap::new();
    let (mut common, mut common_count) = (0i32, 0usize);
    let mut previous = 0i32;
    for &value in values {
        let difference = value.wrapping_sub(previous);
        let count = counts.entry(difference).or_insert(0usize);
        *count += 1;
        if *count > common_count || (*count == common_count && difference > common) {
            common = difference;
            common_count = *count;
        }
        previous = value;
    }
    let codes_len = (values.len() * 2).div_ceil(8);
    let mut out = Vec::with_capacity(4 + codes_len + values.len());
    out.extend_from_slice(&common.to_le_bytes());
    out.resize(4 + codes_len, 0);
    let mut previous = 0i32;
    for (i, &value) in values.iter().enumerate() {
        let difference = value.wrapping_sub(previous);
        previous = value;
        let code: u8 = if difference == common {
            0
        } else if let Ok(small) = i8::try_from(difference) {
            out.push(small as u8);
            1
        } else if let Ok(medium) = i16::try_from(difference) {
            out.extend_from_slice(&medium.to_le_bytes());
            2
        } else {
            out.extend_from_slice(&difference.to_le_bytes());
            3
        };
        out[4 + i / 4] |= code << (2 * (i % 4));
    }
    out
}

/// `count` integers of [`compress_ints`]' coding.
pub fn decompress_ints(input: &[u8], count: usize) -> Result<Vec<i32>, String> {
    let codes_len = (count * 2).div_ceil(8);
    let encoded = fast_decompress(input, 4 + codes_len + count * 4)?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let common = encoded
        .get(..4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or("integer coding without its common value")?;
    let codes = encoded
        .get(4..4 + codes_len)
        .ok_or("integer coding cut short")?;
    let mut rest = &encoded[4 + codes_len..];
    let mut take = |n: usize| -> Result<&[u8], String> {
        let (head, tail) = rest
            .split_at_checked(n)
            .ok_or("integer coding values cut short")?;
        rest = tail;
        Ok(head)
    };
    let mut out = Vec::with_capacity(count);
    let mut previous = 0i32;
    for i in 0..count {
        let difference = match (codes[i / 4] >> (2 * (i % 4))) & 3 {
            0 => common,
            1 => i32::from(take(1)?[0] as i8),
            2 => {
                let b = take(2)?;
                i32::from(i16::from_le_bytes([b[0], b[1]]))
            }
            _ => {
                let b = take(4)?;
                i32::from_le_bytes([b[0], b[1], b[2], b[3]])
            }
        };
        previous = previous.wrapping_add(difference);
        out.push(previous);
    }
    Ok(out)
}
