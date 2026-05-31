//! The post-and-distribute engine: turns one posted message into one durable copy
//! per resolved recipient. The only place an [`Audience`] is expanded.

use super::audience::{Audience, Endpoint};
use super::envelope::Envelope;
use super::error::ApiCommError;
use super::ids::UserId;
use super::message::Message;
use super::policy::Route;
use super::store::{MembershipResolver, RelayStore};

/// The result of distributing one message.
#[derive(Debug, Clone)]
pub struct Distribution {
    pub envelope: Envelope,
    pub recipients: Vec<Endpoint>,
}

/// Fans a posted message out to its audience using Core-supplied store and
/// membership resolver.
pub struct Distributor<'a> {
    store: &'a dyn RelayStore,
    resolver: &'a dyn MembershipResolver,
}

impl<'a> Distributor<'a> {
    pub fn new(store: &'a dyn RelayStore, resolver: &'a dyn MembershipResolver) -> Self {
        Self { store, resolver }
    }

    /// Post a message and distribute it to an audience. Rejects any message whose
    /// route is not `CorePost`, as a runtime backstop to the compile-time policy.
    pub async fn post<M: Message>(
        &self,
        message: &M,
        origin: Option<UserId>,
        audience: Audience,
    ) -> Result<Distribution, ApiCommError> {
        if M::ROUTE != Route::CorePost {
            return Err(ApiCommError::WrongRoute {
                kind: M::KIND,
                route: M::ROUTE,
            });
        }

        let envelope = Envelope::seal(message, origin)?;
        let recipients = self.resolve(&audience).await?;

        for recipient in &recipients {
            self.store.enqueue(recipient, &envelope).await?;
        }

        Ok(Distribution {
            envelope,
            recipients,
        })
    }

    async fn resolve(&self, audience: &Audience) -> Result<Vec<Endpoint>, ApiCommError> {
        match audience {
            Audience::Direct { endpoint } => Ok(vec![endpoint.clone()]),
            Audience::InstanceMembers { instance, except } => {
                self.resolver
                    .instance_members(instance, except.as_ref())
                    .await
            }
            Audience::Group { group, except } => {
                self.resolver.group_members(group, except.as_ref()).await
            }
        }
    }
}
