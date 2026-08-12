use super::*;

impl App {
    pub(in crate::app) fn open_poll_vote(&mut self) {
        let Some(message) = self
            .view
            .active_message
            .and_then(|index| self.view.messages.get(index))
        else {
            return;
        };
        let Some(poll) = message
            .details
            .media
            .as_ref()
            .and_then(|media| media.poll.as_ref())
            .filter(|poll| !poll.closed && !poll.options.is_empty())
        else {
            return;
        };
        self.view.poll_vote = Some(PollVoteView {
            message: message.id,
            selected: 0,
            choices: poll
                .options
                .iter()
                .enumerate()
                .filter_map(|(index, option)| option.chosen.then_some(index))
                .collect(),
            multiple_choice: poll.multiple_choice,
            options: poll
                .options
                .iter()
                .map(|option| option.text.clone())
                .collect(),
        });
    }

    pub(in crate::app) fn apply_poll_vote(&mut self, action: Action) -> Option<Effect> {
        match action {
            Action::MoveUp | Action::MoveDown => {
                let picker = self.view.poll_vote.as_mut()?;
                picker.selected = move_index(
                    Some(picker.selected),
                    picker.options.len(),
                    action == Action::MoveDown,
                )?;
                None
            }
            Action::TogglePollChoice => {
                let picker = self.view.poll_vote.as_mut()?;
                if picker.multiple_choice {
                    if let Some(position) = picker
                        .choices
                        .iter()
                        .position(|choice| *choice == picker.selected)
                    {
                        picker.choices.remove(position);
                    } else {
                        picker.choices.push(picker.selected);
                    }
                } else {
                    picker.choices = vec![picker.selected];
                }
                None
            }
            Action::ConfirmPollVote => self.confirm_poll_vote(),
            Action::Cancel | Action::VotePoll => {
                self.view.poll_vote = None;
                None
            }
            _ => None,
        }
    }

    fn confirm_poll_vote(&mut self) -> Option<Effect> {
        let chat = self.active_chat_id()?;
        let mut picker = self.view.poll_vote.take()?;
        if picker.choices.is_empty() {
            picker.choices.push(picker.selected);
        }
        picker.choices.sort_unstable();
        picker.choices.dedup();
        let mut message = self
            .view
            .messages
            .iter()
            .find(|message| message.id == picker.message)?
            .clone();
        let poll = message.details.media.as_mut()?.poll.as_mut()?;
        for (index, option) in poll.options.iter_mut().enumerate() {
            option.chosen = picker.choices.contains(&index);
        }
        Some(Effect::VotePoll {
            chat,
            message: Box::new(message),
            options: picker.choices,
        })
    }
}
