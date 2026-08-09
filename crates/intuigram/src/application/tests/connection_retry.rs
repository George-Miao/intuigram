use super::*;

#[derive(Clone)]
struct FailingConnectionBackend {
    observed: Rc<RefCell<Vec<AdapterEffect>>>,
}

impl ApplicationBackend for FailingConnectionBackend {
    async fn execute(
        &self,
        effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        let durable = super::super::outbox::admission::handles(&effect.effect);
        self.observed.borrow_mut().push(effect);
        if durable {
            return Ok(BackendOutput::event(None));
        }
        Err(Error::TelegramUpdatesClosed)
    }
}

#[test]
fn connection_failure_does_not_requeue_a_durably_admitted_send() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = AlwaysPendingEvents;
    let mut adapter_events = NoAdapterEvents;
    let backend = FailingConnectionBackend {
        observed: Rc::clone(&observed),
    };
    let mut app = intuigram_app::App::new();
    let update = app.transition(intuigram_app::Input::Adapter(AdapterEvent::Bootstrap(
        application_fixture(),
    )));
    let send = AdapterEffect::new(Effect::SendMessage {
        chat: ChatId(10),
        text: "retry once connected".to_owned(),
        entities: Vec::new(),
        link_preview: true,
        reply_to: None,
        thread_root: None,
        saved_peer: None,
        attachments: Vec::new(),
        local_id: MessageId(-1),
    })
    .expect("operation id should be generated");
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    let exit = runtime
        .block_on(run_application_state(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            backend,
            ApplicationState {
                app,
                update,
                pending_effects: VecDeque::from([send]),
                peers: intuigram_telegram::PeerDirectory::default(),
            },
        ))
        .expect("connection failure should become an application handoff");
    let ApplicationExit::Disconnected(state) = exit else {
        panic!("connection failure should not quit")
    };

    assert_eq!(observed.borrow().len(), 2);
    assert_eq!(state.pending_effects.len(), 1);
    assert!(!super::super::outbox::admission::handles(
        &state.pending_effects[0].effect
    ));
    assert_eq!(
        state.app.view().connection,
        intuigram_app::ConnectionState::ReconnectCooldown
    );
    assert!(
        views
            .borrow()
            .iter()
            .any(|view| { view.connection == intuigram_app::ConnectionState::ReconnectCooldown }),
        "the TUI should render the disconnect before reconnecting"
    );
}
