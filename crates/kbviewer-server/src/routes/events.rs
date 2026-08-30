//! Server-sent events, so a change made in Obsidian shows up in an open browser tab.

use crate::state::AppState;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// Proxies and phone radios drop idle connections; a periodic comment keeps the stream
/// alive without the client needing to reconnect.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.changes.subscribe();

    let stream = BroadcastStream::new(receiver).filter_map(|message| match message {
        Ok(change) => Event::default().json_data(change).ok().map(Ok),
        // A lagging client has missed messages. Dropping the errors keeps the connection
        // alive; the client refetches what it is showing on the next event it does see.
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEP_ALIVE_INTERVAL))
}
