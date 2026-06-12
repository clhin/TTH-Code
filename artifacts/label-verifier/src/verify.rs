use regex::Regex;
use strsim::normalized_levenshtein;

use crate::models::{ApplicationData, FieldResult, VerificationResult, VerificationStatus};

/// The exact TTB-mandated government warning text
const GOVERNMENT_WARNING: &str = "GOVERNMENT WARNING: (1) According to the Surgeon General, \
women should not drink alcoholic beverages during pregnancy because of the risk of birth defects. \
(2) Consumption of alcoholic beverages impairs your ability to drive a car or operate machinery, \
and may cause health problems.";

/// Entry point: run all enabled field checks
pub fn verify_label(
    ocr_text: &str,
    app_data: &ApplicationData,
    filename: Option<String>,
) -> VerificationResult {
    let mut fields: Vec<FieldResult> = Vec::new();

    if let Some(ref v) = app_data.brand_name {
        if !v.trim().is_empty() {
            fields.push(check_fuzzy("Brand Name", v, ocr_text));
        }
    }

    if let Some(ref v) = app_data.class_type {
        if !v.trim().is_empty() {
            fields.push(check_fuzzy("Class / Type", v, ocr_text));
        }
    }

    if let Some(ref v) = app_data.abv {
        if !v.trim().is_empty() {
            fields.push(check_abv(v, ocr_text));
        }
    }

    if let Some(ref v) = app_data.net_contents {
        if !v.trim().is_empty() {
            fields.push(check_net_contents(v, ocr_text));
        }
    }

    if let Some(ref v) = app_data.producer_name {
        if !v.trim().is_empty() {
            fields.push(check_fuzzy("Producer / Bottler", v, ocr_text));
        }
    }

    if let Some(ref v) = app_data.country_of_origin {
        if !v.trim().is_empty() {
            fields.push(check_fuzzy("Country of Origin", v, ocr_text));
        }
    }

    if app_data.check_government_warning {
        fields.push(check_government_warning(ocr_text));
    }

    let fails = fields.iter().filter(|f| f.status == VerificationStatus::Fail).count();
    let warns = fields.iter().filter(|f| f.status == VerificationStatus::Warning).count();
    let passes = fields.iter().filter(|f| f.status == VerificationStatus::Pass).count();

    let overall_status = if fails > 0 {
        VerificationStatus::Fail
    } else if warns > 0 {
        VerificationStatus::Warning
    } else {
        VerificationStatus::Pass
    };

    let summary = format!("{} passed · {} warnings · {} failed", passes, warns, fails);

    VerificationResult {
        fields,
        overall_status,
        ocr_text: ocr_text.to_string(),
        summary,
        filename,
    }
}

// ---------------------------------------------------------------------------
// Field checkers
// ---------------------------------------------------------------------------

fn check_fuzzy(field_name: &str, expected: &str, ocr_text: &str) -> FieldResult {
    let exp_up = expected.trim().to_uppercase();
    let text_up = ocr_text.to_uppercase();

    // 1. Exact case-insensitive substring match
    if text_up.contains(&exp_up) {
        return FieldResult {
            field: field_name.to_string(),
            status: VerificationStatus::Pass,
            expected: expected.to_string(),
            found: Some(expected.to_string()),
            message: "Exact match found on label.".to_string(),
        };
    }

    // 2. Fuzzy per-line match (handles minor OCR noise and trivial case diffs)
    let (best_text, best_score) = best_fuzzy_line(&exp_up, ocr_text);

    if best_score >= 0.88 {
        FieldResult {
            field: field_name.to_string(),
            status: VerificationStatus::Pass,
            expected: expected.to_string(),
            found: Some(best_text),
            message: format!("Close match found ({:.0}% similarity).", best_score * 100.0),
        }
    } else if best_score >= 0.65 {
        FieldResult {
            field: field_name.to_string(),
            status: VerificationStatus::Warning,
            expected: expected.to_string(),
            found: Some(best_text),
            message: format!(
                "Possible match but low confidence ({:.0}%) — manual review recommended.",
                best_score * 100.0
            ),
        }
    } else {
        FieldResult {
            field: field_name.to_string(),
            status: VerificationStatus::Fail,
            expected: expected.to_string(),
            found: if best_text.is_empty() { None } else { Some(best_text) },
            message: "Not found on label.".to_string(),
        }
    }
}

