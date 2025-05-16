use crate::{interest::InterestKind, Pnt};
use ajj::{serde_json, HandlerCtx};
use alloy::{primitives::U64, rpc::types::Log};
use dashmap::DashMap;
use reth::{
    providers::{providers::BlockchainProvider, CanonStateNotifications, CanonStateSubscriptions},
    rpc::types::Header,
};
use std::{
    cmp::min,
    collections::VecDeque,
    future::pending,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
    time::Duration,
};
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};
use tracing::{debug, debug_span, enabled, trace, Instrument};

/// Either type for subscription outputs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum Either {
    Log(Box<Log>),
    Block(Box<Header>),
}

/// Buffer for subscription outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionBuffer {
    Log(VecDeque<Log>),
    Block(VecDeque<Header>),
}

impl SubscriptionBuffer {
    /// True if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the number of items in the buffer.
    pub fn len(&self) -> usize {
        match self {
            Self::Log(buf) => buf.len(),
            Self::Block(buf) => buf.len(),
        }
    }

    /// Extend this buffer with another buffer.
    ///
    /// # Panics
    ///
    /// Panics if the buffers are of different types.
    pub fn extend(&mut self, other: Self) {
        match (self, other) {
            (Self::Log(buf), Self::Log(other)) => buf.extend(other),
            (Self::Block(buf), Self::Block(other)) => buf.extend(other),
            _ => panic!("mismatched buffer types"),
        }
    }

    /// Pop the front of the buffer.
    pub fn pop_front(&mut self) -> Option<Either> {
        match self {
            Self::Log(buf) => buf.pop_front().map(|log| Either::Log(Box::new(log))),
            Self::Block(buf) => buf.pop_front().map(|header| Either::Block(Box::new(header))),
        }
    }
}

impl From<Vec<Log>> for SubscriptionBuffer {
    fn from(logs: Vec<Log>) -> Self {
        Self::Log(logs.into())
    }
}

impl FromIterator<Log> for SubscriptionBuffer {
    fn from_iter<T: IntoIterator<Item = Log>>(iter: T) -> Self {
        let inner: VecDeque<_> = iter.into_iter().collect();
        Self::Log(inner)
    }
}

impl From<Vec<Header>> for SubscriptionBuffer {
    fn from(headers: Vec<Header>) -> Self {
        Self::Block(headers.into())
    }
}

impl FromIterator<Header> for SubscriptionBuffer {
    fn from_iter<T: IntoIterator<Item = Header>>(iter: T) -> Self {
        let inner: VecDeque<_> = iter.into_iter().collect();
        Self::Block(inner)
    }
}

/// Tracks ongoing subscription tasks.
///
/// Performs the following functions:
/// - assigns unique subscription IDs
/// - spawns tasks to manage each subscription
/// - allows cancelling subscriptions by ID
///
/// Calling [`Self::new`] spawns a task that periodically cleans stale filters.
/// This task runs on a separate thread to avoid [`DashMap::retain`] deadlock.
/// See [`DashMap`] documentation for more information.
#[derive(Clone)]
pub struct SubscriptionManager<N: Pnt> {
    inner: Arc<SubscriptionManagerInner<N>>,
}

impl<N: Pnt> SubscriptionManager<N> {
    /// Instantiate a new subscription manager, start a task to clean up
    /// subscriptions cancelled by user disconnection
    pub fn new(provider: BlockchainProvider<N>, clean_interval: Duration) -> Self {
        let inner = Arc::new(SubscriptionManagerInner::new(provider));
        let task = SubCleanerTask::new(Arc::downgrade(&inner), clean_interval);
        task.spawn();
        Self { inner }
    }
}

impl<N: Pnt> core::ops::Deref for SubscriptionManager<N> {
    type Target = SubscriptionManagerInner<N>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<N: Pnt> core::fmt::Debug for SubscriptionManager<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SubscriptionManager").finish_non_exhaustive()
    }
}

/// Inner logic for [`SubscriptionManager`].
#[derive(Debug)]
pub struct SubscriptionManagerInner<N>
where
    N: Pnt,
{
    next_id: AtomicU64,
    tasks: DashMap<U64, CancellationToken>,
    provider: BlockchainProvider<N>,
}

impl<N: Pnt> SubscriptionManagerInner<N> {
    /// Create a new subscription manager.
    pub fn new(provider: BlockchainProvider<N>) -> Self {
        Self { next_id: AtomicU64::new(1), tasks: DashMap::new(), provider }
    }

