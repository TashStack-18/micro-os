//! Minimal flattened-device-tree (DTB) reader.
//!
//! The firmware passes the DTB address in `a1`. We only need one property,
//! `/chosen/bootargs` (QEMU fills it from `-append`), so this walks the
//! structure block token by token instead of building a tree.

const MAGIC: u32 = 0xd00d_feed;
const HEADER_LEN: usize = 40;
const TOK_BEGIN_NODE: u32 = 1;
const TOK_END_NODE: u32 = 2;
const TOK_PROP: u32 = 3;
const TOK_NOP: u32 = 4;
const TOK_END: u32 = 9;

fn be32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off.checked_add(4)?)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn align4(x: usize) -> usize {
    (x + 3) & !3
}

fn cstr(b: &[u8], off: usize) -> Option<&str> {
    let rest = b.get(off..)?;
    let len = rest.iter().position(|&c| c == 0)?;
    core::str::from_utf8(&rest[..len]).ok()
}

/// Bytes needed to read a DTB header.
pub const fn header_len() -> usize {
    HEADER_LEN
}

/// Total blob size declared in a DTB header, or `None` if the magic is wrong.
pub fn total_size(header: &[u8]) -> Option<usize> {
    if be32(header, 0)? != MAGIC {
        return None;
    }
    Some(be32(header, 4)? as usize)
}

/// Return the `bootargs` property of the `/chosen` node, if present.
/// Every offset is bounds-checked, so a malformed blob yields `None`.
pub fn find_bootargs(blob: &[u8]) -> Option<&str> {
    if be32(blob, 0)? != MAGIC {
        return None;
    }
    let mut off = be32(blob, 8)? as usize;
    let strings = be32(blob, 12)? as usize;
    let mut depth = 0usize;
    let mut chosen_depth: Option<usize> = None;
    loop {
        let tok = be32(blob, off)?;
        off += 4;
        match tok {
            TOK_BEGIN_NODE => {
                let name = cstr(blob, off)?;
                off = align4(off + name.len() + 1);
                depth += 1;
                // depth 1 is the root node; /chosen is one of its children.
                if depth == 2 && (name == "chosen" || name.starts_with("chosen@")) {
                    chosen_depth = Some(depth);
                }
            }
            TOK_END_NODE => {
                if chosen_depth == Some(depth) {
                    chosen_depth = None;
                }
                depth = depth.checked_sub(1)?;
            }
            TOK_PROP => {
                let len = be32(blob, off)? as usize;
                let name_off = be32(blob, off + 4)? as usize;
                off += 8;
                let value = blob.get(off..off.checked_add(len)?)?;
                off = align4(off + len);
                if chosen_depth == Some(depth) && cstr(blob, strings + name_off)? == "bootargs" {
                    let text = value.split(|&c| c == 0).next()?;
                    return core::str::from_utf8(text).ok();
                }
            }
            TOK_NOP => {}
            TOK_END => return None,
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(v: &mut Vec<u8>, t: u32) {
        v.extend_from_slice(&t.to_be_bytes());
    }
    fn pad(v: &mut Vec<u8>) {
        while v.len() % 4 != 0 {
            v.push(0);
        }
    }
    fn node(v: &mut Vec<u8>, name: &str) {
        tok(v, TOK_BEGIN_NODE);
        v.extend_from_slice(name.as_bytes());
        v.push(0);
        pad(v);
    }
    fn prop(v: &mut Vec<u8>, name_off: u32, val: &[u8]) {
        tok(v, TOK_PROP);
        tok(v, val.len() as u32);
        tok(v, name_off);
        v.extend_from_slice(val);
        pad(v);
    }

    /// Build a tiny DTB: a decoy `bootargs` in /cpus, the real one in /chosen.
    fn build(bootargs: Option<&str>) -> Vec<u8> {
        let strings = b"model\0bootargs\0"; // "model" at 0, "bootargs" at 6
        let mut st = Vec::new();
        node(&mut st, "");
        prop(&mut st, 0, b"qemu\0");
        node(&mut st, "cpus");
        prop(&mut st, 6, b"decoy\0");
        tok(&mut st, TOK_END_NODE);
        node(&mut st, "chosen");
        tok(&mut st, TOK_NOP);
        if let Some(a) = bootargs {
            let mut val = a.as_bytes().to_vec();
            val.push(0);
            prop(&mut st, 6, &val);
        }
        tok(&mut st, TOK_END_NODE);
        tok(&mut st, TOK_END_NODE);
        tok(&mut st, TOK_END);

        let off_struct = HEADER_LEN as u32;
        let off_strings = off_struct + st.len() as u32;
        let total = off_strings + strings.len() as u32;
        let mut blob = Vec::new();
        for f in [MAGIC, total, off_struct, off_strings, 0, 17, 16, 0, strings.len() as u32, st.len() as u32] {
            tok(&mut blob, f);
        }
        blob.extend_from_slice(&st);
        blob.extend_from_slice(strings);
        blob
    }

    #[test]
    fn finds_chosen_bootargs_and_ignores_other_nodes() {
        let blob = build(Some("test=tc1 quantum=2"));
        assert_eq!(total_size(&blob), Some(blob.len()));
        assert_eq!(find_bootargs(&blob), Some("test=tc1 quantum=2"));
    }

    #[test]
    fn missing_property_and_bad_blobs() {
        assert_eq!(find_bootargs(&build(None)), None);
        let mut bad = build(Some("x=1"));
        bad[0] = 0;
        assert_eq!(total_size(&bad), None);
        assert_eq!(find_bootargs(&bad), None);
        let blob = build(Some("x=1"));
        assert_eq!(find_bootargs(&blob[..60]), None); // truncated: no panic
    }
}
