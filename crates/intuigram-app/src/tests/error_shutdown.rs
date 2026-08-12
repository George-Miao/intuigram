use super::effect_fairness::{BurstAdapterEvents, QuitAfterNotifications};
use super::*;

#[derive(Clone)]
struct FatalBackend {
    shutdown: Rc<Cell<bool>>,
}

impl ApplicationBackend for FatalBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        if matches!(effect.effect, Effect::Notify { .. }) {
            return Err(super::super::Error::TelegramActorMailboxClosed);
        }
        Ok(BackendOutput::event(None))
    }

    async fn shutdown(self) -> Result<()> {
        self.shutdown.set(true);
        Ok(())
    }
}

#[test]
fn fatal_backend_error_still_joins_backend_before_returning() {
    let shutdown = Rc::new(Cell::new(false));
    let mut terminal = RecordingUi {
        views: Rc::new(std::cell::RefCell::new(Vec::new())),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::new(Cell::new(0)),
        expected: usize::MAX,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: 1,
        emitted: Rc::new(Cell::new(0)),
    };
    let backend = FatalBackend {
        shutdown: Rc::clone(&shutdown),
    };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let result = runtime.block_on(run_application(
        &mut terminal,
        &mut events,
        &mut adapter_events,
        backend,
        intuigram_telegram::PeerDirectory::default(),
        application_fixture(),
    ));
    let Err(error) = result else {
        panic!("the fatal backend error should be returned after shutdown")
    };

    assert!(matches!(
        error,
        super::super::Error::TelegramActorMailboxClosed
    ));
    assert!(shutdown.get(), "fatal return bypassed backend shutdown");
}
