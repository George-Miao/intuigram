mod commerce;
mod giveaway_gift;
mod location_game;
mod rendering;
mod story_todo;

pub use commerce::*;
pub use giveaway_gift::*;
pub use location_game::*;
use rendering::*;
pub use story_todo::*;

/// Structured specialized content retained for semantic rendering and actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpecializedMediaView {
    /// A location shared for a bounded period.
    LiveLocation(LiveLocationView),

    /// A Telegram game, retained without an unsafe implicit launch.
    Game(GameView),

    /// A Telegram invoice, retained without an implicit purchase action.
    Invoice(InvoiceView),

    /// Paid media disclosure state, retained without an implicit purchase
    /// action.
    PaidMedia(PaidMediaView),

    /// Active giveaway or published giveaway results.
    Giveaway(GiveawayView),

    /// Gift metadata carried by a Telegram service Message.
    Gift(GiftView),

    /// Story shared into a Telegram Message.
    Story(SharedStoryView),

    /// Collaborative TODO list.
    TodoList(TodoListView),
}

impl SpecializedMediaView {
    pub(super) fn display_description(&self) -> String {
        match self {
            Self::LiveLocation(location) => location.coordinates(),
            Self::Game(game) => game.description.clone(),
            Self::Invoice(invoice) => invoice.description.clone(),
            Self::PaidMedia(media) => format!(
                "{} Stars · {} {}",
                media.stars_amount,
                media.items.len(),
                if media.items.len() == 1 {
                    "item"
                } else {
                    "items"
                },
            ),
            Self::Giveaway(giveaway) => giveaway_description(giveaway),
            Self::Gift(gift) => gift_description(gift),
            Self::Story(story) => story_description(story),
            Self::TodoList(todo) => todo.title.clone(),
        }
    }

    pub(super) fn display_details(&self) -> Vec<String> {
        match self {
            Self::LiveLocation(location) => live_location_details(location),
            Self::Game(game) => vec![format!("game · {}", game.short_name)],
            Self::Invoice(invoice) => invoice_details(invoice),
            Self::PaidMedia(media) => paid_media_details(media),
            Self::Giveaway(giveaway) => giveaway_details(giveaway),
            Self::Gift(gift) => gift_details(gift),
            Self::Story(story) => story_details(story),
            Self::TodoList(todo) => todo_details(todo),
        }
    }
}
