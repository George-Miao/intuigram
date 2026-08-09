use super::*;

pub(super) fn normalize_poll(media: &tl::types::MessageMediaPoll) -> MediaCard {
    let tl::enums::Poll::Poll(poll) = &media.poll;
    let tl::enums::PollResults::Results(results) = &media.results;
    let results_by_option = results.results.as_deref().unwrap_or_default();
    let options = poll
        .answers
        .iter()
        .map(|answer| normalize_option(answer, results_by_option))
        .collect();
    MediaCard {
        kind: MediaKind::Poll,
        title: if poll.quiz { "Quiz" } else { "Poll" }.to_owned(),
        description: text_with_entities(poll.question.clone()),
        details: Vec::new(),
        poll: Some(PollView {
            quiz: poll.quiz,
            multiple_choice: poll.multiple_choice,
            closed: poll.closed,
            total_voters: nonnegative_u32(results.total_voters),
            options,
            solution: results.solution.clone(),
        }),
        specialized: None,
        remote_id: Some(poll.id.to_string()),
    }
}

fn normalize_option(
    answer: &tl::enums::PollAnswer,
    results: &[tl::enums::PollAnswerVoters],
) -> PollOptionView {
    let (text, option) = match answer {
        tl::enums::PollAnswer::Answer(answer) => (
            text_with_entities(answer.text.clone()),
            Some(&answer.option),
        ),
        tl::enums::PollAnswer::InputPollAnswer(answer) => {
            (text_with_entities(answer.text.clone()), None)
        }
    };
    let result = option.and_then(|option| {
        results.iter().find_map(|result| {
            let tl::enums::PollAnswerVoters::Voters(result) = result;
            (result.option == *option).then_some(result)
        })
    });
    PollOptionView {
        text,
        voters: result.and_then(|result| nonnegative_u32(result.voters)),
        chosen: result.is_some_and(|result| result.chosen),
        correct: result.is_some_and(|result| result.correct),
    }
}
