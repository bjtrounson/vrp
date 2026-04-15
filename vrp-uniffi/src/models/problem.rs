use serde::{Deserialize, Serialize};

#[derive(Clone, uniffi::Enum, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Location {
    Coordinate { lat: f64, lng: f64 },
    Reference { index: u32 },
}

#[derive(Clone, uniffi::Enum, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationType {
    Any,
    Sequence,
    Strict,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    #[serde(rename(deserialize = "type", serialize = "type"))]
    pub type_field: RelationType,
    pub jobs: Vec<String>,
    pub vehicle_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift_index: Option<u32>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSkills {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_of: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_of: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub none_of: Option<Vec<String>>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct JobPlace {
    pub location: Location,
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub times: Option<Vec<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct JobTask {
    pub places: Vec<JobPlace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub demand: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Job {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pickups: Option<Vec<JobTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deliveries: Option<Vec<JobTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacements: Option<Vec<JobTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<JobTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<JobSkills>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Plan {
    pub jobs: Vec<Job>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<Relation>>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct VehicleCosts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed: Option<f64>,
    pub distance: f64,
    pub time: f64,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct ShiftStart {
    pub earliest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    pub location: Location,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct ShiftEnd {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest: Option<String>,
    pub latest: String,
    pub location: Location,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct VehicleShift {
    pub start: ShiftStart,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<ShiftEnd>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "shiftTime")]
    pub max_duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tour_size: Option<u32>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct VehicleProfile {
    pub matrix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleType {
    pub type_id: String,
    pub vehicle_ids: Vec<String>,
    pub profile: VehicleProfile,
    pub costs: VehicleCosts,
    pub shifts: Vec<VehicleShift>,
    pub capacity: Vec<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<VehicleLimits>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct MatrixProfile {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Fleet {
    pub vehicles: Vec<VehicleType>,
    pub profiles: Vec<MatrixProfile>,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
pub struct Problem {
    pub plan: Plan,
    pub fleet: Fleet,
}

#[derive(Clone, uniffi::Record, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Matrix {
    pub profile: Option<String>,
    pub timestamp: Option<String>,
    #[serde(alias = "durations")]
    pub travel_times: Vec<i64>,
    pub distances: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_codes: Option<Vec<i64>>,
}