fn best_fuzzy_line(expected_upper: &str, ocr_text: &str) -> (String, f64) {
    let exp_chars: Vec<char> = expected_upper.chars().collect();
    let exp_len = exp_chars.len();

    let mut best_score = 0.0f64;
    let mut best_text = String::new();

    for line in ocr_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let line_up = trimmed.to_uppercase();

        // Full line match
        let score = normalized_levenshtein(expected_upper, &line_up);
        if score > best_score {
            best_score = score;
            best_text = trimmed.to_string();
        }

        // Sliding window (same length as expected)
        let line_chars: Vec<char> = line_up.chars().collect();
        if line_chars.len() > exp_len {
            for i in 0..=(line_chars.len() - exp_len) {
                let window: String = line_chars[i..i + exp_len].iter().collect();
                let s = normalized_levenshtein(expected_upper, &window);
                if s > best_score {
                    best_score = s;
                    best_text = trimmed.to_string();
                }
            }
        }
    }

    (best_text, best_score)
}

fn check_abv(expected: &str, ocr_text: &str) -> FieldResult {
    let abv_re = Regex::new(r"(\d{1,3}(?:\.\d{1,2})?)\s*%").unwrap();

    let expected_val: Option<f64> = abv_re
        .captures(expected)
        .and_then(|c| c[1].parse().ok());

    let Some(exp_abv) = expected_val else {
        return check_fuzzy("Alcohol Content (ABV)", expected, ocr_text);
    };

    let found_vals: Vec<f64> = abv_re
        .captures_iter(ocr_text)
        .filter_map(|c| c[1].parse::<f64>().ok())
        .filter(|&v| v > 0.0 && v <= 100.0)
        .collect();

    if found_vals.is_empty() {
        return FieldResult {
            field: "Alcohol Content (ABV)".to_string(),
            status: VerificationStatus::Fail,
            expected: expected.to_string(),
            found: None,
            message: "No ABV percentage found on label.".to_string(),
        };
    }

    let closest = found_vals
        .iter()
        .copied()
        .min_by(|a, b| {
            (a - exp_abv)
                .abs()
                .partial_cmp(&(*b - exp_abv).abs())
                .unwrap()
        })
        .unwrap();

    let diff = (closest - exp_abv).abs();

    let status = if diff < 0.15 {
        VerificationStatus::Pass
    } else if diff <= 0.5 {
        VerificationStatus::Warning
    } else {
        VerificationStatus::Fail
    };

    let message = match status {
        VerificationStatus::Pass => "ABV matches application.".to_string(),
        VerificationStatus::Warning => {
            format!("ABV differs by {:.2}% — verify against COLAs tolerance rules.", diff)
        }
        VerificationStatus::Fail => {
            format!("ABV mismatch: expected {:.1}%, found {:.1}%.", exp_abv, closest)
        }
    };

    FieldResult {
        field: "Alcohol Content (ABV)".to_string(),
        status,
        expected: expected.to_string(),
        found: Some(format!("{:.1}%", closest)),
        message,
    }
}

fn check_net_contents(expected: &str, ocr_text: &str) -> FieldResult {
    // Match common volume formats: 750 mL, 1 L, 1.75L, 25.4 fl oz, etc.
    let vol_re = Regex::new(
        r"(?i)(\d+(?:\.\d+)?)\s*(ml|milliliter[s]?|l(?:iter[s]?)?|fl\.?\s*oz\.?|ounce[s]?)",
    )
    .unwrap();

    fn to_ml(val: f64, unit: &str) -> f64 {
        let u = unit.to_lowercase();
        let u = u.trim();
        if u.starts_with("fl") || u.contains("oz") {
            val * 29.5735
        } else if u == "l" || u.starts_with("liter") {
            val * 1000.0
        } else {
            val // mL
        }
    }

    let exp_ml = vol_re.captures(expected).and_then(|c| {
        let v: f64 = c[1].parse().ok()?;
        Some(to_ml(v, &c[2]))
    });

    let Some(exp_ml) = exp_ml else {
        return check_fuzzy("Net Contents", expected, ocr_text);
    };

    let found: Vec<(f64, String)> = vol_re
        .captures_iter(ocr_text)
        .filter_map(|c| {
            let v: f64 = c[1].parse().ok()?;
            let ml = to_ml(v, &c[2]);
            Some((ml, format!("{} {}", &c[1], &c[2])))
        })
        .collect();

    if found.is_empty() {
        return FieldResult {
            field: "Net Contents".to_string(),
            status: VerificationStatus::Fail,
            expected: expected.to_string(),
            found: None,
            message: "No net contents volume found on label.".to_string(),
        };
    }

    let (closest_ml, closest_str) = found
        .into_iter()
        .min_by(|(a, _), (b, _)| {
            (*a - exp_ml)
                .abs()
                .partial_cmp(&(*b - exp_ml).abs())
                .unwrap()
        })
        .unwrap();

    let pct_diff = ((closest_ml - exp_ml).abs() / exp_ml) * 100.0;

    if pct_diff <= 2.0 {
        FieldResult {
            field: "Net Contents".to_string(),
            status: VerificationStatus::Pass,
            expected: expected.to_string(),
            found: Some(closest_str),
            message: "Net contents matches.".to_string(),
        }
    } else {
        FieldResult {
            field: "Net Contents".to_string(),
            status: VerificationStatus::Fail,
            expected: expected.to_string(),
            found: Some(closest_str),
            message: format!("Net contents mismatch ({:.1}% off).", pct_diff),
        }
    }
}

