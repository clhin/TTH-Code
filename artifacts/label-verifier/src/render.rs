use crate::models::{FieldResult, VerificationResult, VerificationStatus};

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

pub fn error_fragment(msg: &str) -> actix_web::HttpResponse {
    let html = format!(
        r#"<div class="rounded-lg border border-red-300 bg-red-50 p-4 flex items-start gap-3">
          <span class="text-red-500 text-lg mt-0.5">✕</span>
          <div>
            <p class="font-semibold text-red-700">Error</p>
            <p class="text-red-600 text-sm mt-1">{}</p>
          </div>
        </div>"#,
        html_escape(msg)
    );
    actix_web::HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

pub fn single_result_html(r: &VerificationResult) -> String {
    let (banner_class, banner_icon, banner_label) = status_banner(&r.overall_status);

    let rows: String = r.fields.iter().map(field_row).collect();
    let ocr_preview = html_escape(truncate(&r.ocr_text, 800));

    format!(
        r#"
<!-- overall banner -->
<div class="rounded-lg {banner_class} px-5 py-4 flex items-center gap-3 mb-6">
  <span class="text-2xl">{banner_icon}</span>
  <div>
    <p class="font-bold text-lg">{banner_label}</p>
    <p class="text-sm opacity-80">{summary}</p>
  </div>
</div>

<!-- field table -->
<div class="overflow-x-auto rounded-lg border border-gray-200">
  <table class="w-full text-sm">
    <thead>
      <tr class="bg-gray-50 border-b border-gray-200">
        <th class="text-left px-4 py-3 font-semibold text-gray-600 w-40">Field</th>
        <th class="text-left px-4 py-3 font-semibold text-gray-600">Expected</th>
        <th class="text-left px-4 py-3 font-semibold text-gray-600">Found on Label</th>
        <th class="text-left px-4 py-3 font-semibold text-gray-600 w-28">Result</th>
      </tr>
    </thead>
    <tbody class="divide-y divide-gray-100">
      {rows}
    </tbody>
  </table>
</div>

<!-- OCR text disclosure -->
<details class="mt-4">
  <summary class="cursor-pointer text-sm text-gray-500 hover:text-gray-700 select-none">
    View raw extracted text
  </summary>
  <pre class="mt-2 p-3 bg-gray-50 rounded border border-gray-200 text-xs text-gray-700 whitespace-pre-wrap overflow-x-auto max-h-64">{ocr_preview}</pre>
</details>
"#,
        banner_class = banner_class,
        banner_icon = banner_icon,
        banner_label = banner_label,
        summary = html_escape(&r.summary),
        rows = rows,
        ocr_preview = ocr_preview,
    )
}

pub fn batch_results_html(results: &[VerificationResult]) -> String {
    let total = results.len();
    let passed = results
        .iter()
        .filter(|r| r.overall_status == VerificationStatus::Pass)
        .count();
    let failed = results
        .iter()
        .filter(|r| r.overall_status == VerificationStatus::Fail)
        .count();
    let warned = results
        .iter()
        .filter(|r| r.overall_status == VerificationStatus::Warning)
        .count();

    let summary_class = if failed > 0 {
        "bg-red-50 border-red-300 text-red-800"
    } else if warned > 0 {
        "bg-amber-50 border-amber-300 text-amber-800"
    } else {
        "bg-green-50 border-green-300 text-green-800"
    };

    let rows: String = results.iter().enumerate().map(|(i, r)| {
        let (_, icon, label) = status_banner(&r.overall_status);
        let name = r.filename.as_deref().unwrap_or("—");
        let field_count = r.fields.len();
        let fail_count = r.fields.iter().filter(|f| f.status == VerificationStatus::Fail).count();
        let warn_count = r.fields.iter().filter(|f| f.status == VerificationStatus::Warning).count();
        let details_id = format!("batch-details-{}", i);

        let field_rows: String = r.fields.iter().map(field_row).collect();
        let ocr_preview = html_escape(truncate(&r.ocr_text, 400));

        format!(r#"
          <tr class="border-b border-gray-100 hover:bg-gray-50 cursor-pointer"
              onclick="document.getElementById('{details_id}').classList.toggle('hidden')">
            <td class="px-4 py-3 text-sm font-medium text-gray-700">{idx}</td>
            <td class="px-4 py-3 text-sm text-gray-700 max-w-xs truncate">{name}</td>
            <td class="px-4 py-3 text-sm text-center">{icon} {label}</td>
            <td class="px-4 py-3 text-sm text-gray-500">{field_count} fields · {fail_count} fail · {warn_count} warn</td>
          </tr>
          <tr id="{details_id}" class="hidden bg-gray-50">
            <td colspan="4" class="px-6 py-4">
              <div class="overflow-x-auto rounded border border-gray-200">
                <table class="w-full text-xs">
                  <thead><tr class="bg-gray-100">
                    <th class="text-left px-3 py-2 font-semibold text-gray-600 w-36">Field</th>
                    <th class="text-left px-3 py-2 font-semibold text-gray-600">Expected</th>
                    <th class="text-left px-3 py-2 font-semibold text-gray-600">Found</th>
                    <th class="text-left px-3 py-2 font-semibold text-gray-600 w-24">Result</th>
                  </tr></thead>
                  <tbody class="divide-y divide-gray-100">{field_rows}</tbody>
                </table>
              </div>
              <details class="mt-2">
                <summary class="text-xs text-gray-400 cursor-pointer">Raw OCR text</summary>
                <pre class="mt-1 p-2 bg-white rounded border text-xs text-gray-600 whitespace-pre-wrap max-h-40 overflow-y-auto">{ocr_preview}</pre>
              </details>
            </td>
          </tr>
        "#,
            details_id = details_id,
            idx = i + 1,
            name = html_escape(name),
            icon = icon,
            label = label,
            field_count = field_count,
            fail_count = fail_count,
            warn_count = warn_count,
            field_rows = field_rows,
            ocr_preview = ocr_preview,
        )
    }).collect();

    format!(
        r#"
<!-- batch summary banner -->
<div class="rounded-lg border {summary_class} px-5 py-4 mb-5">
  <p class="font-bold text-base">{total} labels processed</p>
  <p class="text-sm opacity-80 mt-0.5">{passed} passed · {warned} review needed · {failed} failed</p>
</div>

<!-- batch table -->
<div class="overflow-x-auto rounded-lg border border-gray-200">
  <table class="w-full text-sm">
    <thead>
      <tr class="bg-gray-50 border-b border-gray-200">
        <th class="text-left px-4 py-3 font-semibold text-gray-600 w-12">#</th>
        <th class="text-left px-4 py-3 font-semibold text-gray-600">File</th>
        <th class="text-left px-4 py-3 font-semibold text-gray-600 w-36">Overall</th>
        <th class="text-left px-4 py-3 font-semibold text-gray-600">Details</th>
      </tr>
    </thead>
    <tbody>
      {rows}
    </tbody>
  </table>
</div>
<p class="text-xs text-gray-400 mt-2">Click any row to expand field-level details.</p>
"#,
        summary_class = summary_class,
        total = total,
        passed = passed,
        warned = warned,
        failed = failed,
        rows = rows,
    )
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn field_row(f: &FieldResult) -> String {
    let (row_bg, badge) = match f.status {
        VerificationStatus::Pass => ("", r#"<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700">✓ Pass</span>"#),
        VerificationStatus::Fail => ("bg-red-50", r#"<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700">✕ Fail</span>"#),
        VerificationStatus::Warning => ("bg-amber-50", r#"<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700">⚠ Review</span>"#),
    };

    let found_cell = match &f.found {
        Some(v) => html_escape(v),
        None => "<span class=\"text-gray-400 italic\">not detected</span>".to_string(),
    };

    format!(
        r#"<tr class="{row_bg}">
          <td class="px-4 py-3 font-medium text-gray-700 whitespace-nowrap">{field}</td>
          <td class="px-4 py-3 text-gray-600">{expected}</td>
          <td class="px-4 py-3 text-gray-600">
            {found}
            <p class="text-xs text-gray-400 mt-0.5">{message}</p>
          </td>
          <td class="px-4 py-3">{badge}</td>
        </tr>"#,
        row_bg = row_bg,
        field = html_escape(&f.field),
        expected = html_escape(&f.expected),
        found = found_cell,
        message = html_escape(&f.message),
        badge = badge,
    )
}

fn status_banner(status: &VerificationStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        VerificationStatus::Pass => (
            "bg-green-50 border border-green-300 text-green-800",
            "✓",
            "All Checks Passed",
        ),
        VerificationStatus::Fail => (
            "bg-red-50 border border-red-300 text-red-800",
            "✕",
            "Verification Failed",
        ),
        VerificationStatus::Warning => (
            "bg-amber-50 border border-amber-300 text-amber-800",
            "⚠",
            "Manual Review Required",
        ),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
