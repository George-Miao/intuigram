use compio::runtime::ResumeUnwind;
use snafu::{ResultExt, Snafu};

use super::*;

/// Result of driving a pending Account operation alongside terminal input.
pub(super) enum Loading<T> {
    Ready(T),
    Exit(AccountSessionExit),
}

/// Account data prepared away from the terminal event loop.
pub(super) enum PreparedAccount {
    Ready(Box<Bootstrap>),
    Recovery(Box<intuigram_store::AccountRecovery>),
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(super)))]
pub(super) enum Error {
    #[snafu(display("failed to render Account loading state"))]
    Terminal { source: intuigram_tui::Error },

    #[snafu(display("failed to encrypt Account data for Local Lock"))]
    EnableLocalLock {
        source: intuigram_store::SecurityError,
    },

    #[snafu(display("failed to load Account data"))]
    AccountDatabase { source: intuigram_store::Error },
}

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

pub(super) async fn prepare_account(
    layout: StoreLayout,
    account: AccountRecord,
    cipher: AccountCipher,
    accounts: Vec<AccountView>,
) -> Result<PreparedAccount> {
    compio::runtime::spawn_blocking(move || {
        if cipher.is_encrypted() {
            intuigram_store::enable_local_lock(&layout, account.id, &cipher)
                .context(EnableLocalLockSnafu)?;
        }
        match AccountDatabase::open_recoverable_with_cipher(&layout, account.id, cipher)
            .context(AccountDatabaseSnafu)?
        {
            AccountOpen::Ready(database) => prepare_cached(database, account, accounts),
            AccountOpen::Recovery(recovery) => Ok(PreparedAccount::Recovery(recovery)),
        }
    })
    .await
    .resume_unwind()
    .expect("an awaited Account preparation cannot be cancelled")
}

pub(super) async fn prepare_recovered(
    database: AccountDatabase,
    account: AccountRecord,
    accounts: Vec<AccountView>,
) -> Result<PreparedAccount> {
    compio::runtime::spawn_blocking(move || prepare_cached(database, account, accounts))
        .await
        .resume_unwind()
        .expect("an awaited recovered Account preparation cannot be cancelled")
}

fn prepare_cached(
    database: AccountDatabase,
    account: AccountRecord,
    accounts: Vec<AccountView>,
) -> Result<PreparedAccount> {
    let cached = database.cached_account().context(AccountDatabaseSnafu)?;
    drop(database);
    let mut bootstrap =
        cached_bootstrap(account.display_name, account.notification_identity, cached);
    bootstrap.accounts = accounts;
    Ok(PreparedAccount::Ready(Box::new(bootstrap)))
}

pub(super) async fn wait_for_account_load<U, E, F, T>(
    terminal: &mut U,
    events: &mut E,
    account_name: String,
    notification_identity: String,
    accounts: Vec<AccountView>,
    load: F,
) -> Result<Loading<T>>
where
    U: ApplicationUi,
    E: ApplicationEvents,
    F: Future<Output = T>,
{
    let mut bootstrap = cached_bootstrap(
        account_name,
        notification_identity,
        CachedAccount::default(),
    );
    bootstrap.accounts = accounts;
    let mut app = App::new();
    let mut update = app.transition(Input::Adapter(AdapterEvent::Bootstrap(bootstrap)));
    let mut load = Box::pin(load);
    let mut animation = Box::pin(compio::time::sleep(Duration::from_millis(90)));
    let mut draw_requested = true;

    loop {
        if draw_requested {
            terminal.draw(&update.view).context(TerminalSnafu)?;
            draw_requested = false;
        }
        if let Some(effect) = update.effect.take() {
            match effect {
                Effect::Quit => return Ok(Loading::Exit(AccountSessionExit::Quit)),
                Effect::AccountLifecycle { request } => {
                    return Ok(Loading::Exit(AccountSessionExit::Lifecycle(request)));
                }
                _ => {}
            }
        }

        enum Wake<T, L> {
            Terminal(T),
            Redraw(intuigram_tui::Result<()>),
            Loaded(L),
            Animate,
        }

        let wake = poll_fn(|cx| {
            if let Poll::Ready(result) = terminal.poll_redraw(cx) {
                return Poll::Ready(Wake::Redraw(result));
            }
            if let Poll::Ready(event) = events.poll_next_event(cx) {
                return Poll::Ready(Wake::Terminal(event));
            }
            if let Poll::Ready(loaded) = load.as_mut().poll(cx) {
                return Poll::Ready(Wake::Loaded(loaded));
            }
            if animation.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Wake::Animate);
            }
            Poll::Pending
        })
        .await;

        match wake {
            Wake::Redraw(result) => {
                result.context(TerminalSnafu)?;
                draw_requested = true;
            }
            Wake::Terminal(event) => {
                let event = event.context(TerminalSnafu)?;
                match terminal.resolve_event(&update.view, event) {
                    Some(UiEvent::Redraw) => draw_requested = true,
                    Some(UiEvent::Intent(intent)) => {
                        let next = app.transition(Input::Intent(intent));
                        draw_requested |= update.view != next.view;
                        update = next;
                    }
                    None => {}
                }
            }
            Wake::Loaded(loaded) => return Ok(Loading::Ready(loaded)),
            Wake::Animate => {
                animation = Box::pin(compio::time::sleep(Duration::from_millis(90)));
                let next = app.transition(Input::Intent(Intent::Animate));
                draw_requested |= update.view != next.view;
                update = next;
            }
        }
    }
}
