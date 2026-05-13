use crate::{market_service::MarketService, ports::clock::Clock};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Driver that periodically calls `MarketService::refresh()`.
/// Emits a tick value (an incrementing counter) over a watch channel so consumers can wait on
/// "the most recent refresh finished". This is deliberately decoupled from IPC.
pub struct PollScheduler {
    market: Arc<MarketService>,
    clock: Arc<dyn Clock>,
    tick_tx: watch::Sender<u64>,
}

pub struct PollHandle { task: tokio::task::JoinHandle<()> }

impl PollHandle { pub fn abort(self) { self.task.abort(); } }

impl PollScheduler {
    pub fn new(market: Arc<MarketService>, clock: Arc<dyn Clock>) -> (Self, watch::Receiver<u64>) {
        let (tx, rx) = watch::channel(0);
        (Self { market, clock, tick_tx: tx }, rx)
    }

    pub fn start(self, interval: Duration) -> PollHandle {
        let task = tokio::spawn(async move {
            let mut counter: u64 = 0;
            loop {
                let _ = self.clock.now(); // forces dyn Clock to be live (and easy to mock-call in tests)
                if let Err(e) = self.market.refresh().await {
                    tracing::warn!(error = ?e, "poll refresh failed");
                }
                counter = counter.wrapping_add(1);
                let _ = self.tick_tx.send(counter);
                tokio::time::sleep(interval).await;
            }
        });
        PollHandle { task }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_service::MarketService;
    use crate::ports::asset_provider::MockAssetProvider;
    use crate::ports::clock::MockClock;
    use crate::ports::repos::MockWatchlistRepo;
    use chrono::Utc;
    use domain::watchlist::Watchlist;

    #[tokio::test(start_paused = true)]
    async fn ticks_at_least_twice_in_three_intervals() {
        let mut wl_repo = MockWatchlistRepo::new();
        wl_repo.expect_load().returning(|| Ok(Watchlist::new()));

        let mut prov = MockAssetProvider::new();
        prov.expect_supports().return_const(false);

        let mut clock = MockClock::new();
        clock.expect_now().returning(Utc::now);

        let market = Arc::new(MarketService::new(Arc::new(wl_repo), vec![Arc::new(prov)]));
        let (scheduler, mut rx) = PollScheduler::new(market, Arc::new(clock));
        let handle = scheduler.start(Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(160)).await;
        assert!(*rx.borrow_and_update() >= 2);
        handle.abort();
    }
}