    /// Assign a new subscription ID.
    fn next_id(&self) -> U64 {
        U64::from(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Cancel a subscription task.
    pub fn unsubscribe(&self, id: U64) -> bool {
        if let Some(task) = self.tasks.remove(&id) {
            task.1.cancel();
            true
        } else {
            false
        }
    }

    /// Subscribe to notifications. Returns `None` if notifications are
    /// disabled.
    pub fn subscribe(&self, ajj_ctx: &HandlerCtx, filter: InterestKind) -> Option<U64> {
        if !ajj_ctx.notifications_enabled() {
            return None;
        }

        let id = self.next_id();
        let token = CancellationToken::new();
        let task = SubscriptionTask {
            id,
            filter,
            token: token.clone(),
            notifs: self.provider.subscribe_to_canonical_state(),
        };
        task.spawn(ajj_ctx);

        debug!(%id, "registered new subscription");

        Some(id)
    }
}

/// Task to manage a single subscription.
#[derive(Debug)]
struct SubscriptionTask {
    id: U64,
    filter: InterestKind,
    token: CancellationToken,
    notifs: CanonStateNotifications,
}

impl SubscriptionTask {
    /// Create the task future.
    pub(crate) async fn task_future(
        self,
        ajj_ctx: HandlerCtx,
        ajj_cancel: WaitForCancellationFutureOwned,
    ) {
        let SubscriptionTask { id, filter, token, mut notifs } = self;

        let Some(sender) = ajj_ctx.notifications() else { return };

        // Buffer for notifications to be sent to the client
        let mut notif_buffer = filter.empty_sub_buffer();
        tokio::pin!(ajj_cancel);

        loop {
            let span = debug_span!(parent: None, "SubscriptionTask::task_future", %id, filter = tracing::field::Empty);
            if enabled!(tracing::Level::TRACE) {
                span.record("filter", format!("{filter:?}"));
            }

            let guard = span.enter();
            // This future checks if the notification buffer is non-empty and
            // waits for the sender to have some capacity before sending.
            let permit_fut = async {
                if !notif_buffer.is_empty() {
                    // NB: we reserve half the capacity to avoid blocking other
                    // usage. This is a heuristic and can be adjusted as needed.
                    sender.reserve_many(min(sender.max_capacity() / 2, notif_buffer.len())).await
                } else {
                    // If the notification buffer is empty, just never return
                    pending().await
                }
            }
            .in_current_span();
            drop(guard);

            // NB: this select is biased, this ensures that the outbound
            // buffer is either drained, or blocked on permits before checking
            // the inbound buffer
            tokio::select! {
                biased;
                _ = &mut ajj_cancel => {
                    let _guard = span.enter();
                    // if AJJ cancelled us via client disconnect, we should
                    // cancel the token so that we can be reaped by the
                    // subscription cleaner task.
                    trace!("subscription cancelled by client disconnect");
                    token.cancel();
                    break;
                }
                _ = token.cancelled() => {
                    // If the token is cancelled, this subscription has been
                    // cancelled by eth_unsubscribe
                    let _guard = span.enter();
                    trace!("subscription cancelled by user");
                    break;
                }
                permits = permit_fut => {
                    let _guard = span.enter();
                    // channel closed
                    let Ok(permits) = permits else {
                        trace!("channel to client closed");
                        break
                    };

                    for permit in permits {
                        // Send notification to the client for each permit.
                        let Some(item) = notif_buffer.pop_front() else {
                            // if we run out of notifications, we should break
                            // This would be weird, as we only allocated
                            // permits for notifications we had. Let's handle it anyway.
                            break;
                        };
                        let notification = ajj::serde_json::json!{
                            {
                                "jsonrpc": "2.0",
                                "method": "eth_subscription",
                                "params": {
                                    "result": &item,
                                    "subscription": id
                                },
                            }
                        };
                        // Serialize and send.
                        let Ok(brv) = serde_json::value::to_raw_value(&notification) else {
                            trace!(?item, "failed to serialize notification");
                            continue
                        };
                        permit.send(brv);
                    }
                }
                notif_res = notifs.recv() => {
                    let _guard = span.enter();

                    let notif = match notif_res {
                        Ok(notif) => notif,
                        Err(RecvError::Lagged(skipped)) => {
                            trace!(skipped, "missed notifications");
                            continue;
                        },
                        Err(e) =>{
                            trace!(?e, "CanonStateNotifications stream closed");
                            break;
                        }
                    };

                    let output = filter.filter_notification_for_sub(&notif);

                    trace!(count = output.len(), "Filter applied to notification");
                    if !output.is_empty() {
                        // NB: this will panic if the filter type changes
                        // mid-task. But that should never happen as it would
                        // break API guarantees anyway
                        notif_buffer.extend(output);
                    }
                }
            }
        }
    }

    /// Spawn on the ajj [`HandlerCtx`].
    pub(crate) fn spawn(self, ctx: &HandlerCtx) {
        ctx.spawn_graceful_with_ctx(|ctx, ajj_cancel| self.task_future(ctx, ajj_cancel));
    }
}

/// Task to clean up cancelled subscriptions.
///
/// This task runs on a separate thread to avoid [`DashMap::retain`] deadlocks.
#[derive(Debug)]
pub(super) struct SubCleanerTask<N: Pnt> {
    inner: Weak<SubscriptionManagerInner<N>>,
    interval: std::time::Duration,
}

impl<N: Pnt> SubCleanerTask<N> {
    /// Create a new subscription cleaner task.
    pub(super) const fn new(
        inner: Weak<SubscriptionManagerInner<N>>,
        interval: std::time::Duration,
    ) -> Self {
        Self { inner, interval }
    }

    /// Run the task. This task runs on a separate thread, which ensures that
    /// [`DashMap::retain`]'s deadlock condition is not met. See [`DashMap`]
    /// documentation for more information.
    pub(super) fn spawn(self) {
        std::thread::spawn(move || loop {
            std::thread::sleep(self.interval);
            if let Some(inner) = self.inner.upgrade() {
                inner.tasks.retain(|_, task| !task.is_cancelled());
            }
        });
    }
}
