// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! CVSS v4.0 base score from a vector string (`CVSS:4.0/AV:N/AC:L/...`).
//!
//! Y6 (2026-11-10): advisories that publish ONLY a v4.0 vector (RustSec's
//! newer records, e.g. RUSTSEC-2026-0459) were ungraded ("unknown"), because
//! [`super::cvss::parse_cvss_score`] read v3.x only. CVSS v4.0 has no closed
//! formula: the score is the MacroVector's score from FIRST's lookup table,
//! less the mean proportional severity distance to the next lower
//! MacroVectors. This is a port of FIRST's reference calculator
//! (`cvss_score.js`, `cvss_lookup.js`, `max_composed.js`,
//! `max_severity.js`), step for step, so it rounds as the reference does.
//!
//! The lookup table, the highest-severity vectors and the severity depths
//! are FIRST's, used under the BSD-2-Clause licence:
//!
//! Copyright (c) 2023 FIRST.ORG, Inc., Red Hat, and contributors
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//! 1. Redistributions of source code must retain the above copyright notice,
//!    this list of conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright
//!    notice, this list of conditions and the following disclaimer in the
//!    documentation and/or other materials provided with the distribution.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
//! LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//! INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//! ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.

use std::collections::HashMap;

/// The metrics a base vector must carry, with their allowed values.
const BASE: [(&str, &[&str]); 11] = [
    ("AV", &["N", "A", "L", "P"]),
    ("AC", &["L", "H"]),
    ("AT", &["N", "P"]),
    ("PR", &["N", "L", "H"]),
    ("UI", &["N", "P", "A"]),
    ("VC", &["H", "L", "N"]),
    ("VI", &["H", "L", "N"]),
    ("VA", &["H", "L", "N"]),
    ("SC", &["H", "L", "N"]),
    ("SI", &["H", "L", "N"]),
    ("SA", &["H", "L", "N"]),
];

/// The metrics the severity distance is measured over.
const DISTANCE_METRICS: [&str; 14] = [
    "AV", "PR", "UI", "AC", "AT", "VC", "VI", "VA", "SC", "SI", "SA", "CR", "IR", "AR",
];

struct Vector<'a>(HashMap<&'a str, &'a str>);

impl<'a> Vector<'a> {
    /// The reference's `m()`: environmental metrics default to the worst
    /// case, and a modified metric overrides its base value.
    fn m(&self, metric: &str) -> &'a str {
        let selected = self.0.get(metric).copied().unwrap_or("X");
        match (metric, selected) {
            ("E", "X") => return "A",
            ("CR" | "IR" | "AR", "X") => return "H",
            _ => {}
        }
        if let Some(modified) = self.0.get(format!("M{metric}").as_str()) {
            if *modified != "X" {
                return modified;
            }
        }
        selected
    }
}

fn level(metric: &str, value: &str) -> Option<f64> {
    let table: &[(&str, f64)] = match metric {
        "AV" => &[("N", 0.0), ("A", 0.1), ("L", 0.2), ("P", 0.3)],
        "PR" => &[("N", 0.0), ("L", 0.1), ("H", 0.2)],
        "UI" => &[("N", 0.0), ("P", 0.1), ("A", 0.2)],
        "AC" => &[("L", 0.0), ("H", 0.1)],
        "AT" => &[("N", 0.0), ("P", 0.1)],
        "VC" | "VI" | "VA" => &[("H", 0.0), ("L", 0.1), ("N", 0.2)],
        "SC" => &[("H", 0.1), ("L", 0.2), ("N", 0.3)],
        "SI" | "SA" => &[("S", 0.0), ("H", 0.1), ("L", 0.2), ("N", 0.3)],
        "CR" | "IR" | "AR" => &[("H", 0.0), ("M", 0.1), ("L", 0.2)],
        _ => return None,
    };
    table.iter().find(|(v, _)| *v == value).map(|(_, l)| *l)
}

