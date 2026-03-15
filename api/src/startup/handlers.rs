use std::sync::Arc;
use application::scheduler::commands::cancel_booking::CancelBookingHandler;
use application::scheduler::commands::create_booking::CreateBookingHandler;
use application::scheduler::commands::create_resource::CreateResourceHandler;
use application::scheduler::commands::create_schedule_rule::CreateScheduleRuleHandler;
use application::scheduler::commands::delete_schedule_rule::DeleteScheduleRuleHandler;
use application::scheduler::commands::update_schedule_rule::UpdateScheduleRuleHandler;
use application::scheduler::queries::get_available_slots::GetAvailableSlotsHandler;
use application::scheduler::queries::get_booking::GetBookingHandler;
use application::scheduler::queries::get_resource::GetResourceHandler;
use application::scheduler::queries::get_resource_schedule::GetResourceScheduleHandler;
use application::scheduler::queries::list_resources::ListResourcesHandler;
use infrastructure::startup::Infrastructure;
use crate::state::{AppState, BookingHandlers, ResourceHandlers};

pub fn build_app_state(infra: &Infrastructure) -> AppState {
    let resources = build_resource_handlers(infra);
    let bookings = build_booking_handlers(infra);

    AppState {
        resources,
        bookings,
    }
}

fn build_resource_handlers(infra: &Infrastructure) -> ResourceHandlers {
    ResourceHandlers {
        create: Arc::new(CreateResourceHandler::new(
            infra.resource_repo.clone(),
            infra.dispatcher.clone(),
        )),
        get: Arc::new(GetResourceHandler::new(infra.resource_repo.clone())),
        list: Arc::new(ListResourcesHandler::new(infra.resource_repo.clone())),
        get_schedule: Arc::new(GetResourceScheduleHandler::new(
            infra.resource_repo.clone(),
            infra.rule_repo.clone(),
            infra.interval_store.clone(),
        )),
        get_available_slots: Arc::new(GetAvailableSlotsHandler::new(
            infra.resource_repo.clone(),
            infra.rule_repo.clone(),
            infra.interval_store.clone(),
            infra.booking_repo.clone(),
        )),
        create_schedule_rule: Arc::new(CreateScheduleRuleHandler::new(
            infra.resource_repo.clone(),
            infra.rule_repo.clone(),
            infra.interval_store.clone(),
            infra.booking_repo.clone(),
            infra.dispatcher.clone(),
        )),
        update_schedule_rule: Arc::new(UpdateScheduleRuleHandler::new(
            infra.resource_repo.clone(),
            infra.rule_repo.clone(),
            infra.interval_store.clone(),
            infra.booking_repo.clone(),
            infra.dispatcher.clone(),
        )),
        delete_schedule_rule: Arc::new(DeleteScheduleRuleHandler::new(
            infra.resource_repo.clone(),
            infra.rule_repo.clone(),
            infra.interval_store.clone(),
            infra.booking_repo.clone(),
            infra.dispatcher.clone(),
        )),
    }
}

fn build_booking_handlers(infra: &Infrastructure) -> BookingHandlers {
    BookingHandlers {
        create: Arc::new(CreateBookingHandler::new(
            infra.resource_repo.clone(),
            infra.rule_repo.clone(),
            infra.interval_store.clone(),
            infra.booking_repo.clone(),
            infra.dispatcher.clone(),
        )),
        get: Arc::new(GetBookingHandler::new(infra.booking_repo.clone())),
        cancel: Arc::new(CancelBookingHandler::new(
            infra.booking_repo.clone(),
            infra.dispatcher.clone(),
        )),
    }
}