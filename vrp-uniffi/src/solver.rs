use std::io::BufReader;
use std::sync::Arc;
use vrp_cli::extensions::solve::config::{read_config, Config};
use vrp_cli::get_solution_serialized;
use vrp_pragmatic::format::problem::{deserialize_matrix, deserialize_problem, PragmaticProblem};
use vrp_pragmatic::format::CoordIndex;
use vrp_pragmatic::validation::ValidationContext;

use crate::models::problem::{Matrix, Problem};
use crate::models::solution::Solution;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum VrpError {
    #[error("Format error: {message}")]
    FormatError { message: String },
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    #[error("Solving error: {message}")]
    SolvingError { message: String },
}

#[derive(uniffi::Object)]
pub struct VrpSolver;

#[uniffi::export]
impl VrpSolver {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {}
    }

    /// Validates the pragmatic problem and matrices.
    pub fn validate(&self, problem: Problem, matrices: Vec<Matrix>) -> Result<(), VrpError> {
        let problem_json =
            serde_json::to_string(&problem).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
        let core_problem = deserialize_problem(BufReader::new(problem_json.as_bytes()))
            .map_err(|e| VrpError::FormatError { message: e.to_string() })?;

        let mut core_matrices = Vec::new();
        for matrix in matrices {
            let matrix_json =
                serde_json::to_string(&matrix).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
            let core_matrix = deserialize_matrix(BufReader::new(matrix_json.as_bytes()))
                .map_err(|e| VrpError::FormatError { message: e.to_string() })?;
            core_matrices.push(core_matrix);
        }

        let matrices_ref = if core_matrices.is_empty() { None } else { Some(&core_matrices) };
        let coord_index = CoordIndex::new(&core_problem);

        ValidationContext::new(&core_problem, matrices_ref, &coord_index)
            .validate()
            .map_err(|errs| VrpError::ValidationError { message: errs.to_string() })?;

        Ok(())
    }

    /// Solves the routing problem. Config is passed as a JSON string for flexibility.
    pub fn solve(
        &self,
        problem: Problem,
        matrices: Vec<Matrix>,
        config_json: Option<String>,
    ) -> Result<Solution, VrpError> {
        let problem_json =
            serde_json::to_string(&problem).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
        let core_problem = deserialize_problem(BufReader::new(problem_json.as_bytes()))
            .map_err(|e| VrpError::FormatError { message: e.to_string() })?;

        let mut core_matrices = Vec::new();
        for matrix in matrices {
            let matrix_json =
                serde_json::to_string(&matrix).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
            let core_matrix = deserialize_matrix(BufReader::new(matrix_json.as_bytes()))
                .map_err(|e| VrpError::FormatError { message: e.to_string() })?;
            core_matrices.push(core_matrix);
        }

        // Read pragmatic problem using standard vrp_pragmatic methods
        let core_problem = if core_matrices.is_empty() {
            core_problem.read_pragmatic()
        } else {
            (core_problem, core_matrices).read_pragmatic()
        }
        .map_err(|errs| VrpError::ValidationError { message: errs.to_string() })?;

        let config = if let Some(cfg) = config_json {
            read_config(BufReader::new(cfg.as_bytes())).map_err(|e| VrpError::FormatError { message: e.to_string() })?
        } else {
            Config::default()
        };

        // Call cli solver wrapper which returns a JSON string
        let solution_json = get_solution_serialized(Arc::new(core_problem), config)
            .map_err(|e| VrpError::SolvingError { message: e.to_string() })?;

        // Deserialize the resulting JSON string into our UniFFI Solution model
        let solution: Solution = serde_json::from_str(&solution_json)
            .map_err(|e| VrpError::FormatError { message: format!("Failed to parse solution: {}", e.to_string()) })?;

        Ok(solution)
    }

    /// Retrieves unique routing locations from a given problem.
    pub fn get_routing_locations(&self, problem: Problem) -> Result<Vec<crate::models::problem::Location>, VrpError> {
        let problem_json =
            serde_json::to_string(&problem).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
        let core_problem = deserialize_problem(BufReader::new(problem_json.as_bytes()))
            .map_err(|e| VrpError::FormatError { message: e.to_string() })?;

        let locations = vrp_pragmatic::get_unique_locations(&core_problem);

        let mut result = Vec::new();
        for loc in locations {
            let loc_json = serde_json::to_string(&loc).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
            let uniffi_loc: crate::models::problem::Location =
                serde_json::from_str(&loc_json).map_err(|e| VrpError::FormatError { message: e.to_string() })?;
            result.push(uniffi_loc);
        }

        Ok(result)
    }
}
