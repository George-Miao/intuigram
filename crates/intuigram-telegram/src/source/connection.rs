/// Controls whether an invocation waits through Telegram flood control or
/// returns it to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationPolicy {
    /// Sleep for Telegram's requested delay, then invoke the request again.
    WaitForFlood,

    /// Return Telegram's flood-wait error without delaying or reinvoking.
    SurfaceFloodWait,
}

pub(super) enum Connection {
    Login(Box<EncryptedConnection>),
    Live(InvocationHandle),
}

impl Connection {
    pub(super) async fn invoke<R>(
        &mut self,
        request: &R,
    ) -> std::result::Result<R::Return, InvocationError>
    where
        R: tl::RemoteCall + tl::Serializable,
        R::Return: tl::Deserializable,
    {
        self.invoke_with_policy(request, InvocationPolicy::WaitForFlood)
            .await
    }

    pub(super) async fn invoke_with_policy<R>(
        &mut self,
        request: &R,
        policy: InvocationPolicy,
    ) -> std::result::Result<R::Return, InvocationError>
    where
        R: tl::RemoteCall + tl::Serializable,
        R::Return: tl::Deserializable,
    {
        loop {
            let result = match self {
                Self::Login(connection) => connection.invoke(request).await,
                Self::Live(connection) => connection.invoke(request).await,
            };
            match result {
                Err(error) => {
                    let Some(delay) = flood_wait_delay(policy, &error) else {
                        return Err(error);
                    };
                    compio::time::sleep(delay).await;
                }
                Ok(value) => return Ok(value),
            }
        }
    }

    pub(super) fn take_updates(&mut self) -> Vec<Vec<u8>> {
        match self {
            Self::Login(connection) => connection.take_updates(),
            Self::Live(_) => Vec::new(),
        }
    }
}

pub(crate) fn flood_wait_delay(
    policy: InvocationPolicy,
    error: &InvocationError,
) -> Option<Duration> {
    match policy {
        InvocationPolicy::WaitForFlood => error.retry_after(),
        InvocationPolicy::SurfaceFloodWait => None,
    }
}

/// Telegram API client built on Intuigram's Compio `MTProto` sender.
pub struct Client {
    pub(super) connection: Connection,
    pub(super) dc_id: i32,
    pub(super) route: Route,
    pub(super) credentials: ApplicationCredentials,
    pub(super) password: Option<tl::types::account::Password>,
    pub(super) identity: Option<AuthorizedUser>,
    pub(super) peers: PeerDirectory,
    pub(super) names: HashMap<ChatId, String>,
    pub(super) channel_pts: HashMap<ChatId, i32>,
    pub(super) data_centers: HashMap<i32, SocketAddr>,
    pub(super) venue_search_username: Option<String>,
    pub(super) venue_search_bot: Option<tl::enums::InputUser>,
}

/// Passive normalized Telegram updates driven by one persistent MTProto
/// connection.
pub struct LiveUpdates {
    pub(super) driver: Pin<Box<ConnectionDriver>>,
    pub(super) updates: UpdateStream,
    pub(super) names: HashMap<ChatId, String>,
    pub(super) terminated: bool,
}

impl LiveUpdates {
    /// Returns the number of raw updates already buffered by MTProto.
    pub fn buffered_len(&self) -> usize {
        self.updates.buffered_len()
    }
}

impl Stream for LiveUpdates {
    type Item = Result<LiveEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }
        match self.driver.as_mut().poll(cx) {
            Poll::Ready(Err(source)) => {
                self.terminated = true;
                return Poll::Ready(Some(Err(Error::Invoke { source })));
            }
            Poll::Ready(Ok(())) => {
                self.terminated = true;
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }
        match Pin::new(&mut self.updates).poll_next(cx) {
            Poll::Ready(Some(bytes)) => match normalize_live_update(&bytes, &mut self.names) {
                Ok(batch) => Poll::Ready(Some(Ok(LiveEvent {
                    events: batch.events,
                    cursors: batch.cursors,
                    peers: batch.peers,
                }))),
                Err(error) => Poll::Ready(Some(Err(error))),
            },
            Poll::Ready(None) => {
                self.terminated = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
use super::*;