fn macro_vector(v: &Vector<'_>) -> [u8; 6] {
    let m = |k: &str| v.m(k);
    let (av, pr, ui) = (m("AV"), m("PR"), m("UI"));
    let eq1 = if av == "N" && pr == "N" && ui == "N" {
        0
    } else if (av == "N" || pr == "N" || ui == "N") && av != "P" {
        1
    } else {
        2
    };
    let eq2 = u8::from(!(m("AC") == "L" && m("AT") == "N"));
    let (vc, vi, va) = (m("VC"), m("VI"), m("VA"));
    let eq3 = if vc == "H" && vi == "H" {
        0
    } else if vc == "H" || vi == "H" || va == "H" {
        1
    } else {
        2
    };
    let eq4 = if m("MSI") == "S" || m("MSA") == "S" {
        0
    } else if m("SC") == "H" || m("SI") == "H" || m("SA") == "H" {
        1
    } else {
        2
    };
    let eq5 = match m("E") {
        "P" => 1,
        "U" => 2,
        _ => 0,
    };
    let eq6 = u8::from(
        !((m("CR") == "H" && vc == "H")
            || (m("IR") == "H" && vi == "H")
            || (m("AR") == "H" && va == "H")),
    );
    [eq1, eq2, eq3, eq4, eq5, eq6]
}

fn lookup(eqs: [u8; 6]) -> Option<f64> {
    let key: String = eqs.iter().map(|d| char::from(b'0' + d)).collect();
    LOOKUP.iter().find(|(m, _)| *m == key).map(|(_, s)| *s)
}

/// `maxComposed`: the highest-severity vectors of each EQ level.
fn eq_maxes(eq: u8, level: u8, eq6: u8) -> &'static [&'static str] {
    match (eq, level, eq6) {
        (1, 0, _) => &["AV:N/PR:N/UI:N/"],
        (1, 1, _) => &["AV:A/PR:N/UI:N/", "AV:N/PR:L/UI:N/", "AV:N/PR:N/UI:P/"],
        (1, 2, _) => &["AV:P/PR:N/UI:N/", "AV:A/PR:L/UI:P/"],
        (2, 0, _) => &["AC:L/AT:N/"],
        (2, 1, _) => &["AC:H/AT:N/", "AC:L/AT:P/"],
        (3, 0, 0) => &["VC:H/VI:H/VA:H/CR:H/IR:H/AR:H/"],
        (3, 0, 1) => &[
            "VC:H/VI:H/VA:L/CR:M/IR:M/AR:H/",
            "VC:H/VI:H/VA:H/CR:M/IR:M/AR:M/",
        ],
        (3, 1, 0) => &[
            "VC:L/VI:H/VA:H/CR:H/IR:H/AR:H/",
            "VC:H/VI:L/VA:H/CR:H/IR:H/AR:H/",
        ],
        (3, 1, 1) => &[
            "VC:L/VI:H/VA:L/CR:H/IR:M/AR:H/",
            "VC:L/VI:H/VA:H/CR:H/IR:M/AR:M/",
            "VC:H/VI:L/VA:H/CR:M/IR:H/AR:M/",
            "VC:H/VI:L/VA:L/CR:M/IR:H/AR:H/",
            "VC:L/VI:L/VA:H/CR:H/IR:H/AR:M/",
        ],
        (3, 2, 1) => &["VC:L/VI:L/VA:L/CR:H/IR:H/AR:H/"],
        (4, 0, _) => &["SC:H/SI:S/SA:S/"],
        (4, 1, _) => &["SC:H/SI:H/SA:H/"],
        (4, 2, _) => &["SC:L/SI:L/SA:L/"],
        (5, 0, _) => &["E:A/"],
        (5, 1, _) => &["E:P/"],
        (5, 2, _) => &["E:U/"],
        _ => &[],
    }
}

/// `maxSeverity`: the depth of each EQ level (+1), before the 0.1 step.
fn max_severity(eqs: [u8; 6]) -> Option<(f64, f64, f64, f64)> {
    let eq1 = [1.0, 4.0, 5.0].get(usize::from(eqs[0]))?;
    let eq2 = [1.0, 2.0].get(usize::from(eqs[1]))?;
    let eq3eq6 = match (eqs[2], eqs[5]) {
        (0, 0) => 7.0,
        (0, 1) => 6.0,
        (1, _) => 8.0,
        (2, 1) => 10.0,
        _ => return None,
    };
    let eq4 = [6.0, 5.0, 4.0].get(usize::from(eqs[3]))?;
    Some((*eq1, *eq2, eq3eq6, *eq4))
}

