use utoipa::OpenApi;

use application::scheduler::dto::{AvailableSlot, BookingDto, ResourceDto, ScheduleIntervalDto};
use domain::scheduler::schedule_rule::{RecurrenceParams, RuleKind};

use crate::handlers::bookings::{CreateBookingBody};
use crate::handlers::resources::{CreateResourceBody, ScheduleRuleBody, ScheduleRuleResponse};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Scheduler API",
        version = "0.1.0",
        description = "Resource scheduling API — manage resources, availability rules, and bookings"
    ),
    paths(
        crate::handlers::resources::create_resource,
        crate::handlers::resources::get_resource,
        crate::handlers::resources::list_resources,
        crate::handlers::resources::get_resource_schedule,
        crate::handlers::resources::get_available_slots,
        crate::handlers::resources::create_schedule_rule,
        crate::handlers::resources::update_schedule_rule,
        crate::handlers::resources::delete_schedule_rule,
        crate::handlers::bookings::create_booking,
        crate::handlers::bookings::get_booking,
        crate::handlers::bookings::cancel_booking,
    ),
    components(
        schemas(
            ResourceDto,
            CreateResourceBody,
            AvailableSlot,
            ScheduleIntervalDto,
            ScheduleRuleBody,
            ScheduleRuleResponse,
            RuleKind,
            RecurrenceParams,
            BookingDto,
            CreateBookingBody,
        )
    ),
    tags(
        (name = "Resources", description = "Resource management and hierarchy"),
        (name = "Schedule Rules", description = "Availability and break rules for resources"),
        (name = "Bookings", description = "Create and manage bookings"),
    )
)]
pub struct ApiDoc;
