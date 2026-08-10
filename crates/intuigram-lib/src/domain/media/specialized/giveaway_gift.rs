/// Telegram giveaway lifecycle represented independently of TL constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GiveawayStateView {
    /// Entries are still being accepted.
    Active,

    /// Telegram has published the result set.
    Results,
}

/// Giveaway prize, eligibility, and result state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GiveawayView {
    /// Lifecycle state carried by the constructor.
    pub state: GiveawayStateView,

    /// Number of prizes advertised at launch.
    pub quantity: u32,

    /// Premium duration when Premium is the prize.
    pub premium_months: Option<u32>,

    /// Stars awarded per Telegram's giveaway metadata.
    pub stars: Option<u64>,

    /// Organizer-provided prize description.
    pub prize_description: Option<String>,

    /// Local date label for the giveaway deadline.
    pub until_date: String,

    /// Whether only new subscribers are eligible.
    pub only_new_subscribers: bool,

    /// Whether Telegram permits displaying the winners.
    pub winners_visible: bool,

    /// Eligible ISO 3166-1 country codes.
    pub country_codes: Vec<String>,

    /// Number of participating Channels.
    pub channel_count: u32,

    /// Winners reported by a results constructor.
    pub winners_count: Option<u32>,

    /// Prizes not claimed by a winner.
    pub unclaimed_count: Option<u32>,

    /// Whether Telegram reports that the giveaway was refunded.
    pub refunded: bool,

    /// Account-specific participation or published-result information.
    pub info: Option<GiveawayInfoView>,
}

/// Account-specific state returned by `payments.getGiveawayInfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GiveawayInfoView {
    /// Current participation and eligibility state.
    Active {
        /// Whether this Account currently participates.
        participating: bool,

        /// Whether Telegram is preparing the final result set.
        preparing_results: bool,

        /// Local start-date label.
        start_date: String,

        /// Eligibility reason when this Account cannot participate.
        eligibility_issue: Option<String>,
    },

    /// Account-specific published result.
    Results {
        /// Whether this Account won.
        winner: bool,

        /// Local start-date label.
        start_date: String,

        /// Local finish-date label.
        finish_date: String,

        /// Number of prizes already activated.
        activated_count: Option<u32>,

        /// Redeemable prize slug when Telegram supplies one.
        gift_code_slug: Option<String>,
    },
}

/// Telegram gift family normalized away from service-action constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GiftKindView {
    /// Telegram Premium duration.
    Premium,

    /// Telegram Stars balance.
    Stars,

    /// TON transfer.
    Ton,

    /// Redeemable Premium code.
    Code,

    /// Ordinary collectible Star Gift.
    StarGift,

    /// Numbered unique Star Gift.
    UniqueStarGift,
}

/// Structured gift value and lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GiftView {
    /// Semantic gift family.
    pub kind: GiftKindView,

    /// User-facing gift name.
    pub title: String,

    /// Stars carried by or assigned to the gift.
    pub stars: Option<u64>,

    /// Premium duration when applicable.
    pub days: Option<u32>,

    /// Fiat currency code reported for the gift purchase.
    pub currency: Option<String>,

    /// Fiat value in the currency's smallest units.
    pub amount_minor_units: Option<i64>,

    /// Cryptocurrency code reported by Telegram.
    pub crypto_currency: Option<String>,

    /// Cryptocurrency amount in Telegram's integer units.
    pub crypto_amount_minor_units: Option<i64>,

    /// Code slug, transaction ID, gift ID, or unique slug.
    pub identifier: Option<String>,

    /// Whether the recipient saved the collectible to their profile.
    pub saved: bool,

    /// Whether the collectible was converted to Stars.
    pub converted: bool,

    /// Whether the collectible was upgraded.
    pub upgraded: bool,

    /// Whether Telegram reports that the gift was refunded.
    pub refunded: bool,

    /// Whether the sender identity is intentionally hidden.
    pub anonymous: bool,
}
