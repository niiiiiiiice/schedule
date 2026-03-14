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
pub struct AppState {
    pub create_resource: Arc<CreateResourceHandler>,
    pub get_resource: Arc<GetResourceHandler>,
    pub list_resources: Arc<ListResourcesHandler>,
    pub get_resource_schedule: Arc<GetResourceScheduleHandler>,
    pub get_available_slots: Arc<GetAvailableSlotsHandler>,
    pub create_schedule_rule: Arc<CreateScheduleRuleHandler>,
    pub update_schedule_rule: Arc<UpdateScheduleRuleHandler>,
    pub delete_schedule_rule: Arc<DeleteScheduleRuleHandler>,
    pub create_booking: Arc<CreateBookingHandler>,
    pub get_booking: Arc<GetBookingHandler>,
    pub cancel_booking: Arc<CancelBookingHandler>,
}