fn metric_of<'v>(max_vector: &'v str, metric: &str) -> Option<&'v str> {
    max_vector
        .split('/')
        .filter_map(|p| p.split_once(':'))
        .find(|(k, _)| *k == metric)
        .map(|(_, v)| v)
}

/// The score of the next lower MacroVector for EQ3+EQ6 (they move together).
fn lower_eq3eq6(eqs: [u8; 6]) -> Option<f64> {
    let bump = |i: &[usize]| {
        let mut e = eqs;
        for &k in i {
            e[k] += 1;
        }
        lookup(e)
    };
    match (eqs[2], eqs[5]) {
        (1 | 0, 1) => bump(&[2]),
        (1, 0) => bump(&[5]),
        // Two paths down: the higher scoring one (the reference compares
        // the left with the right and takes the right on a tie or a gap).
        (0, 0) => match (bump(&[5]), bump(&[2])) {
            (Some(l), Some(r)) if l > r => Some(l),
            (_, r) => r,
        },
        _ => bump(&[2, 5]),
    }
}

/// The CVSS v4.0 base score (0.0-10.0, one decimal) of a `CVSS:4.0/...`
/// vector. `None` for anything else, a missing or invalid base metric, or a
/// repeated metric.
pub(crate) fn cvss4_base_score(vector: &str) -> Option<f64> {
    let rest = vector.trim().strip_prefix("CVSS:4.0/")?;
    let mut map: HashMap<&str, &str> = HashMap::new();
    for part in rest.split('/').filter(|p| !p.is_empty()) {
        let (k, v) = part.split_once(':')?;
        if map.insert(k, v).is_some() {
            return None;
        }
    }
    for (metric, allowed) in BASE {
        if !map.get(metric).is_some_and(|v| allowed.contains(v)) {
            return None;
        }
    }
    let v = Vector(map);
    if ["VC", "VI", "VA", "SC", "SI", "SA"]
        .iter()
        .all(|m| v.m(m) == "N")
    {
        return Some(0.0);
    }
    let eqs = macro_vector(&v);
    let value = lookup(eqs)?;
    let lower = |i: usize| {
        let mut e = eqs;
        e[i] += 1;
        lookup(e)
    };

    // The first composed highest-severity vector the scored one does not
    // exceed; as in the reference, the last one tried when none fits.
    let mut max_vectors: Vec<String> = Vec::new();
    for a in eq_maxes(1, eqs[0], 0) {
        for b in eq_maxes(2, eqs[1], 0) {
            for c in eq_maxes(3, eqs[2], eqs[5]) {
                for d in eq_maxes(4, eqs[3], 0) {
                    for e in eq_maxes(5, eqs[4], 0) {
                        max_vectors.push(format!("{a}{b}{c}{d}{e}"));
                    }
                }
            }
        }
    }
    let mut dist: HashMap<&str, f64> = HashMap::new();
    for max_vector in &max_vectors {
        dist.clear();
        for metric in DISTANCE_METRICS {
            let ours = level(metric, v.m(metric))?;
            let theirs = level(metric, metric_of(max_vector, metric)?)?;
            dist.insert(metric, ours - theirs);
        }
        if dist.values().all(|d| *d >= 0.0) {
            break;
        }
    }
    let d = |m: &str| dist.get(m).copied().unwrap_or(0.0);
    let step = 0.1;
    let (ms1, ms2, ms36, ms4) = max_severity(eqs)?;
    let parts = [
        (lower(0), d("AV") + d("PR") + d("UI"), ms1 * step),
        (lower(1), d("AC") + d("AT"), ms2 * step),
        (
            lower_eq3eq6(eqs),
            d("VC") + d("VI") + d("VA") + d("CR") + d("IR") + d("AR"),
            ms36 * step,
        ),
        (lower(3), d("SC") + d("SI") + d("SA"), ms4 * step),
    ];
    let mut n_existing = 0u32;
    let mut sum = 0.0;
    for (lower_score, current, depth) in parts {
        if let Some(l) = lower_score {
            n_existing += 1;
            sum += (value - l) * (current / depth);
        }
    }
    // EQ5's proportion is always 0; it still counts toward the mean.
    if lower(4).is_some() {
        n_existing += 1;
    }
    let mean = if n_existing == 0 {
        0.0
    } else {
        sum / f64::from(n_existing)
    };
    let score = (value - mean).clamp(0.0, 10.0);
    Some((score * 10.0).round() / 10.0)
}

