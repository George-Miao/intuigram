use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use crate::mailbox::ErasedMailbox;
use crate::{Actor, Mailbox};

#[derive(Default)]
pub(super) struct Registry {
    state: OnceLock<Arc<RegistryState>>,
}

#[derive(Default)]
struct RegistryState {
    actors: Mutex<HashMap<RegistryName, Option<ErasedMailbox>>>,
}

#[derive(Clone)]
enum RegistryName {
    Static(&'static str),
    Owned(Arc<String>),
}

impl RegistryName {
    fn as_str(&self) -> &str {
        match self {
            Self::Static(name) => name,
            Self::Owned(name) => name,
        }
    }
}

impl From<Cow<'static, str>> for RegistryName {
    fn from(name: Cow<'static, str>) -> Self {
        match name {
            Cow::Borrowed(name) => Self::Static(name),
            Cow::Owned(name) => Self::Owned(Arc::new(name)),
        }
    }
}

impl Borrow<str> for RegistryName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for RegistryName {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for RegistryName {}

impl Hash for RegistryName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl Registry {
    pub(super) fn reserve(
        &self,
        name: Cow<'static, str>,
    ) -> Result<Registration, Cow<'static, str>> {
        let state = self
            .state
            .get_or_init(|| Arc::new(RegistryState::default()));
        let mut actors = state
            .actors
            .lock()
            .expect("actor registry lock poisoning is unrecoverable");
        if actors.contains_key(name.as_ref()) {
            return Err(name);
        }
        let name = RegistryName::from(name);
        actors.insert(name.clone(), None);
        Ok(Registration {
            name,
            state: state.clone(),
        })
    }

    pub(super) fn get<A>(&self, name: &str) -> Option<Mailbox<A>>
    where
        A: Actor,
    {
        let mailbox = self
            .state
            .get()?
            .actors
            .lock()
            .expect("actor registry lock poisoning is unrecoverable")
            .get(name)
            .and_then(Clone::clone)?;
        Mailbox::from_erased(mailbox)
    }
}

pub(super) struct Registration {
    name: RegistryName,
    state: Arc<RegistryState>,
}

impl Registration {
    pub(super) fn activate<A>(&self, mailbox: &Mailbox<A>)
    where
        A: Actor,
    {
        let mut actors = self
            .state
            .actors
            .lock()
            .expect("actor registry lock poisoning is unrecoverable");
        let actor = actors
            .get_mut(&self.name)
            .expect("actor registration disappeared before startup");
        *actor = Some(mailbox.erase());
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.state
            .actors
            .lock()
            .expect("actor registry lock poisoning is unrecoverable")
            .remove(&self.name);
    }
}
