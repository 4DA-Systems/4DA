//! CVSS v3.0/v3.1 base-score computation from a vector string.
//!
//! OSV / GitHub Security Advisories put the CVSS **vector** (e.g.
//! `CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H`) in the severity `score` field per the OSV schema,
//! NOT the numeric base score. 4DA previously did `score.parse::<f64>()` everywhere, which yields `None`
//! for vector strings — so severity was silently dropped for the common (vector) case. This module
//! computes the numeric base score from the vector per the CVSS v3.1 spec (§7.1). Bare-number scores
//! (some sources emit "9.8") are still handled by [`parse_cvss_score`].

/// Parse a CVSS severity `score` string into a numeric base score (0.0–10.0).
/// Tries a bare number first, else computes from a CVSS v3.x vector. `None` if neither yields a value.
pub(crate) fn parse_cvss_score(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Ok(n) = s.parse::<f64>() {
        if (0.0..=10.0).contains(&n) {
            return Some(n);
        }
    }
    cvss_base_score(s)
}

/// Compute the CVSS v3.0/v3.1 base score from a vector string. `None` on a malformed vector or a
/// missing required base metric. Temporal/environmental metrics and the `CVSS:3.x` prefix are ignored.
pub(crate) fn cvss_base_score(vector: &str) -> Option<f64> {
    let (mut av, mut ac, mut pr, mut ui) = (None, None, None, None);
    let (mut sc, mut conf, mut integ, mut avail) = (None, None, None, None);
    for part in vector.split('/') {
        let (key, val) = part.split_once(':')?; // every segment is Key:Val ("CVSS:3.1" too, key="CVSS")
        match key {
            "AV" => av = Some(val),
            "AC" => ac = Some(val),
            "PR" => pr = Some(val),
            "UI" => ui = Some(val),
            "S" => sc = Some(val),
            "C" => conf = Some(val),
            "I" => integ = Some(val),
            "A" => avail = Some(val),
            _ => {} // CVSS prefix, temporal (E/RL/RC), environmental — not part of the base score
        }
    }
    let (av, ac, pr, ui, sc, conf, integ, avail) = (av?, ac?, pr?, ui?, sc?, conf?, integ?, avail?);
    let scope_changed = match sc {
        "C" => true,
        "U" => false,
        _ => return None,
    };
    let av_v = match av {
        "N" => 0.85,
        "A" => 0.62,
        "L" => 0.55,
        "P" => 0.20,
        _ => return None,
    };
    let ac_v = match ac {
        "L" => 0.77,
        "H" => 0.44,
        _ => return None,
    };
    let pr_v = match (pr, scope_changed) {
        ("N", _) => 0.85,
        ("L", false) => 0.62,
        ("L", true) => 0.68,
        ("H", false) => 0.27,
        ("H", true) => 0.50,
        _ => return None,
    };
    let ui_v = match ui {
        "N" => 0.85,
        "R" => 0.62,
        _ => return None,
    };
    let imp = |x: &str| -> Option<f64> {
        match x {
            "H" => Some(0.56),
            "L" => Some(0.22),
            "N" => Some(0.0),
            _ => None,
        }
    };
    let (cv, iv, av2) = (imp(conf)?, imp(integ)?, imp(avail)?);

    let iss = 1.0 - ((1.0 - cv) * (1.0 - iv) * (1.0 - av2));
    let impact = if scope_changed {
        7.52 * (iss - 0.029) - 3.25 * (iss - 0.02).powf(15.0)
    } else {
        6.42 * iss
    };
    if impact <= 0.0 {
        return Some(0.0);
    }
    let exploitability = 8.22 * av_v * ac_v * pr_v * ui_v;
    let raw = if scope_changed {
        (1.08 * (impact + exploitability)).min(10.0)
    } else {
        (impact + exploitability).min(10.0)
    };
    Some(roundup(raw))
}

/// CVSS v3.1 spec roundup (§Appendix A): round UP to one decimal place, float-precision-safe.
fn roundup(x: f64) -> f64 {
    let int_input = (x * 100_000.0).round() as i64;
    if int_input % 10_000 == 0 {
        int_input as f64 / 100_000.0
    } else {
        ((int_input / 10_000) + 1) as f64 / 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Canonical vectors from the CVSS v3.1 spec / well-known CVEs (vector -> exact base score).
    #[test]
    fn canonical_vectors() {
        let cases = [
            ("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H", 9.8), // network RCE
            ("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H", 7.5), // network DoS
            ("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:C/C:H/I:H/A:H", 10.0), // scope-changed max
            ("CVSS:3.1/AV:L/AC:L/PR:L/UI:N/S:U/C:H/I:H/A:H", 7.8), // local privesc
            ("CVSS:3.1/AV:P/AC:H/PR:H/UI:R/S:U/C:L/I:N/A:N", 1.6), // physical, low (spec-computed: iss .22, impact 1.4124, exploit .1211 -> 1.6)
            ("CVSS:3.0/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H", 9.8), // v3.0 vectors compute identically
        ];
        for (v, want) in cases {
            let got = cvss_base_score(v).unwrap_or_else(|| panic!("no score for {v}"));
            assert!(
                (got - want).abs() < 1e-9,
                "vector {v}: got {got}, want {want}"
            );
        }
    }

    #[test]
    fn parse_prefers_bare_number_then_vector() {
        assert_eq!(parse_cvss_score("9.8"), Some(9.8));
        assert_eq!(
            parse_cvss_score("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"),
            Some(9.8)
        );
        assert_eq!(parse_cvss_score("HIGH"), None);
        assert_eq!(parse_cvss_score(""), None);
        // malformed vector -> None, never a wrong number
        assert_eq!(parse_cvss_score("CVSS:3.1/AV:N/AC:L"), None);
    }
}
