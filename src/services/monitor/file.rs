use serde::{Deserialize, Serialize};
use std::future::Future;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use crate::base::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub enum FileMonitorEvent {
    NewFile,
    NewLine(String),
}

pub struct FileMonitor {
    task_holder: Option<JoinHandle<()>>,
}

impl Default for FileMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl FileMonitor {
    pub fn new() -> Self {
        Self { task_holder: None }
    }

    pub async fn start<F, Fut>(&mut self, file_path: &str, on_update: F)
    where
        F: Fn(FileMonitorEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), AppError>> + Send + 'static,
    {
        let file_path_clone = file_path.to_string();

        let monitor = tokio::spawn(async move {
            loop {
                if let Err(err) = Self::start_monitor(file_path_clone.as_str(), &on_update).await {
                    eprintln!("file monitor {file_path_clone}: {err}; retrying");
                }
                sleep(Duration::from_secs(1)).await;
            }
        });

        if let Some(old_task) = self.task_holder.replace(monitor) {
            println!("Aborting old file monitor task");
            old_task.abort();
        }
    }

    pub async fn stop(&mut self) {
        if let Some(handle) = self.task_holder.take() {
            handle.abort();
        }
    }

    async fn start_monitor<F, Fut>(file_path: &str, on_update: F) -> Result<(), AppError>
    where
        F: Fn(FileMonitorEvent) -> Fut + Send + Sync,
        Fut: Future<Output = Result<(), AppError>> + Send + 'static,
    {
        let mut skip_existing = Path::new(file_path).exists();
        'reopen: loop {
            let file = match OpenOptions::new().read(true).open(file_path).await {
                Ok(file) => file,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(err) => return Err(err.into()),
            };
            let metadata = file.metadata().await?;
            let identity = (metadata.dev(), metadata.ino());
            let mut reader = BufReader::new(file);
            let mut position = if skip_existing { metadata.len() } else { 0 };
            skip_existing = false;
            reader.seek(SeekFrom::Start(position)).await?;
            let mut pending = Vec::new();
            loop {
                let current = match tokio::fs::metadata(file_path).await {
                    Ok(meta) => meta,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        let _ = on_update(FileMonitorEvent::NewFile).await;
                        continue 'reopen;
                    }
                    Err(err) => return Err(err.into()),
                };
                if (current.dev(), current.ino()) != identity {
                    let _ = on_update(FileMonitorEvent::NewFile).await;
                    continue 'reopen;
                }
                if current.len() < position {
                    pending.clear();
                    position = 0;
                    reader.seek(SeekFrom::Start(0)).await?;
                    let _ = on_update(FileMonitorEvent::NewFile).await;
                }
                // Bound each batch so rotations and other tasks are serviced.
                for _ in 0..512 {
                    let n = reader.read_until(b'\n', &mut pending).await?;
                    position += n as u64;
                    if n == 0 {
                        break;
                    }
                    if pending.len() > 256 * 1024 {
                        return Err("log line exceeds 256 KiB".into());
                    }
                    if pending.last() == Some(&b'\n') {
                        if let Ok(line) = std::str::from_utf8(&pending) {
                            let line = line.trim();
                            if !line.is_empty() {
                                let _ =
                                    on_update(FileMonitorEvent::NewLine(line.to_string())).await;
                            }
                        }
                        pending.clear();
                    }
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn follows_rotation_and_partial_lines() {
        let dir = std::env::temp_dir().join(format!("lx06-monitor-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir(&dir).await.unwrap();
        let path = dir.join("messages");
        tokio::fs::write(&path, b"old\n").await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut monitor = FileMonitor::new();
        monitor
            .start(path.to_str().unwrap(), move |event| {
                let tx = tx.clone();
                async move {
                    if let FileMonitorEvent::NewLine(s) = event {
                        let _ = tx.send(s);
                    }
                    Ok(())
                }
            })
            .await;
        sleep(Duration::from_millis(200)).await;
        use tokio::io::AsyncWriteExt;
        let mut file = OpenOptions::new().append(true).open(&path).await.unwrap();
        file.write_all(b"new-part").await.unwrap();
        sleep(Duration::from_millis(100)).await;
        assert!(rx.try_recv().is_err());
        file.write_all(b"-end\n").await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap(),
            "new-part-end"
        );
        tokio::fs::rename(&path, dir.join("old")).await.unwrap();
        tokio::fs::write(&path, b"rotated\n").await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .unwrap()
                .unwrap(),
            "rotated"
        );
        monitor.stop().await;
        drop(file);
        tokio::fs::remove_dir_all(dir).await.unwrap();
    }
}
