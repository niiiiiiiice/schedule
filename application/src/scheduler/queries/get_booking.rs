use std::sync::Arc;

use uuid::Uuid;

use domain::errors::DomainError;

use crate::errors::AppError;
use crate::scheduler::dto::BookingDto;
use crate::scheduler::ports::BookingRepository;

pub struct GetBookingQuery {
    pub booking_id: Uuid,
}

pub struct GetBookingHandler {
    repo: Arc<dyn BookingRepository>,
}

impl GetBookingHandler {
    pub fn new(repo: Arc<dyn BookingRepository>) -> Self {
        Self { repo }
    }

    pub async fn handle(&self, query: GetBookingQuery) -> Result<BookingDto, AppError> {
        self.repo
            .find_by_id(query.booking_id)
            .await?
            .map(BookingDto::from)
            .ok_or_else(|| {
                AppError::Domain(DomainError::NotFound(format!("Booking {}", query.booking_id)))
            })
    }
}
