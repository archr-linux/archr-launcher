//! "Scrape again" from the game details: one forced single-game job. The
//! setup (config, credential, system list) runs on its own thread.
use crate::paths::Paths;
use archr_scraper::{Event, Job, Stop};
use std::{
    path::PathBuf,
    sync::mpsc::{channel, Receiver, TryRecvError},
    thread,
};

pub struct Rescrape {
    pub system: String,
    started: Receiver<Result<Job, String>>,
    job: Option<Job>,
}

impl Rescrape {
    pub fn start(paths: Paths, system: String, rom: PathBuf) -> Self {
        let (send, started) = channel();
        let name = system.clone();
        thread::spawn(move || {
            let job = crate::settings::start_scrape(&paths, Some((&name, rom)));
            let _ = send.send(job.map(|(job, _)| job));
        });
        Self {
            system,
            started,
            job: None,
        }
    }

    /// Locale key of the result once the job has ended; never blocks.
    pub fn poll(&mut self) -> Option<&'static str> {
        if self.job.is_none() {
            match self.started.try_recv() {
                Ok(Ok(job)) => self.job = Some(job),
                Ok(Err(error)) => {
                    log::warn!("Cannot scrape the game again: {error}");
                    return Some(if error == "settings.scraper_unavailable" {
                        "settings.scraper_unavailable"
                    } else {
                        "details.failed"
                    });
                }
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => return Some("details.failed"),
            }
        }
        while let Some(event) = self.job.as_ref()?.poll() {
            if let Event::Done(report) = event {
                let progress = report.progress;
                return Some(match report.stop {
                    Stop::Completed if progress.scraped > 0 => "details.updated",
                    Stop::Completed if progress.not_found > 0 => "details.not_found",
                    Stop::Completed => "details.failed",
                    stop => crate::settings::scrape_outcome(stop),
                });
            }
        }
        None
    }
}
