// main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod detector;


use detector::{CheatDetector, ThreatResult};
use eframe::egui;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;

// ==================== СТРУКТУРЫ ====================


#[derive(Debug, Clone)]
enum ScanMessage {
    Progress(f32),
    ThreatFound(ThreatResult),
    Stats(ScanStats),
    Complete,
}

#[derive(Debug, Clone)]
struct ScanStats {
    total: usize,
    checked: usize,
    found: usize,
}

// ==================== СКАНЕР ====================

fn find_jar_files(path: &Path) -> Vec<PathBuf> {
    WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .par_bridge()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.extension()?.to_str()?.eq_ignore_ascii_case("jar") {
                return None;
            }
            let size = std::fs::metadata(path).ok()?.len();
            if size >= 1024 && size <= 500 * 1024 * 1024 {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect()
}

fn scan_files(
    detector: CheatDetector,
    files: Vec<PathBuf>,
    sender: mpsc::Sender<ScanMessage>,
    cancel: Arc<AtomicBool>,
    num_threads: usize,
) {
    let total = files.len();
    let checked = Arc::new(AtomicUsize::new(0));
    let found = Arc::new(AtomicUsize::new(0));

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .ok();

    files.par_iter().for_each(|jar_path| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }

        if let Some(threat) = detector.check_jar_file(jar_path) {
            found.fetch_add(1, Ordering::Relaxed);
            sender.send(ScanMessage::ThreatFound(threat)).ok();
        }

        let current = checked.fetch_add(1, Ordering::Relaxed) + 1;

        // Обновление каждые 50 файлов
        if current % 50 == 0 || current == total {
            sender.send(ScanMessage::Progress(current as f32 / total as f32)).ok();
            sender.send(ScanMessage::Stats(ScanStats {
                total,
                checked: current,
                found: found.load(Ordering::Relaxed),
            })).ok();
        }
    });

    sender.send(ScanMessage::Complete).ok();
}

// ==================== GUI ====================

struct CheatDetectorApp {
    search_path: String,
    scanning: bool,
    stats: ScanStats,
    threats: Vec<ThreatResult>,
    scan_start: Option<Instant>,
    num_threads: usize,
    progress: f32,
    receiver: Option<mpsc::Receiver<ScanMessage>>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl Default for CheatDetectorApp {
    fn default() -> Self {
        Self {
            search_path: dirs::home_dir()
                .unwrap_or_default()
                .join(".minecraft/mods")
                .display()
                .to_string(),
            scanning: false,
            stats: ScanStats { total: 0, checked: 0, found: 0 },
            threats: Vec::new(),
            scan_start: None,
            num_threads: num_cpus::get().clamp(2, 8),
            progress: 0.0,
            receiver: None,
            cancel_flag: None,
        }
    }
}

impl eframe::App for CheatDetectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Обработка сообщений
        if let Some(ref receiver) = self.receiver {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    ScanMessage::Progress(p) => self.progress = p,
                    ScanMessage::ThreatFound(t) => self.threats.push(t),
                    ScanMessage::Stats(s) => self.stats = s,
                    ScanMessage::Complete => {
                        self.scanning = false;
                        self.progress = 1.0;
                    }
                }
            }
        }

        ctx.set_visuals(egui::Visuals::dark());

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(10.0);

            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("🛡️ SB|Cheats ")
                    .size(24.0).strong());
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Панель управления
            ui.group(|ui| {
                ui.label(egui::RichText::new("📁 Управление").strong());
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Путь:");
                    ui.text_edit_singleline(&mut self.search_path);
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("📂 Обзор").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            self.search_path = p.display().to_string();
                        }
                    }

                    let btn = if self.scanning {
                        egui::Button::new("⏳ Сканирование...")
                    } else {
                        egui::Button::new("🔍 Начать сканирование")
                    };

                    if ui.add(btn).clicked() && !self.scanning {
                        self.start_scan();
                    }

                    if self.scanning && ui.button("❌ Отмена").clicked() {
                        if let Some(ref flag) = self.cancel_flag {
                            flag.store(true, Ordering::Relaxed);
                        }
                    }

                    ui.label(format!("Потоков: {}", self.num_threads));
                    ui.add(egui::Slider::new(&mut self.num_threads, 1..=16));
                });

                if self.scanning {
                    ui.add_space(8.0);
                    ui.add(egui::ProgressBar::new(self.progress)
                        .text(format!("{:.0}%", self.progress * 100.0)));
                }
            });

            ui.add_space(10.0);

            // Статистика
            ui.group(|ui| {
                ui.label(egui::RichText::new("📊 Статистика").strong());
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(format!("📦 Всего: {}", self.stats.total));
                    ui.separator();
                    ui.label(format!("✅ Проверено: {}", self.stats.checked));
                    ui.separator();
                    let color = if self.stats.found > 0 {
                        egui::Color32::from_rgb(248, 180, 73)
                    } else {
                        egui::Color32::GREEN
                    };
                    ui.label(egui::RichText::new(format!("⚠️ Найдено: {}", self.stats.found))
                        .color(color));
                });
            });

            ui.add_space(10.0);

            // Угрозы
            if !self.threats.is_empty() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(format!("⚠️ Угрозы: {}", self.threats.len()))
                        .strong().color(egui::Color32::from_rgb(248, 100, 73)));

                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        for threat in &self.threats {
                            ui.group(|ui| {
                                ui.label(egui::RichText::new(&threat.name)
                                    .strong().color(egui::Color32::RED));
                                ui.label(format!("Тип: {}", threat.cheat_type));
                                for detail in &threat.details {
                                    ui.label(detail);
                                }
                                ui.label(egui::RichText::new(&threat.path)
                                    .small().color(egui::Color32::GRAY));
                            });
                        }
                    });
                });
            }
        });

        if self.scanning {
            ctx.request_repaint();
        }
    }
}

impl CheatDetectorApp {
    fn start_scan(&mut self) {
        self.threats.clear();
        self.stats = ScanStats { total: 0, checked: 0, found: 0 };
        self.progress = 0.0;
        self.scanning = true;
        self.scan_start = Some(Instant::now());

        let path = PathBuf::from(&self.search_path);
        let detector = CheatDetector::new(); // ИСПОЛЬЗУЕМ detector.rs!
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel.clone());
        let num_threads = self.num_threads;

        thread::spawn(move || {
            let files = find_jar_files(&path);
            sender.send(ScanMessage::Stats(ScanStats {
                total: files.len(),
                checked: 0,
                found: 0,
            })).ok();

            scan_files(detector, files, sender, cancel, num_threads);
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    std::env::set_var("RUST_LOG", "off");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("🛡️ SB|Cheats Scanner"),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "Cheat Detector",
        options,
        Box::new(|_| Box::<CheatDetectorApp>::default()),
    )
}