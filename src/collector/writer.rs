use crate::BatteryInfo;
use crate::collector::{SampleBuffer, metrics::batch_to_influx};
use reqwest::Client;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub struct VmWriter {
    client: Client,
    url: String,
    buffer: SampleBuffer,
    cancel: CancellationToken,
}

impl VmWriter {
    pub fn new(vm_url: &str, buffer: SampleBuffer, cancel: CancellationToken) -> Self {
        let url = format!("{}/write", vm_url.trim_end_matches('/'));

        Self {
            client: Client::new(),
            url,
            buffer,
            cancel,
        }
    }

    pub async fn run(&self) {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(60);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                _ = self.cancel.cancelled() => {
                    self.flush_on_shutdown().await;
                    return;
                }
            }

            let samples = self.buffer.drain_all();
            if samples.is_empty() {
                tracing::trace!("Buffer empty, waiting...");
                continue;
            }
            tracing::debug!("Draining {} samples from buffer", samples.len());

            match self.write_samples(&samples).await {
                Ok(()) => {
                    tracing::debug!("Wrote {} samples to VictoriaMetrics", samples.len());
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to write to VictoriaMetrics: {}. Retrying in {:?}",
                        e,
                        backoff
                    );
                    self.buffer.extend_front(samples);

                    tokio::select! {
                        _ = tokio::time::sleep(backoff) => {}
                        _ = self.cancel.cancelled() => {
                            self.flush_on_shutdown().await;
                            return;
                        }
                    }

                    backoff = (backoff * 2).min(max_backoff);
                }
            }
        }
    }

    async fn write_samples(&self, samples: &[BatteryInfo]) -> Result<(), String> {
        let body = batch_to_influx(samples);
        tracing::debug!("POST {} ({} bytes)", self.url, body.len());

        let response = self
            .client
            .post(&self.url)
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(format!("HTTP {}: {}", status, text))
        }
    }

    async fn flush_on_shutdown(&self) {
        let samples = self.buffer.drain_all();
        if samples.is_empty() {
            tracing::info!("Shutdown: no buffered samples to flush");
            return;
        }

        tracing::info!(
            "Shutdown: flushing {} buffered samples to VictoriaMetrics",
            samples.len()
        );

        let timeout = Duration::from_secs(30);
        match tokio::time::timeout(timeout, self.write_samples(&samples)).await {
            Ok(Ok(())) => {
                tracing::info!("Shutdown: successfully flushed all samples");
            }
            Ok(Err(e)) => {
                tracing::error!("Shutdown: failed to flush {} samples: {}", samples.len(), e);
            }
            Err(_) => {
                tracing::error!(
                    "Shutdown: timed out flushing {} samples after {:?}",
                    samples.len(),
                    timeout
                );
            }
        }
    }
}
