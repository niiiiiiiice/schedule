use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use domain::errors::DomainError;
use domain::events::DomainEvent;
use domain::scheduler::events::SchedulerEvent;

use crate::errors::AppError;
use crate::ports::EventDispatcher;
use crate::scheduler::ports::BookingRepository;

pub struct CancelBookingCommand {
    pub booking_id: Uuid,
}

pub struct CancelBookingHandler {
    booking_repo: Arc<dyn BookingRepository>,
    dispatcher: Arc<dyn EventDispatcher>,
}

impl CancelBookingHandler {
    pub fn new(booking_repo: Arc<dyn BookingRepository>, dispatcher: Arc<dyn EventDispatcher>) -> Self {
        Self { booking_repo, dispatcher }
    }

    pub async fn handle(&self, cmd: CancelBookingCommand) -> Result<(), AppError> {
        let mut booking = self
            .booking_repo
            .find_by_id(cmd.booking_id)
            .await?
            .ok_or_else(|| {
                AppError::Domain(DomainError::NotFound(format!("Booking {}", cmd.booking_id)))
            })?;

        booking.cancel()?;
        self.booking_repo.cancel(&booking).await?;

        self.dispatcher
            .dispatch(&[DomainEvent::Scheduler(SchedulerEvent::BookingCancelled {
                booking_id: cmd.booking_id,
                cancelled_at: Utc::now(),
            })])
            .await?;

        Ok(())
    }
}