fn check_government_warning(ocr_text: &str) -> FieldResult {
    const PREFIX: &str = "GOVERNMENT WARNING:";

    fn normalize(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    // Must have ALL CAPS prefix
    if !ocr_text.contains(PREFIX) {
        // Check if a case-variant exists
        if ocr_text.to_uppercase().contains(PREFIX) {
            // Found but not in all-caps
            return FieldResult {
                field: "Government Warning".to_string(),
                status: VerificationStatus::Fail,
                expected: "GOVERNMENT WARNING: [full text]".to_string(),
                found: Some("Detected but header is not ALL CAPS".to_string()),
                message: "\"GOVERNMENT WARNING:\" must appear in ALL CAPS per 27 CFR § 16.21."
                    .to_string(),
            };
        } else {
            return FieldResult {
                field: "Government Warning".to_string(),
                status: VerificationStatus::Fail,
                expected: "GOVERNMENT WARNING: [full text]".to_string(),
                found: None,
                message: "Government Warning Statement not found on label.".to_string(),
            };
        }
    }

    // Extract everything from the prefix onward
    let start = ocr_text.find(PREFIX).unwrap();
    let found_raw = &ocr_text[start..];
    let found_norm = normalize(found_raw);
    let exp_norm = normalize(GOVERNMENT_WARNING);

    // Check if expected text is contained verbatim (after whitespace normalisation)
    if found_norm.to_uppercase().contains(&exp_norm.to_uppercase()) {
        return FieldResult {
            field: "Government Warning".to_string(),
            status: VerificationStatus::Pass,
            expected: "GOVERNMENT WARNING: [full text]".to_string(),
            found: Some("Present and correct".to_string()),
            message: "Government Warning Statement is present and correctly formatted.".to_string(),
        };
    }

    // Fuzzy fallback: Tesseract may introduce noise on small text
    let sim = normalized_levenshtein(
        &found_norm.to_uppercase(),
        &exp_norm.to_uppercase(),
    );

    if sim >= 0.92 {
        FieldResult {
            field: "Government Warning".to_string(),
            status: VerificationStatus::Pass,
            expected: "GOVERNMENT WARNING: [full text]".to_string(),
            found: Some("Present (minor OCR noise detected)".to_string()),
            message: format!(
                "Warning text appears correct ({:.0}% similarity — minor OCR artifacts acceptable).",
                sim * 100.0
            ),
        }
    } else if sim >= 0.75 {
        FieldResult {
            field: "Government Warning".to_string(),
            status: VerificationStatus::Warning,
            expected: "GOVERNMENT WARNING: [full text]".to_string(),
            found: Some(found_norm.chars().take(120).collect::<String>() + "…"),
            message: format!(
                "Warning text is present but differs from required language ({:.0}% match) — manual review recommended.",
                sim * 100.0
            ),
        }
    } else {
        FieldResult {
            field: "Government Warning".to_string(),
            status: VerificationStatus::Fail,
            expected: "GOVERNMENT WARNING: [full text]".to_string(),
            found: Some(found_norm.chars().take(120).collect::<String>() + "…"),
            message: "Government Warning text does not match TTB-mandated language.".to_string(),
        }
    }
}
