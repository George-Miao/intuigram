use super::effect_fairness::{BurstAdapterEvents, QuitAfterNotifications};
use super::*;

#[derive(Clone)]
struct BackgroundBackend {
    emitted: Rc<Cell<usize>>,
}

impl ApplicationBackend for BackgroundBackend {
    async fn execute(
        &self,
        _effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        Ok(BackendOutput::event(None))
    }

    fn poll_background(&self, _cx: &mut Context<'_>) -> Poll<Result<BackendOutput>> {
        if self.emitted.replace(1) == 1 {
            Poll::Pending
        } else {
            Poll::Ready(Ok(BackendOutput::event(Some(
                AdapterEvent::OperationCompleted("outbox replayed".to_owned()),
            ))))
        }
    }
}

#[test]
fn background_backend_progress_is_reduced_without_waiting_for_user_effects() {
    let emitted = Rc::new(Cell::new(0));
    let views = Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = QuitAfterNotifications {
        notifications: Rc::clone(&emitted),
        expected: 1,
    };
    let mut adapter_events = BurstAdapterEvents {
        next_message: 0,
        total: 0,
        emitted: Rc::new(Cell::new(0)),
    };
    let backend = BackgroundBackend { emitted };
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            intuigram_telegram::PeerDirectory::default(),
            application_fixture(),
        ))
        .expect("application should stop cleanly");

    assert!(
        views
            .borrow()
            .iter()
            .any(|view| { view.notice.as_deref() == Some("outbox replayed") })
    );
}
