//! Deterministic, single-owner application state for Popgram.

use std::num::NonZeroUsize;

use async_channel::{Receiver, Sender};
use snafu::Snafu;

/// Current Telegram connection state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// The account is synchronized over a live connection.
    Connected,
    /// A connection attempt is in progress.
    Connecting,
    /// Automatic reconnection is waiting for its backoff deadline.
    ReconnectCooldown,
}

/// User actions understood by the state owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intent {
    /// Retry immediately during a reconnect cooldown.
    Reconnect,
}

/// Results reported by external adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterEvent {
    /// Telegram connectivity changed.
    ConnectionChanged(ConnectionState),
}

/// Ordered inputs to the state owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Input {
    /// An action from the active user interface.
    Intent(Intent),
    /// A result from an external adapter.
    Adapter(AdapterEvent),
}

/// Side effects requested from adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Start a connection attempt immediately.
    Reconnect,
}

/// Context-sensitive actions shown by every user interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Retry immediately during a reconnect cooldown.
    Reconnect,
}

/// Immutable data rendered by a user interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    /// Current Telegram connectivity.
    pub connection: ConnectionState,
    /// Actions currently valid in this context.
    pub actions: Vec<Action>,
}

/// One state transition observed by adapters and user interfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    /// Immutable view after applying the input.
    pub view: View,
    /// Optional external work requested by the transition.
    pub effect: Option<Effect>,
}

/// Failure while running the state owner.
#[derive(Debug, Snafu)]
pub enum Error {
    /// All view consumers disconnected while the application was running.
    #[snafu(display("application output channel closed"))]
    OutputClosed,
}

/// Result returned by the application state owner.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// UI and adapter endpoints for a bounded application channel pair.
pub struct AppHandle {
    /// Ordered input producer.
    pub inputs: Sender<Input>,
    /// Immutable updates from the state owner.
    pub updates: Receiver<Update>,
}

/// State-owner endpoints that can only be created as a bounded pair.
pub struct AppChannels {
    inputs: Receiver<Input>,
    updates: Sender<Update>,
}

/// Creates the typed bounded channels used by one application state owner.
#[must_use]
pub fn bounded_channels(capacity: NonZeroUsize) -> (AppHandle, AppChannels) {
    let (input_tx, input_rx) = async_channel::bounded(capacity.get());
    let (update_tx, update_rx) = async_channel::bounded(capacity.get());
    (
        AppHandle {
            inputs: input_tx,
            updates: update_rx,
        },
        AppChannels {
            inputs: input_rx,
            updates: update_tx,
        },
    )
}

/// Sole owner of mutable application state.
pub struct App {
    connection: ConnectionState,
}

impl App {
    /// Creates an application in the connection-attempt state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            connection: ConnectionState::Connecting,
        }
    }

    /// Processes ordered input until every producer disconnects.
    pub async fn run(mut self, channels: AppChannels) -> Result<()> {
        self.publish(&channels.updates, None).await?;
        while let Ok(input) = channels.inputs.recv().await {
            let effect = match input {
                Input::Adapter(AdapterEvent::ConnectionChanged(connection)) => {
                    self.connection = connection;
                    None
                }
                Input::Intent(Intent::Reconnect)
                    if self.connection == ConnectionState::ReconnectCooldown =>
                {
                    self.connection = ConnectionState::Connecting;
                    Some(Effect::Reconnect)
                }
                Input::Intent(Intent::Reconnect) => None,
            };
            self.publish(&channels.updates, effect).await?;
        }
        Ok(())
    }

    async fn publish(&self, updates: &Sender<Update>, effect: Option<Effect>) -> Result<()> {
        let actions = if self.connection == ConnectionState::ReconnectCooldown {
            vec![Action::Reconnect]
        } else {
            Vec::new()
        };
        updates
            .send(Update {
                view: View {
                    connection: self.connection,
                    actions,
                },
                effect,
            })
            .await
            .map_err(|_| Error::OutputClosed)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use futures_lite::future;

    use super::{
        Action, AdapterEvent, App, ConnectionState, Effect, Input, Intent, bounded_channels,
    };

    #[test]
    fn reconnect_is_available_only_during_cooldown() {
        future::block_on(async {
            let capacity = NonZeroUsize::new(4).expect("fixture capacity should be positive");
            let (handle, channels) = bounded_channels(capacity);

            let drive = App::new().run(channels);
            let observe = async move {
                let initial = handle
                    .updates
                    .recv()
                    .await
                    .expect("initial view should arrive");
                assert!(initial.view.actions.is_empty());

                handle
                    .inputs
                    .send(Input::Adapter(AdapterEvent::ConnectionChanged(
                        ConnectionState::ReconnectCooldown,
                    )))
                    .await
                    .expect("cooldown event should be accepted");
                let cooldown = handle
                    .updates
                    .recv()
                    .await
                    .expect("cooldown view should arrive");
                assert_eq!(cooldown.view.actions, vec![Action::Reconnect]);

                handle
                    .inputs
                    .send(Input::Intent(Intent::Reconnect))
                    .await
                    .expect("reconnect intent should be accepted");
                let reconnecting = handle
                    .updates
                    .recv()
                    .await
                    .expect("reconnect view should arrive");
                assert_eq!(reconnecting.view.connection, ConnectionState::Connecting);
                assert!(reconnecting.view.actions.is_empty());
                assert_eq!(reconnecting.effect, Some(Effect::Reconnect));
                drop(handle.inputs);
            };

            let (result, ()) = future::zip(drive, observe).await;
            result.expect("application should shut down cleanly");
        });
    }
}
