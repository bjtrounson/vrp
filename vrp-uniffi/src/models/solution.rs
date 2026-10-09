use serde::{Deserialize, Serialize};

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Timing {
    pub driving: i64,
    pub serving: i64,
    pub waiting: i64,
    #[serde(rename(serialize = "break", deserialize = "break"))]
    pub break_time: i64,
    #[serde(default = "i64::default")]
    pub commuting: i64,
    #[serde(default = "i64::default")]
    pub parking: i64,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Statistic {
    pub cost: f64,
    pub distance: i64,
    pub duration: i64,
    pub times: Timing,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Schedule {
    pub arrival: String,
    pub departure: String,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Interval {
    pub start: String,
    pub end: String,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub location: super::problem::Location,
    pub time: Schedule,
    pub distance: i64,
    pub load: Vec<i32>,
    pub activities: Vec<Activity>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub job_id: String,
    #[serde(rename(deserialize = "type", serialize = "type"))]
    pub type_field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<super::problem::Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Interval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_tag: Option<String>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tour {
    pub vehicle_id: String,
    pub type_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift_index: Option<u32>,
    pub stops: Vec<Stop>,
    pub statistic: Statistic,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnassignedJob {
    pub job_id: String,
    pub reasons: Vec<UnassignedJobReason>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct UnassignedJobReason {
    pub code: String,
    pub description: String,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Solution {
    pub statistic: Statistic,
    pub tours: Vec<Tour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unassigned: Option<Vec<UnassignedJob>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violations: Option<Vec<String>>,
}
