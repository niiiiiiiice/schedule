use serde::{Deserialize, Serialize};

use crate::scheduler::events::SchedulerEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "aggregate_type", content = "payload")]
pub enum DomainEvent {
    Scheduler(SchedulerEvent),
}

impl DomainEvent {
    pub fn routing_key(&self) -> &'static str {
        match self {
            DomainEvent::Scheduler(e) => e.routing_key(),
        }
    }
}
