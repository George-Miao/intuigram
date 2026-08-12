//! Context-sensitive action availability, menus, and transitions.

use super::*;

mod availability;
mod menu;
mod transition;

pub(in crate::app) use availability::move_index;
