use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use application::errors::AppError;
use domain::errors::DomainError;
use domain::scheduler::errors::SchedulerError;

pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = &self.0;
        let (status, message) = match err {
            AppError::Domain(domain_err) => match domain_err {
                DomainError::Validation(_) => (StatusCode::BAD_REQUEST, err.to_string()),
                DomainError::NotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
                DomainError::Scheduler(sched_err) => match sched_err {
                    SchedulerError::Validation(_) => (StatusCode::BAD_REQUEST, err.to_string()),
                    SchedulerError::NotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
                    SchedulerError::ScheduleConflict => {
                        (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
                    }
                    SchedulerError::CapacityExceeded => {
                        (StatusCode::CONFLICT, err.to_string())
                    }
                    SchedulerError::RuleConflictsWithBookings(_) => {
                        (StatusCode::CONFLICT, err.to_string())
                    }
                    SchedulerError::InvalidTimeRange => {
                        (StatusCode::BAD_REQUEST, err.to_string())
                    }
                },
            },
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal database error".to_string(),
            ),
            AppError::Cache(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal cache error".to_string(),
            ),
            AppError::EventBus(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal event bus error".to_string(),
            ),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %err, "Internal error");
        }

        let body = json!({
            "error": {
                "code": status.as_u16(),
                "message": message,
            }
        });

        (status, Json(body)).into_response()
    }
}
