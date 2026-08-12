use intuigram_lib::{AccountKey, AccountLifecycle};

use super::*;

#[derive(Clone, Copy)]
struct LifecycleBackend;

impl ApplicationBackend for LifecycleBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        let Effect::AccountLifecycle { request } = effect.effect else {
            panic!("lifecycle test queued an unrelated effect")
        };
        Ok(BackendOutput::event(Some(
            AdapterEvent::AccountLifecycleReady(request),
        )))
    }
}

#[test]
fn completed_account_lifecycle_returns_the_owned_backend_to_startup() {
    let request = AccountLifecycle::Switch(AccountKey(20));
    let mut terminal = RecordingUi {
        views: Rc::new(RefCell::new(Vec::new())),
    };
    let mut events = AlwaysPendingEvents;
    let mut adapter_events = NoAdapterEvents;
    let mut app = intuigram_lib::App::new();
    let update = app.transition(intuigram_lib::Input::Adapter(AdapterEvent::Bootstrap(
        application_fixture(),
    )));
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let exit = runtime
        .block_on(run_application_state(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            LifecycleBackend,
            ApplicationState {
                app,
                update,
                pending_effects: VecDeque::from([AdapterEffect::new(Effect::AccountLifecycle {
                    request,
                })
                .expect("lifecycle effects need no random token")]),
                peers: intuigram_telegram::PeerDirectory::default(),
                media_limits: intuigram_telegram::MediaLimits::default(),
            },
        ))
        .expect("completed lifecycle should return to startup");

    assert!(
        matches!(exit, ApplicationExit::Lifecycle { request: actual, .. } if actual == request)
    );
}
