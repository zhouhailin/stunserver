//! RFC 5780 non-advertising scan (FR-017, SC-007).
//!
//! The server is a single-address STUN server without NAT behavior discovery.
//! Documentation and deployment configuration must not advertise the
//! `stun-behavior` or `stun-behaviors` service names. Normative reference text
//! under `docs/rfcs/` is excluded entirely.

use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn is_negated(line: &str) -> bool {
    let lower = line.to_lowercase();
    [
        "not advertise",
        "never advertise",
        "不宣传",
        "未宣传",
        "不提供",
        "不得宣传",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn is_out_of_scope_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
        && line
            .split('|')
            .any(|cell| cell.trim().eq_ignore_ascii_case("out-of-scope"))
}

/// Return the 1-based line number and text of every affirmative service-name
/// mention. Negated non-claims are ignored; when `skip_out_of_scope_rows` is
/// set, table rows whose status cell is `out-of-scope` are ignored too.
fn advertising_lines(text: &str, skip_out_of_scope_rows: bool) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let lower = line.to_lowercase();
            (lower.contains("stun-behavior") || lower.contains("stun-behaviors"))
                && !is_negated(line)
                && !(skip_out_of_scope_rows && is_out_of_scope_row(line))
        })
        .map(|(index, line)| (index + 1, line.trim().to_string()))
        .collect()
}

#[test]
fn test_detector_flags_affirmative_and_ignores_negated() {
    assert_eq!(
        advertising_lines("Publish _stun._udp.stun-behavior SRV records", false).len(),
        1
    );
    assert!(advertising_lines("The server does not advertise stun-behavior", false).is_empty());
    assert!(advertising_lines("服务器不宣传 stun-behavior 服务名", false).is_empty());
    assert!(advertising_lines(
        "| 5780 | §7 | MAY | Behavior discovery | out-of-scope | justification stun-behavior |",
        true
    )
    .is_empty());
}

#[test]
fn test_no_rfc5780_service_advertising() {
    let root = repository_root();
    let mut files = vec![
        root.join("README.md"),
        root.join("README.zh-CN.md"),
        root.join("Makefile"),
    ];
    for directory in ["config", "deploy", "docker", "systemd"] {
        collect_files(&root.join(directory), &mut files);
    }

    let mut offending = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (line_number, line) in advertising_lines(&text, false) {
            offending.push(format!("{}:{}: {}", file.display(), line_number, line));
        }
    }

    // The conformance matrix may name the service in out-of-scope rows.
    let matrix = root.join("docs/rfc-conformance.md");
    if let Ok(text) = fs::read_to_string(&matrix) {
        for (line_number, line) in advertising_lines(&text, true) {
            offending.push(format!("{}:{}: {}", matrix.display(), line_number, line));
        }
    }

    assert!(
        offending.is_empty(),
        "RFC 5780 service advertising found:\n{}",
        offending.join("\n")
    );
}
