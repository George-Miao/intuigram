use super::*;

impl Client {
    /// Sends a rich text Message, optionally as a reply.
    pub async fn send_text(&mut self, request: TextSend) -> Result<MessageId> {
        self.send_text_with_policy(request, InvocationPolicy::WaitForFlood)
            .await
    }

    /// Sends a rich text Message using the requested invocation policy.
    pub async fn send_text_with_policy(
        &mut self,
        request: TextSend,
        policy: InvocationPolicy,
    ) -> Result<MessageId> {
        let TextSend {
            chat,
            text,
            entities,
            link_preview,
            reply_to,
            thread_root,
            monoforum_peer,
            random_id,
            schedule_date,
        } = request;
        let peer = self.peers.resolve(chat)?;
        let monoforum_peer = monoforum_peer
            .map(|peer| self.peers.resolve(peer))
            .transpose()?;
        let reply_to = input_reply_to(reply_to, thread_root, monoforum_peer)?;
        let entities = serialize_entities(entities)?;
        let updates = self
            .invoke_outbound(
                &tl::functions::messages::SendMessage {
                    no_webpage: !link_preview,
                    silent: false,
                    background: false,
                    clear_draft: true,
                    noforwards: false,
                    update_stickersets_order: false,
                    invert_media: false,
                    allow_paid_floodskip: false,
                    peer,
                    reply_to,
                    message: text,
                    random_id,
                    reply_markup: None,
                    entities,
                    schedule_date,
                    schedule_repeat_period: None,
                    send_as: None,
                    quick_reply_shortcut: None,
                    effect: None,
                    allow_paid_stars: None,
                    suggested_post: None,
                    rich_message: None,
                },
                policy,
            )
            .await?;
        sent_message_id(updates, random_id)
    }
}
