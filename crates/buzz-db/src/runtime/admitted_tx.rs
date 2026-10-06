//! Community-admitted event-write transactions.

use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};

use buzz_core::CommunityId;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use crate::Result;

/// An event-write transaction that has passed community admission.
///
/// Only the admission chokepoints in this crate can construct one, and only
/// after taking the shared community admission lock (or validating a serving
/// write lease) on the transaction. Event-write helpers take `&mut AdmittedTx`
/// and read the community from it, so the compiler rejects a raw
/// [`sqlx::Transaction`] or a transaction admitted for a different community.
///
/// The value dereferences to the underlying [`PgConnection`] for statements,
/// but never exposes the inner transaction: [`AdmittedTx::commit`] and
/// [`AdmittedTx::rollback`] are the only ways to end it. Dropping it without
/// committing rolls back, as with [`sqlx::Transaction`].
///
/// These are live regressions for the guarantee. Event-write helpers accept an
/// admitted transaction:
///
/// ```no_run
/// async fn write(tx: &mut buzz_db::AdmittedTx, event: &nostr::Event) {
///     let _ = buzz_db::event::insert_event_in_transaction(tx, event, None).await;
/// }
/// ```
///
/// but not a raw transaction, which carries no proof of admission:
///
/// ```compile_fail
/// async fn write(tx: &mut sqlx::Transaction<'static, sqlx::Postgres>, event: &nostr::Event) {
///     let _ = buzz_db::event::insert_event_in_transaction(tx, event, None).await;
/// }
/// ```
///
/// Code outside the crate cannot construct one, either through the
/// constructor:
///
/// ```compile_fail
/// fn forge(
///     tx: sqlx::Transaction<'static, sqlx::Postgres>,
///     community: buzz_core::CommunityId,
/// ) -> buzz_db::AdmittedTx {
///     buzz_db::AdmittedTx::admitted(tx, community)
/// }
/// ```
///
/// or through a conversion:
///
/// ```compile_fail
/// fn forge(tx: sqlx::Transaction<'static, sqlx::Postgres>) -> buzz_db::AdmittedTx {
///     tx.into()
/// }
/// ```
#[must_use = "dropping an AdmittedTx rolls it back; call commit()"]
pub struct AdmittedTx {
    tx: Transaction<'static, Postgres>,
    community: CommunityId,
    /// Channels whose TTL deadline [`AdmittedTx::commit`] refreshes.
    ttl_refresh_channels: BTreeSet<Uuid>,
}

impl AdmittedTx {
    /// Wrap a transaction that has already been admitted for `community`.
    ///
    /// Callers must have taken the community admission lock (or validated a
    /// serving lease for `community`) on `tx` before calling this. The source
    /// policy in `tests/observability_source.rs` pins every call site.
    pub(super) fn admitted(tx: Transaction<'static, Postgres>, community: CommunityId) -> Self {
        Self {
            tx,
            community,
            ttl_refresh_channels: BTreeSet::new(),
        }
    }

    /// The community this transaction was admitted for.
    pub fn community(&self) -> CommunityId {
        self.community
    }

    /// Record that an event committed with this transaction belongs to
    /// `channel_id`, so [`AdmittedTx::commit`] refreshes the channel's TTL.
    pub(crate) fn record_channel_event(&mut self, channel_id: Option<Uuid>, kind: i32) {
        if let Some(channel) =
            crate::store::event_follow_up::refreshes_channel_ttl(channel_id, kind)
        {
            self.ttl_refresh_channels.insert(channel);
        }
    }

    /// Commit the transaction. This is the only commit path for admitted
    /// event writes.
    ///
    /// Refreshes the TTL of every channel that received an event first, as
    /// the last statement before COMMIT.
    pub async fn commit(mut self) -> Result<()> {
        if !self.ttl_refresh_channels.is_empty() {
            crate::store::event_follow_up::refresh_channel_ttls(
                &mut self.tx,
                self.community,
                &self.ttl_refresh_channels,
            )
            .await?;
        }
        self.tx.commit().await?;
        Ok(())
    }

    /// Roll the transaction back explicitly.
    pub async fn rollback(self) -> Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }
}

impl Deref for AdmittedTx {
    type Target = PgConnection;

    fn deref(&self) -> &PgConnection {
        &self.tx
    }
}

impl DerefMut for AdmittedTx {
    fn deref_mut(&mut self) -> &mut PgConnection {
        &mut self.tx
    }
}

impl AsMut<PgConnection> for AdmittedTx {
    fn as_mut(&mut self) -> &mut PgConnection {
        &mut self.tx
    }
}

impl std::fmt::Debug for AdmittedTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdmittedTx")
            .field("community", &self.community)
            .finish_non_exhaustive()
    }
}
