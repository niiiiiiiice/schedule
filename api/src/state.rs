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

#[derive(Clone)]
pub struct ResourceHandlers {
    pub create: Arc<CreateResourceHandler>,
    pub get: Arc<GetResourceHandler>,
    pub list: Arc<ListResourcesHandler>,
    pub get_schedule: Arc<GetResourceScheduleHandler>,
    pub get_available_slots: Arc<GetAvailableSlotsHandler>,
    pub create_schedule_rule: Arc<CreateScheduleRuleHandler>,
    pub update_schedule_rule: Arc<UpdateScheduleRuleHandler>,
    pub delete_schedule_rule: Arc<DeleteScheduleRuleHandler>,
}

#[derive(Clone)]
pub struct BookingHandlers {
    pub create: Arc<CreateBookingHandler>,
    pub get: Arc<GetBookingHandler>,
    pub cancel: Arc<CancelBookingHandler>,
}

#[derive(Clone)]
pub struct AppState {
    pub resources: ResourceHandlers,
    pub bookings: BookingHandlers,
}
