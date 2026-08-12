use super::*;

mod text;
mod types;
mod upload;

pub use types::{TextSend, UploadSend};

impl Client {
    pub(crate) async fn invoke_outbound<R>(
        &mut self,
        request: &R,
        policy: InvocationPolicy,
    ) -> Result<R::Return>
    where
        R: tl::RemoteCall + tl::Serializable,
        R::Return: tl::Deserializable,
    {
        self.connection
            .invoke_with_policy(request, policy)
            .await
            .context(InvokeSnafu)
    }

    /// Returns a direct IPv4 endpoint advertised by Telegram for a data center.
    #[must_use]
    pub fn data_center_endpoint(&self, dc_id: i32) -> Option<SocketAddr> {
        self.data_centers.get(&dc_id).copied()
    }
}
