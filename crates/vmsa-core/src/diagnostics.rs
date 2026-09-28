//! Redacted support reports. The user previews the text before saving; nothing is uploaded.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub body: String,
}

/// Remove credentials, tokens, user names in paths, e-mail addresses, MAC addresses and public IPs.
pub fn redact(text: &str) -> String {
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        vec![
            // key=value / key: value secrets
            (Regex::new(r"(?i)(password|passwd|pwd|secret|token|api[_-]?key|authorization|bearer)(\s*[:=]\s*)\S+").unwrap(), "$1$2[REDACTED]"),
            // Windows product keys
            (Regex::new(r"\b[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}\b").unwrap(), "[PRODUCT-KEY]"),
            // e-mail addresses
            (Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(), "[EMAIL]"),
            // user home directories (Windows, macOS, Linux)
            (Regex::new(r"(?i)([A-Z]:\\Users\\)[^\\/\r\n]+").unwrap(), "$1[USER]"),
            (Regex::new(r"(/Users/)[^/\r\n]+").unwrap(), "$1[USER]"),
            (Regex::new(r"(/home/)[^/\r\n]+").unwrap(), "$1[USER]"),
            // MAC addresses (colon, dash, or VirtualBox's 12 hex form after "MAC")
            (Regex::new(r"\b([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b").unwrap(), "[MAC]"),
            (Regex::new(r#"(?i)(macaddress\d*="?)[0-9A-F]{12}"#).unwrap(), "$1[MAC]"),
            // GitHub / generic long tokens
            (Regex::new(r"\b(gh[pousr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,})\b").unwrap(), "[TOKEN]"),
        ]
    });
    let mut out = text.to_string();
    for (re, rep) in rules {
        out = re.replace_all(&out, *rep).into_owned();
    }
    // Public IPv4 addresses (keep RFC1918 / NAT 10.0.2.x which are useful for VM diagnosis).
    static IP: OnceLock<Regex> = OnceLock::new();
    let ip = IP.get_or_init(|| Regex::new(r"\b(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\b").unwrap());
    out = ip
        .replace_all(&out, |c: &regex::Captures| {
            let a: u8 = c[1].parse().unwrap_or(0);
            let b: u8 = c[2].parse().unwrap_or(0);
            let private = a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168) || a == 127 || (a == 169 && b == 254) || a == 0 || a == 255;
            if private { c[0].to_string() } else { "[PUBLIC-IP]".to_string() }
        })
        .into_owned();
    out
}

/// Assemble a report with a fixed header. Every section body is redacted.
pub fn build_report(app_version: &str, sections: &[ReportSection]) -> String {
    let mut s = String::new();
    s.push_str(&format!("VM Setup Assistant support report\nApp version: {app_version}\nGenerated: {}\n", chrono::Utc::now().to_rfc3339()));
    s.push_str("Redaction: passwords, tokens, product keys, e-mail addresses, user folder names, MAC addresses and public IP addresses are removed. Review before sharing.\n\n");
    for sec in sections {
        s.push_str(&format!("===== {} =====\n{}\n\n", sec.title, redact(&sec.body)));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secrets_and_identities() {
        let input = "password=hunter2\nToken: abc123\nC:\\Users\\Alice Smith\\VirtualBox VMs\\Win\n/Users/bob/x\n/home/carol/y\nmail alice@example.com\nMAC 08:00:27:ab:cd:ef macaddress1=\"080027ABCDEF\"\nkey XXXXX-YYYYY-ZZZZZ-11111-22222\nip 8.8.8.8 and 10.0.2.15 and 192.168.1.5\nghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let out = redact(input);
        assert!(!out.contains("hunter2"));
        assert!(!out.contains("abc123"));
        assert!(out.contains("C:\\Users\\[USER]\\VirtualBox VMs\\Win"));
        assert!(out.contains("/Users/[USER]/x"));
        assert!(out.contains("/home/[USER]/y"));
        assert!(!out.contains("alice@example.com"));
        assert!(out.contains("[MAC]"));
        assert!(!out.contains("080027ABCDEF"));
        assert!(out.contains("[PRODUCT-KEY]"));
        assert!(out.contains("[PUBLIC-IP]"));
        assert!(out.contains("10.0.2.15"));
        assert!(out.contains("192.168.1.5"));
        assert!(!out.contains("ghp_"));
    }

    #[test]
    fn report_has_header_and_sections() {
        let r = build_report("0.1.0", &[ReportSection { title: "Host".into(), body: "user /Users/dan".into() }]);
        assert!(r.starts_with("VM Setup Assistant support report"));
        assert!(r.contains("===== Host ====="));
        assert!(r.contains("/Users/[USER]"));
    }
}
