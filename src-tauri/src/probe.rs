//! "Bağlantıyı test et": seçili uygulamalara ve sitelere gerçek bir HTTPS isteği atar.
//!
//! İstek Windows'un kendi DNS'i ve TLS'iyle (SChannel) gider; yani Discord'un ya da tarayıcının gördüğünü görür.
//! Sunucudan herhangi bir HTTP yanıtı gelmesi (TLS el sıkışması geçerli sertifikayla tamamlandı demektir) "açılıyor"
//! sayılır. Engel sayfasına yönlendirme, bağlantının kesilmesi ya da zaman aşımı "açılmıyor"dur.

use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Target {
    pub id: String,
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Outcome {
    pub id: String,
    pub label: String,
    pub ok: bool,
    pub ms: u64,
}

pub async fn run(targets: Vec<Target>) -> Vec<Outcome> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) RobzDPI")
        .build();
    let Ok(client) = client else {
        return targets.into_iter().map(|t| Outcome { id: t.id, label: t.label, ok: false, ms: 0 }).collect();
    };

    let tasks: Vec<_> = targets
        .into_iter()
        .map(|target| {
            let client = client.clone();
            tokio::spawn(async move {
                let started = Instant::now();
                let ok = client.get(&target.url).send().await.is_ok();
                Outcome { id: target.id, label: target.label, ok, ms: started.elapsed().as_millis() as u64 }
            })
        })
        .collect();

    let mut outcomes = Vec::new();
    for task in tasks {
        if let Ok(outcome) = task.await {
            outcomes.push(outcome);
        }
    }
    outcomes
}
