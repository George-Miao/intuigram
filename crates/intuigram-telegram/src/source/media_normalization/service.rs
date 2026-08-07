use super::*;

pub(crate) fn service_event_description(action: &tl::enums::MessageAction) -> String {
    match action {
        tl::enums::MessageAction::Empty => "Empty Telegram service event".to_owned(),
        tl::enums::MessageAction::ChatCreate(action) => {
            format!("Created group “{}”", action.title)
        }
        tl::enums::MessageAction::ChatEditTitle(action) => {
            format!("Changed the Chat title to “{}”", action.title)
        }
        tl::enums::MessageAction::ChatEditPhoto(_) => "Changed the Chat photo".to_owned(),
        tl::enums::MessageAction::ChatDeletePhoto => "Removed the Chat photo".to_owned(),
        tl::enums::MessageAction::ChatAddUser(action) => {
            format!("Added {} member(s)", action.users.len())
        }
        tl::enums::MessageAction::ChatDeleteUser(_) => "Removed a member".to_owned(),
        tl::enums::MessageAction::ChatJoinedByLink(_) => "Joined through an invite link".to_owned(),
        tl::enums::MessageAction::ChannelCreate(action) => {
            format!("Created Channel “{}”", action.title)
        }
        tl::enums::MessageAction::ChatMigrateTo(_) => "Upgraded group to a Supergroup".to_owned(),
        tl::enums::MessageAction::ChannelMigrateFrom(_) => {
            "Migrated history from a Basic Group".to_owned()
        }
        tl::enums::MessageAction::PinMessage => "Pinned a Message".to_owned(),
        tl::enums::MessageAction::HistoryClear => "Cleared Chat history".to_owned(),
        tl::enums::MessageAction::GameScore(_) => "Updated a game score".to_owned(),
        tl::enums::MessageAction::PaymentSentMe(_) => "Received a payment".to_owned(),
        tl::enums::MessageAction::PaymentSent(_) => "Sent a payment".to_owned(),
        tl::enums::MessageAction::PhoneCall(_) => "Telegram call".to_owned(),
        tl::enums::MessageAction::ScreenshotTaken => "Took a screenshot".to_owned(),
        tl::enums::MessageAction::CustomAction(action) => action.message.clone(),
        tl::enums::MessageAction::BotAllowed(_) => "Allowed a bot to message".to_owned(),
        tl::enums::MessageAction::SecureValuesSentMe(_) => {
            "Received Telegram Passport data".to_owned()
        }
        tl::enums::MessageAction::SecureValuesSent(_) => "Sent Telegram Passport data".to_owned(),
        tl::enums::MessageAction::ContactSignUp => "Joined Telegram".to_owned(),
        tl::enums::MessageAction::GeoProximityReached(_) => {
            "Reached a live-location proximity alert".to_owned()
        }
        tl::enums::MessageAction::GroupCall(_) => "Changed the group call".to_owned(),
        tl::enums::MessageAction::InviteToGroupCall(_) => {
            "Invited members to a group call".to_owned()
        }
        tl::enums::MessageAction::SetMessagesTtl(_) => {
            "Changed the Message auto-delete timer".to_owned()
        }
        tl::enums::MessageAction::GroupCallScheduled(_) => "Scheduled a group call".to_owned(),
        tl::enums::MessageAction::SetChatTheme(_) => "Changed the Chat theme".to_owned(),
        tl::enums::MessageAction::ChatJoinedByRequest => "Joined after approval".to_owned(),
        tl::enums::MessageAction::WebViewDataSentMe(_) => "Received bot web-app data".to_owned(),
        tl::enums::MessageAction::WebViewDataSent(_) => "Sent bot web-app data".to_owned(),
        tl::enums::MessageAction::GiftPremium(_) => "Gifted Telegram Premium".to_owned(),
        tl::enums::MessageAction::TopicCreate(action) => {
            format!("Created Topic “{}”", action.title)
        }
        tl::enums::MessageAction::TopicEdit(_) => "Changed a Topic".to_owned(),
        tl::enums::MessageAction::SuggestProfilePhoto(_) => "Suggested a profile photo".to_owned(),
        tl::enums::MessageAction::RequestedPeer(_) => "Shared a requested Chat or user".to_owned(),
        tl::enums::MessageAction::SetChatWallPaper(_) => "Changed the Chat wallpaper".to_owned(),
        tl::enums::MessageAction::GiftCode(_) => "Sent a gift code".to_owned(),
        tl::enums::MessageAction::GiveawayLaunch(_) => "Started a giveaway".to_owned(),
        tl::enums::MessageAction::GiveawayResults(_) => "Published giveaway results".to_owned(),
        tl::enums::MessageAction::BoostApply(_) => "Applied boosts to the Chat".to_owned(),
        tl::enums::MessageAction::RequestedPeerSentMe(_) => {
            "Received a requested Chat or user".to_owned()
        }
        tl::enums::MessageAction::PaymentRefunded(_) => "Refunded a payment".to_owned(),
        tl::enums::MessageAction::GiftStars(_) => "Gifted Telegram Stars".to_owned(),
        tl::enums::MessageAction::PrizeStars(_) => "Awarded Telegram Stars".to_owned(),
        tl::enums::MessageAction::StarGift(_) => "Sent a Star Gift".to_owned(),
        tl::enums::MessageAction::StarGiftUnique(_) => "Sent a unique Star Gift".to_owned(),
        tl::enums::MessageAction::PaidMessagesRefunded(_) => "Refunded paid Messages".to_owned(),
        tl::enums::MessageAction::PaidMessagesPrice(_) => {
            "Changed the paid-Message price".to_owned()
        }
        tl::enums::MessageAction::ConferenceCall(_) => "Changed the conference call".to_owned(),
        tl::enums::MessageAction::TodoCompletions(_) => "Updated TODO completion".to_owned(),
        tl::enums::MessageAction::TodoAppendTasks(_) => "Added TODO tasks".to_owned(),
        tl::enums::MessageAction::SuggestedPostApproval(_) => {
            "Reviewed a suggested post".to_owned()
        }
        tl::enums::MessageAction::SuggestedPostSuccess(_) => {
            "Published a suggested post".to_owned()
        }
        tl::enums::MessageAction::SuggestedPostRefund(_) => "Refunded a suggested post".to_owned(),
        tl::enums::MessageAction::GiftTon(_) => "Gifted TON".to_owned(),
        tl::enums::MessageAction::SuggestBirthday(_) => "Suggested a birthday".to_owned(),
        tl::enums::MessageAction::StarGiftPurchaseOffer(_) => {
            "Offered to buy a Star Gift".to_owned()
        }
        tl::enums::MessageAction::StarGiftPurchaseOfferDeclined(_) => {
            "Declined a Star Gift purchase offer".to_owned()
        }
        tl::enums::MessageAction::NewCreatorPending(_) => {
            "Started an ownership transfer".to_owned()
        }
        tl::enums::MessageAction::ChangeCreator(_) => "Transferred Chat ownership".to_owned(),
        tl::enums::MessageAction::NoForwardsToggle(_) => "Changed content protection".to_owned(),
        tl::enums::MessageAction::NoForwardsRequest(_) => "Requested content protection".to_owned(),
        tl::enums::MessageAction::PollAppendAnswer(_) => "Added a poll answer".to_owned(),
        tl::enums::MessageAction::PollDeleteAnswer(_) => "Removed a poll answer".to_owned(),
        tl::enums::MessageAction::ManagedBotCreated(_) => "Created a managed bot".to_owned(),
    }
}
