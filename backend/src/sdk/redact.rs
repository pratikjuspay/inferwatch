//! PII redaction for log previews.
//!
//! Everything that goes into `inference_logs` previews passes through here
//! BEFORE the 500-char cap (redact first, cap after — a hard cap could slice
//! a sensitive token in half and leave the front of it readable).

use regex::Regex;
use std::sync::LazyLock;

/// Ordered: cards before phones (a spaced card number also matches the loose
/// phone shape — first match wins).
static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // credit/debit card: 13–19 digits, optionally separated by spaces/dashes
        (
            Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap(),
            "[REDACTED:card]",
        ),
        // email
        (
            Regex::new(r"\b[\w.+%-]+@[\w-]+(?:\.[\w-]+)+\b").unwrap(),
            "[REDACTED:email]",
        ),
        // known key formats + Bearer tokens
        (
            Regex::new(r"\b(?:sk|pk|api|key|token|xox[baprs])-[A-Za-z0-9_\-]{8,}\b").unwrap(),
            "[REDACTED:secret]",
        ),
        (
            Regex::new(r"\bBearer\s+[A-Za-z0-9._\-]{10,}\b").unwrap(),
            "[REDACTED:secret]",
        ),
        (
            Regex::new(r"\bAIza[0-9A-Za-z_\-]{10,}\b").unwrap(),
            "[REDACTED:secret]",
        ),
        // phone: +country international form, or a bare 10-digit run.
        // (cards are 13–19 digits and already replaced above — no clash)
        (
            Regex::new(r"\+\d{1,3}[\d .\-()]{8,16}\d|\b\d{10}\b").unwrap(),
            "[REDACTED:phone]",
        ),
    ]
});

/// Replace every PII-shaped substring with a typed placeholder.
pub fn redact(s: &str) -> String {
    let mut out = s.to_string();
    for (re, tag) in PATTERNS.iter() {
        out = re.replace_all(&out, *tag).into_owned();
    }
    out
}

/// The only way previews are produced: redact, then cap.
pub fn preview(s: &str, max: usize) -> String {
    redact(s).chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email() {
        assert_eq!(
            redact("reach me at pratik.giramkar@juspay.in for details"),
            "reach me at [REDACTED:email] for details"
        );
    }

    #[test]
    fn redacts_multiple_emails() {
        assert_eq!(
            redact("cc a@b.co and x.y@z.io"),
            "cc [REDACTED:email] and [REDACTED:email]"
        );
    }

    #[test]
    fn redacts_card_spaced_and_dashed() {
        assert_eq!(redact("card 4242 4242 4242 4242"), "card [REDACTED:card]");
        assert_eq!(redact("card 4242-4242-4242-4242"), "card [REDACTED:card]");
        assert_eq!(redact("4111111111111111"), "[REDACTED:card]");
    }

    #[test]
    fn redacts_secret_shapes() {
        assert_eq!(
            redact("use key sk-abc123XYZ_longerkey"),
            "use key [REDACTED:secret]"
        );
        assert_eq!(
            redact("header: Bearer eyJhbGciOiJIUzI1NiJ9.payload"),
            "header: [REDACTED:secret]"
        );
    }

    #[test]
    fn redacts_phone_floor_and_plus() {
        assert_eq!(redact("call +91 98765 43210"), "call [REDACTED:phone]");
    }

    #[test]
    fn leaves_clean_text_untouched() {
        let s = "What is the throughput of tokio mpsc channels under load? Order #45821 shipped in 3 days.";
        assert_eq!(redact(s), s);
    }

    #[test]
    fn preview_redacts_then_caps() {
        // '3' * 495 chars, then a card across the cap boundary:
        // redaction runs BEFORE the cap, so no partial digits leak.
        let mut s = "3".repeat(495);
        s.push_str(" 4111 1111 1111 1111 end");
        let p = preview(&s, 500);
        assert!(!p.contains("4111"), "sensitive digits must never survive");
        assert!(p.chars().count() <= 500, "cap holds after redaction");
    }

    #[test]
    fn preview_is_char_safe_not_byte_slicing() {
        let s = "नमस्ते ".repeat(200); // multi-byte chars
        let p = preview(&s, 500);
        assert!(p.chars().count() <= 500);
    }
}
