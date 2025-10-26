// scanner.rs - Оптимизированный модуль сканирования
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use rayon::prelude::*;
use walkdir::WalkDir;
use crate::detector::{CheatDetector, ThreatResult};

#[derive(Debug, Clone)]
pub enum ScanMessage {
    Progress(f32),
    ThreatFound(ThreatResult),
    Stats(ScanStats),
    Complete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ScanStats {
    pub total: usize,
    pub checked: usize,
    pub found: usize,
}

#[derive(Clone)]
pub struct Scanner {
    detector: CheatDetector,
    cancel_flag: Arc<AtomicBool>,
}

impl Scanner {
    pub fn new(detector: CheatDetector) -> Self {
        Self {
            detector,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn find_jar_files(&self, search_path: &Path) -> Vec<PathBuf> {
        WalkDir::new(search_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .par_bridge() // Параллельная обработка
            .filter_map(|entry| {
                let path = entry.path();

                // FAST PATH: проверка расширения
                if !path.extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("jar"))
                    .unwrap_or(false) {
                    return None;
                }

                // FAST PATH: проверка размера
                std::fs::metadata(path).ok()
                    .filter(|m| {
                        let size = m.len();
                        size >= 1024 && size <= 500 * 1024 * 1024
                    })
                    .map(|_| path.to_path_buf())
            })
            .collect()
    }

    // ОПТИМИЗАЦИЯ: Батч-сканирование с минимальными логами
    pub fn scan_files(
        &self,
        jar_files: Vec<PathBuf>,
        sender: Sender<ScanMessage>,
        num_threads: usize,
    ) -> Result<(), String> {
        if jar_files.is_empty() {
            return Ok(());
        }

        // Настройка пула потоков
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .map_err(|e| format!("Thread pool error: {}", e))?
            .install(|| {
                let total = jar_files.len();
                let checked = Arc::new(AtomicUsize::new(0));
                let found = Arc::new(AtomicUsize::new(0));

                // КРИТИЧЕСКАЯ ОПТИМИЗАЦИЯ: Батч-обработка без логов на каждый файл
                jar_files.par_iter()
                    .try_for_each(|jar_path| {
                        // Проверка отмены
                        if self.cancel_flag.load(Ordering::Relaxed) {
                            return Err("Cancelled");
                        }

                        // Сканируем файл
                        if let Some(threat) = self.detector.check_jar_file(jar_path) {
                            found.fetch_add(1, Ordering::Relaxed);
                            sender.send(ScanMessage::ThreatFound(threat))
                                .map_err(|_| "Channel closed")?;
                        }

                        let current = checked.fetch_add(1, Ordering::Relaxed) + 1;

                        // ОПТИМИЗАЦИЯ: Обновляем статистику каждые 50 файлов
                        if current % 50 == 0 || current == total {
                            let progress = current as f32 / total as f32;
                            sender.send(ScanMessage::Progress(progress))
                                .map_err(|_| "Channel closed")?;

                            sender.send(ScanMessage::Stats(ScanStats {
                                total,
                                checked: current,
                                found: found.load(Ordering::Relaxed),
                            })).map_err(|_| "Channel closed")?;
                        }

                        Ok::<_, &str>(())
                    })
                    .map_err(|e| e.to_string())?;

                // Финальная статистика
                sender.send(ScanMessage::Stats(ScanStats {
                    total,
                    checked: checked.load(Ordering::Relaxed),
                    found: found.load(Ordering::Relaxed),
                })).map_err(|_| "Channel closed".to_string())?;

                Ok(())
            })
    }
}