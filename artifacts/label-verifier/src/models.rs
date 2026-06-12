use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Pass,
    Fail,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldResult {
    pub field: String,
    pub status: VerificationStatus,
    pub expected: String,
    pub found: Option<String>,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct ApplicationData {
    pub brand_name: Option<String>,
    pub class_type: Option<String>,
    pub abv: Option<String>,
    pub net_contents: Option<String>,
    pub producer_name: Option<String>,
    pub country_of_origin: Option<String>,
    pub check_government_warning: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    pub fields: Vec<FieldResult>,
    pub overall_status: VerificationStatus,
    pub ocr_text: String,
    pub summary: String,
    pub filename: Option<String>,
}
