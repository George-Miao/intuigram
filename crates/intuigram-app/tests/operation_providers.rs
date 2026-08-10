use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use intuigram_app::{Clock, OperationIdSource, OperationProviders, ProviderResult};

#[derive(Clone)]
struct ManualClock(Rc<Cell<i64>>);

impl Clock for ManualClock {
    fn unix_seconds(&self) -> ProviderResult<i64> {
        Ok(self.0.get())
    }
}

struct ScriptedIds(VecDeque<i64>);

impl OperationIdSource for ScriptedIds {
    fn next_id(&mut self) -> ProviderResult<i64> {
        Ok(self
            .0
            .pop_front()
            .expect("the operation-ID script should cover this test"))
    }
}

#[test]
fn replay_reuses_the_persisted_random_id_without_consuming_entropy() {
    let clock = ManualClock(Rc::new(Cell::new(1_786_291_200)));
    let ids = ScriptedIds(VecDeque::from([41, 99]));
    let mut providers = OperationProviders::new(clock, ids);

    let admitted = providers
        .admit()
        .expect("admission should be deterministic");
    let replayed = providers
        .replay(admitted.random_id())
        .expect("replay should keep the persisted identity");
    let next = providers
        .admit()
        .expect("replay must not consume another identity");

    assert_eq!(admitted.random_id(), 41);
    assert_eq!(replayed.random_id(), 41);
    assert_eq!(next.random_id(), 99);
}

#[test]
fn manual_clock_controls_operation_time_without_sleeping() {
    let now = Rc::new(Cell::new(1_786_291_200));
    let clock = ManualClock(Rc::clone(&now));
    let ids = ScriptedIds(VecDeque::from([41]));
    let mut providers = OperationProviders::new(clock, ids);

    let admitted = providers.admit().expect("admission should observe time");
    now.set(1_786_291_230);
    assert_eq!(
        providers.now().expect("manual time should be readable"),
        1_786_291_230
    );
    let replayed = providers
        .replay(admitted.random_id())
        .expect("replay should observe advanced time");

    assert_eq!(admitted.observed_at(), 1_786_291_200);
    assert_eq!(replayed.observed_at(), 1_786_291_230);
}

#[test]
fn production_providers_issue_distinct_nonzero_ids() {
    let mut providers = OperationProviders::production();

    let first = providers.admit().expect("system providers should work");
    let second = providers
        .admit()
        .expect("system providers should remain usable");

    assert_ne!(first.random_id(), 0);
    assert_ne!(second.random_id(), 0);
    assert_ne!(first.random_id(), second.random_id());
}