/// FIRST's MacroVector scores (`cvss_lookup.js`).
const LOOKUP: [(&str, f64); 270] = [
    ("000000", 10.0),
    ("000001", 9.9),
    ("000010", 9.8),
    ("000011", 9.5),
    ("000020", 9.5),
    ("000021", 9.2),
    ("000100", 10.0),
    ("000101", 9.6),
    ("000110", 9.3),
    ("000111", 8.7),
    ("000120", 9.1),
    ("000121", 8.1),
    ("000200", 9.3),
    ("000201", 9.0),
    ("000210", 8.9),
    ("000211", 8.0),
    ("000220", 8.1),
    ("000221", 6.8),
    ("001000", 9.8),
    ("001001", 9.5),
    ("001010", 9.5),
    ("001011", 9.2),
    ("001020", 9.0),
    ("001021", 8.4),
    ("001100", 9.3),
    ("001101", 9.2),
    ("001110", 8.9),
    ("001111", 8.1),
    ("001120", 8.1),
    ("001121", 6.5),
    ("001200", 8.8),
    ("001201", 8.0),
    ("001210", 7.8),
    ("001211", 7.0),
    ("001220", 6.9),
    ("001221", 4.8),
    ("002001", 9.2),
    ("002011", 8.2),
    ("002021", 7.2),
    ("002101", 7.9),
    ("002111", 6.9),
    ("002121", 5.0),
    ("002201", 6.9),
    ("002211", 5.5),
    ("002221", 2.7),
    ("010000", 9.9),
    ("010001", 9.7),
    ("010010", 9.5),
    ("010011", 9.2),
    ("010020", 9.2),
    ("010021", 8.5),
    ("010100", 9.5),
    ("010101", 9.1),
    ("010110", 9.0),
    ("010111", 8.3),
    ("010120", 8.4),
    ("010121", 7.1),
    ("010200", 9.2),
    ("010201", 8.1),
    ("010210", 8.2),
    ("010211", 7.1),
    ("010220", 7.2),
    ("010221", 5.3),
    ("011000", 9.5),
    ("011001", 9.3),
    ("011010", 9.2),
    ("011011", 8.5),
    ("011020", 8.5),
    ("011021", 7.3),
    ("011100", 9.2),
    ("011101", 8.2),
    ("011110", 8.0),
    ("011111", 7.2),
    ("011120", 7.0),
    ("011121", 5.9),
    ("011200", 8.4),
    ("011201", 7.0),
    ("011210", 7.1),
    ("011211", 5.2),
    ("011220", 5.0),
    ("011221", 3.0),
    ("012001", 8.6),
    ("012011", 7.5),
    ("012021", 5.2),
    ("012101", 7.1),
    ("012111", 5.2),
    ("012121", 2.9),
    ("012201", 6.3),
    ("012211", 2.9),
    ("012221", 1.7),
    ("100000", 9.8),
    ("100001", 9.5),
    ("100010", 9.4),
    ("100011", 8.7),
    ("100020", 9.1),
    ("100021", 8.1),
    ("100100", 9.4),
    ("100101", 8.9),
    ("100110", 8.6),
    ("100111", 7.4),
    ("100120", 7.7),
    ("100121", 6.4),
    ("100200", 8.7),
    ("100201", 7.5),
    ("100210", 7.4),
    ("100211", 6.3),
    ("100220", 6.3),
    ("100221", 4.9),
    ("101000", 9.4),
    ("101001", 8.9),
    ("101010", 8.8),
    ("101011", 7.7),
    ("101020", 7.6),
    ("101021", 6.7),
    ("101100", 8.6),
    ("101101", 7.6),
    ("101110", 7.4),
    ("101111", 5.8),
    ("101120", 5.9),
    ("101121", 5.0),
    ("101200", 7.2),
    ("101201", 5.7),
    ("101210", 5.7),
    ("101211", 5.2),
    ("101220", 5.2),
    ("101221", 2.5),
    ("102001", 8.3),
    ("102011", 7.0),
    ("102021", 5.4),
    ("102101", 6.5),
    ("102111", 5.8),
    ("102121", 2.6),
    ("102201", 5.3),
    ("102211", 2.1),
    ("102221", 1.3),
    ("110000", 9.5),
    ("110001", 9.0),
    ("110010", 8.8),
    ("110011", 7.6),
    ("110020", 7.6),
    ("110021", 7.0),
    ("110100", 9.0),
    ("110101", 7.7),
    ("110110", 7.5),
    ("110111", 6.2),
    ("110120", 6.1),
    ("110121", 5.3),
    ("110200", 7.7),
    ("110201", 6.6),
    ("110210", 6.8),
    ("110211", 5.9),
    ("110220", 5.2),
    ("110221", 3.0),
    ("111000", 8.9),
    ("111001", 7.8),
    ("111010", 7.6),
    ("111011", 6.7),
    ("111020", 6.2),
    ("111021", 5.8),
    ("111100", 7.4),
    ("111101", 5.9),
    ("111110", 5.7),
    ("111111", 5.7),
    ("111120", 4.7),
    ("111121", 2.3),
    ("111200", 6.1),
    ("111201", 5.2),
    ("111210", 5.7),
    ("111211", 2.9),
    ("111220", 2.4),
    ("111221", 1.6),
    ("112001", 7.1),
    ("112011", 5.9),
    ("112021", 3.0),
    ("112101", 5.8),
    ("112111", 2.6),
    ("112121", 1.5),
    ("112201", 2.3),
    ("112211", 1.3),
    ("112221", 0.6),
    ("200000", 9.3),
    ("200001", 8.7),
    ("200010", 8.6),
    ("200011", 7.2),
    ("200020", 7.5),
    ("200021", 5.8),
    ("200100", 8.6),
    ("200101", 7.4),
    ("200110", 7.4),
    ("200111", 6.1),
    ("200120", 5.6),
    ("200121", 3.4),
    ("200200", 7.0),
    ("200201", 5.4),
    ("200210", 5.2),
    ("200211", 4.0),
    ("200220", 4.0),
    ("200221", 2.2),
    ("201000", 8.5),
    ("201001", 7.5),
    ("201010", 7.4),
    ("201011", 5.5),
    ("201020", 6.2),
    ("201021", 5.1),
    ("201100", 7.2),
    ("201101", 5.7),
    ("201110", 5.5),
    ("201111", 4.1),
    ("201120", 4.6),
    ("201121", 1.9),
    ("201200", 5.3),
    ("201201", 3.6),
    ("201210", 3.4),
    ("201211", 1.9),
    ("201220", 1.9),
    ("201221", 0.8),
    ("202001", 6.4),
    ("202011", 5.1),
    ("202021", 2.0),
    ("202101", 4.7),
    ("202111", 2.1),
    ("202121", 1.1),
    ("202201", 2.4),
    ("202211", 0.9),
    ("202221", 0.4),
    ("210000", 8.8),
    ("210001", 7.5),
    ("210010", 7.3),
    ("210011", 5.3),
    ("210020", 6.0),
    ("210021", 5.0),
    ("210100", 7.3),
    ("210101", 5.5),
    ("210110", 5.9),
    ("210111", 4.0),
    ("210120", 4.1),
    ("210121", 2.0),
    ("210200", 5.4),
    ("210201", 4.3),
    ("210210", 4.5),
    ("210211", 2.2),
    ("210220", 2.0),
    ("210221", 1.1),
    ("211000", 7.5),
    ("211001", 5.5),
    ("211010", 5.8),
    ("211011", 4.5),
    ("211020", 4.0),
    ("211021", 2.1),
    ("211100", 6.1),
    ("211101", 5.1),
    ("211110", 4.8),
    ("211111", 1.8),
    ("211120", 2.0),
    ("211121", 0.9),
    ("211200", 4.6),
    ("211201", 1.8),
    ("211210", 1.7),
    ("211211", 0.7),
    ("211220", 0.8),
    ("211221", 0.2),
    ("212001", 5.3),
    ("212011", 2.4),
    ("212021", 1.4),
    ("212101", 2.4),
    ("212111", 1.2),
    ("212121", 0.5),
    ("212201", 1.0),
    ("212211", 0.3),
    ("212221", 0.1),
];

#[cfg(test)]
#[path = "cvss4_tests.rs"]
mod tests;
