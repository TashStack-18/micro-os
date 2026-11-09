//! Kernel command line: space-separated `key=value` pairs (e.g. `test=tc2 quantum=4`).

#[derive(Clone, Copy, Debug)]
pub struct BootArgs<'a> {
    raw: &'a str,
}

impl<'a> BootArgs<'a> {
    pub const fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    pub fn raw(&self) -> &'a str {
        self.raw
    }

    /// Value of `key`. A bare `key` yields `""`. When a key is repeated the
    /// last occurrence wins, so appended arguments override defaults.
    pub fn get(&self, key: &str) -> Option<&'a str> {
        self.raw
            .split_whitespace()
            .rev()
            .find_map(|kv| match kv.split_once('=') {
                Some((k, v)) if k == key => Some(v),
                None if kv == key => Some(""),
                _ => None,
            })
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key)?.parse().ok()
    }

    pub fn flag(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some("" | "1" | "on" | "true" | "yes") => true,
            Some("0" | "off" | "false" | "no") => false,
            _ => default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pairs_flags_and_overrides() {
        let a = BootArgs::new("test=tc2 quantum=4 verbose quantum=8 preempt=0");
        assert_eq!(a.get("test"), Some("tc2"));
        assert_eq!(a.get_u64("quantum"), Some(8));
        assert!(a.flag("verbose", false));
        assert!(!a.flag("preempt", true));
        assert!(a.flag("missing", true));
        assert_eq!(a.get("missing"), None);
        assert_eq!(a.get_u64("test"), None);
    }

    #[test]
    fn empty_command_line() {
        let a = BootArgs::new("");
        assert_eq!(a.get("test"), None);
    }
}
