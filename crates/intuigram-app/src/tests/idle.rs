use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use super::*;

#[test]
fn ignored_terminal_input_does_not_redraw_an_unchanged_view() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = ScriptedEvents {
        steps: [EventStep::Ready(key('!')), EventStep::Ready(key('q'))].into(),
    };
    let mut adapter_events = NoAdapterEvents;
    let (app, mut update) = application_state(application_fixture());
    update.effect = None;
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application_state(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            PendingHistoryBackend {
                polls: Rc::new(Cell::new(0)),
            },
            state(app, update),
        ))
        .expect("application should stop cleanly");

    assert_eq!(
        views.borrow().len(),
        1,
        "an ignored key and unchanged Quit view should not trigger full redraws"
    );
}

#[test]
fn idle_runtime_parks_until_a_registered_source_wakes() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let polls = Rc::new(Cell::new(0));
    let mut terminal = RecordingUi {
        views: Rc::clone(&views),
    };
    let mut events = TimedQuit {
        timer: None,
        polls: Rc::clone(&polls),
    };
    let mut adapter_events = NoAdapterEvents;
    let mut fixture = application_fixture();
    fixture.chats.clear();
    fixture.messages.clear();
    fixture.histories.clear();
    fixture.avatar_peers.clear();
    fixture.restored_selection = None;
    let (app, update) = application_state(fixture);
    assert!(!update.view.has_pending_effort());
    let background_polls = Rc::new(Cell::new(0));
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application_state(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            IdleBackend {
                polls: Rc::clone(&background_polls),
            },
            state(app, update),
        ))
        .expect("idle application should stop cleanly");

    assert!(
        polls.get() <= 2,
        "idle terminal source was polled repeatedly"
    );
    assert!(
        background_polls.get() <= 2,
        "idle background source was polled repeatedly"
    );
    assert_eq!(views.borrow().len(), 1, "idle state should draw only once");
}

#[test]
#[ignore = "same-machine CPU benchmark"]
fn idle_runtime_cpu_sample() {
    let draws = run_cpu_sample(BenchmarkEvents::idle());

    assert_eq!(
        draws, 1,
        "idle benchmark should draw only its initial frame"
    );
}

#[test]
#[ignore = "same-machine CPU benchmark"]
fn rapid_navigation_cpu_sample() {
    let draws = run_cpu_sample(BenchmarkEvents::navigation(600));

    assert!(
        draws >= 600,
        "navigation benchmark should render each changed selection"
    );
}

fn run_cpu_sample(mut events: BenchmarkEvents) -> usize {
    let draws = Rc::new(Cell::new(0));
    let mut terminal = BenchmarkUi {
        renderer: intuigram_tui::TestRenderer::default(),
        draws: Rc::clone(&draws),
    };
    let mut adapter_events = NoAdapterEvents;
    let (app, mut update) = application_state(application_fixture());
    update.effect = None;
    assert!(!update.view.has_pending_effort());
    let runtime = compio::runtime::Runtime::new().expect("test runtime should initialize");

    runtime
        .block_on(run_application_state(
            &mut terminal,
            &mut events,
            &mut adapter_events,
            IdleBackend {
                polls: Rc::new(Cell::new(0)),
            },
            state(app, update),
        ))
        .expect("CPU sample should stop cleanly");

    draws.get()
}

struct BenchmarkUi {
    renderer: intuigram_tui::TestRenderer,
    draws: Rc<Cell<usize>>,
}

impl ApplicationUi for BenchmarkUi {
    fn draw(&mut self, view: &intuigram_lib::View) -> intuigram_tui::Result<()> {
        std::hint::black_box(self.renderer.render(view, 120, 40));
        self.draws.set(self.draws.get().saturating_add(1));
        Ok(())
    }

    fn resolve_event(&self, _view: &intuigram_lib::View, event: Event) -> Option<UiEvent> {
        let Event::Key(key) = event else {
            return Some(UiEvent::Redraw);
        };
        match key.code {
            KeyCode::Up => Some(UiEvent::Intent(Intent::Action(Action::MoveUp))),
            KeyCode::Down => Some(UiEvent::Intent(Intent::Action(Action::MoveDown))),
            KeyCode::Char('q') => Some(UiEvent::Intent(Intent::Action(Action::Quit))),
            _ => None,
        }
    }

    fn poll_redraw(&mut self, _cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<()>> {
        Poll::Pending
    }
}

struct BenchmarkEvents {
    timer: Option<Pin<Box<dyn Future<Output = ()>>>>,
    remaining: Option<usize>,
    move_down: bool,
}

impl BenchmarkEvents {
    const fn idle() -> Self {
        Self {
            timer: None,
            remaining: None,
            move_down: true,
        }
    }

    const fn navigation(remaining: usize) -> Self {
        Self {
            timer: None,
            remaining: Some(remaining),
            move_down: true,
        }
    }
}

impl ApplicationEvents for BenchmarkEvents {
    fn poll_next_event(&mut self, cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        let delay = if self.remaining.is_some() {
            Duration::from_millis(16)
        } else {
            Duration::from_secs(10)
        };
        if self
            .timer
            .get_or_insert_with(|| Box::pin(compio::time::sleep(delay)))
            .as_mut()
            .poll(cx)
            .is_pending()
        {
            return Poll::Pending;
        }
        self.timer = None;
        let Some(remaining) = self.remaining.as_mut() else {
            return Poll::Ready(Ok(key('q')));
        };
        if *remaining == 0 {
            return Poll::Ready(Ok(key('q')));
        }
        *remaining = remaining.saturating_sub(1);
        let code = if self.move_down {
            KeyCode::Down
        } else {
            KeyCode::Up
        };
        self.move_down = !self.move_down;
        Poll::Ready(Ok(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))))
    }
}

fn state(app: intuigram_lib::App, update: intuigram_lib::Update) -> ApplicationState {
    ApplicationState {
        app,
        update,
        pending_effects: VecDeque::new(),
        peers: intuigram_telegram::PeerDirectory::default(),
        media_limits: intuigram_telegram::MediaLimits::default(),
    }
}

struct TimedQuit {
    timer: Option<Pin<Box<dyn Future<Output = ()>>>>,
    polls: Rc<Cell<usize>>,
}

impl ApplicationEvents for TimedQuit {
    fn poll_next_event(&mut self, cx: &mut Context<'_>) -> Poll<intuigram_tui::Result<Event>> {
        self.polls.set(self.polls.get().saturating_add(1));
        self.timer
            .get_or_insert_with(|| Box::pin(compio::time::sleep(Duration::from_millis(25))))
            .as_mut()
            .poll(cx)
            .map(|()| Ok(key('q')))
    }
}

#[derive(Clone)]
struct IdleBackend {
    polls: Rc<Cell<usize>>,
}

impl ApplicationBackend for IdleBackend {
    async fn execute(
        &self,
        _effect: AdapterEffect,
        _peers: intuigram_telegram::PeerDirectory,
    ) -> Result<BackendOutput> {
        Ok(BackendOutput::event(None))
    }

    fn poll_background(&self, _cx: &mut Context<'_>) -> Poll<Result<BackendOutput>> {
        self.polls.set(self.polls.get().saturating_add(1));
        Poll::Pending
    }
}
