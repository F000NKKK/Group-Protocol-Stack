//! Interoperability conformance classes (gbp-interop-profile §2).
//!
//! Class A = GBP + GSP (mandatory base)
//! Class B = Class A + GTP
//! Class C = Class B + GAP (full stack)
//!
//! Implementations MUST declare their conformance class during capability
//! negotiation using the well-known token strings defined here.

use core::fmt;

/// Interoperability conformance class (gbp-interop-profile §2).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConformanceClass {
    /// Class A: GBP + GSP (mandatory minimum).
    A,
    /// Class B: Class A + GTP (text messaging).
    B,
    /// Class C: Class B + GAP (audio).
    C,
}

impl ConformanceClass {
    /// Well-known capability token for Class A.
    pub const TOKEN_A: &'static str = "gbp/class-a";
    /// Well-known capability token for Class B.
    pub const TOKEN_B: &'static str = "gbp/class-b";
    /// Well-known capability token for Class C.
    pub const TOKEN_C: &'static str = "gbp/class-c";

    /// Returns all capability tokens that must be advertised for this class
    /// (including tokens for lower classes — each class implies the ones below).
    pub fn tokens(self) -> &'static [&'static str] {
        match self {
            Self::A => &[Self::TOKEN_A],
            Self::B => &[Self::TOKEN_A, Self::TOKEN_B],
            Self::C => &[Self::TOKEN_A, Self::TOKEN_B, Self::TOKEN_C],
        }
    }

    /// Infers the highest conformance class from a set of capability tokens.
    /// Returns `None` if not even Class A is present.
    pub fn from_tokens<'a>(tokens: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let mut has_a = false;
        let mut has_b = false;
        let mut has_c = false;
        for t in tokens {
            match t {
                Self::TOKEN_A => has_a = true,
                Self::TOKEN_B => has_b = true,
                Self::TOKEN_C => has_c = true,
                _ => {}
            }
        }
        if has_a && has_b && has_c {
            Some(Self::C)
        } else if has_a && has_b {
            Some(Self::B)
        } else if has_a {
            Some(Self::A)
        } else {
            None
        }
    }
}

impl fmt::Display for ConformanceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::A => "Class A (GBP+GSP)",
            Self::B => "Class B (GBP+GSP+GTP)",
            Self::C => "Class C (GBP+GSP+GTP+GAP)",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_cumulative() {
        assert_eq!(ConformanceClass::A.tokens(), &["gbp/class-a"]);
        assert_eq!(
            ConformanceClass::B.tokens(),
            &["gbp/class-a", "gbp/class-b"]
        );
        assert_eq!(
            ConformanceClass::C.tokens(),
            &["gbp/class-a", "gbp/class-b", "gbp/class-c"]
        );
    }

    #[test]
    fn from_tokens_detects_class() {
        assert_eq!(
            ConformanceClass::from_tokens(["gbp/class-a"]),
            Some(ConformanceClass::A)
        );
        assert_eq!(
            ConformanceClass::from_tokens(["gbp/class-a", "gbp/class-b"]),
            Some(ConformanceClass::B)
        );
        assert_eq!(
            ConformanceClass::from_tokens(["gbp/class-a", "gbp/class-b", "gbp/class-c"]),
            Some(ConformanceClass::C)
        );
        assert_eq!(ConformanceClass::from_tokens(["something-else"]), None);
    }

    #[test]
    fn class_ordering() {
        assert!(ConformanceClass::A < ConformanceClass::B);
        assert!(ConformanceClass::B < ConformanceClass::C);
    }
}
