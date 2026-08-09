pub(super) trait ApplicationUi {
    fn draw(&mut self, view: &intuigram_app::View) -> intuigram_tui::Result<()>;

    fn resolve_event(
        &self,
        view: &intuigram_app::View,
        event: crossterm::event::Event,
    ) -> Option<UiEvent>;

    fn poll_redraw(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<intuigram_tui::Result<()>>;
}

impl ApplicationUi for TerminalUi {
    fn draw(&mut self, view: &intuigram_app::View) -> intuigram_tui::Result<()> {
        Self::draw(self, view)
    }

    fn resolve_event(
        &self,
        view: &intuigram_app::View,
        event: crossterm::event::Event,
    ) -> Option<UiEvent> {
        Self::resolve_event(self, view, event)
    }

    fn poll_redraw(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<intuigram_tui::Result<()>> {
        Self::poll_redraw(self, cx)
    }
}

pub(crate) fn main() {
    if let Err(error) = run() {
        print_error_chain(&error);
        std::process::exit(1);
    }
}

pub(super) fn print_error_chain(error: &(dyn std::error::Error + 'static)) {
    for (depth, line) in error_lines(error).into_iter().enumerate() {
        if depth == 0 {
            eprintln!("intuigram: {line}");
        } else {
            eprintln!("  caused by: {line}");
        }
    }
}

pub(super) fn error_lines(error: &(dyn std::error::Error + 'static)) -> Vec<String> {
    std::iter::successors(Some(error), |error| error.source())
        .map(|error| {
            error
                .to_string()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

pub(super) fn run() -> Result<()> {
    let arguments = parse_arguments(env::args().skip(1))?;
    if arguments.help {
        print_help();
        return Ok(());
    }
    let runtime = compio::runtime::Runtime::new().context(RuntimeSnafu)?;
    runtime.block_on(run_async(arguments))
}
use super::*;
