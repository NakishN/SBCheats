// main.rs
use eframe::egui;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use walkdir::WalkDir;
use zip::ZipArchive;
use std::thread;
use std::sync::mpsc;
// ===================== СТРУКТУРЫ ДАННЫХ =====================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheatInfo {
    directories: Vec<String>,
    classes: Vec<String>,
    exclude_dirs: Vec<String>,
    sizes_kb: Vec<f32>,
    description: String,
    strict_mode: bool,
    min_conditions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThreatResult {
    path: String,
    name: String,
    size: u64,
    cheat_type: String,
    details: Vec<String>,
    found_signatures: Vec<String>,
    match_score: usize,
    conditions_met: Vec<String>,
}

#[derive(Debug, Clone)]
struct ScanStats {
    total: usize,
    checked: usize,
    found: usize,
    clean: usize,
}

#[derive(Debug, Clone)]
struct PerformanceSettings {
    max_threads: usize,
    batch_size: usize,
    low_memory_mode: bool,
}

#[derive(Debug, Clone)]
enum ScanMessage {
    Progress(f32),
    FileFound(String),
    ThreatFound(ThreatResult),
    Complete,
    Error(String),
}

impl Default for ScanStats {
    fn default() -> Self {
        Self {
            total: 0,
            checked: 0,
            found: 0,
            clean: 0,
        }
    }
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_threads: num_cpus::get().min(4), // Ограничиваем до 4 потоков по умолчанию
            batch_size: 10,
            low_memory_mode: false,
        }
    }
}

// ===================== ДЕТЕКТОР =====================

#[derive(Clone)]
struct CheatDetector {
    database: HashMap<String, CheatInfo>,
}

impl CheatDetector {
    fn new() -> Self {
        let mut database = HashMap::new();

        // DoomsDay - строгий режим, strict_mode: true - строгий режим
        database.insert(
            "DoomsDay".to_string(),
            CheatInfo {
                directories: vec!["net/java/".to_string()],
                classes: vec!["i.class".to_string()],
                exclude_dirs: vec![
                    "org/apache/".to_string(),
                    "com/google/".to_string(),
                    "io/netty/".to_string(),
                    "net/minecraft/".to_string(),
                    "net/minecraftforge/".to_string(),
                    "optifine/".to_string(),
                    "javax/".to_string(),
                    "sun/".to_string(),
                    "org/lwjgl/".to_string(),
                ],
                sizes_kb: vec![],
                description: "DoomsDay чит (опасный)".to_string(),
                strict_mode: true,
                min_conditions: 3,
            },
        );

        // Freecam
        database.insert(
            "Freecam".to_string(),
            CheatInfo {
                directories: vec!["net/xolt/freecam/".to_string()],
                classes: vec!["freecam.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![42.0, 74.0, 1047.0, 1048.0, 1059.0, 1069.0, 1075.0, 1104.0, 1111.0, 1117.0, 1122.0, 1124.0, 1130.0],
                description: "Freecam мод".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // NekoClient
        database.insert(
            "NekoClient".to_string(),
            CheatInfo {
                directories: vec![
                    "net/redteadev/nekoclient/".to_string(),
                    "zrhx/nekoparts/".to_string(),
                ],
                classes: vec!["NekoClient.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![40.0],
                description: "NekoClient Ghost чит".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // SeedCracker
        database.insert(
            "SeedCracker".to_string(),
            CheatInfo {
                directories: vec!["kaptainwutax/seedcracker/".to_string()],
                classes: vec!["SeedCracker.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![607.0],
                description: "SeedCracker".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Britva
        database.insert(
            "Britva".to_string(),
            CheatInfo {
                directories: vec!["britva/britva/".to_string(), "me/britva/myst/".to_string()],
                classes: vec!["britva.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![1207.0, 782.0, 24.0, 4503.0],
                description: "Britva Ghost/AutoMyst".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Troxill
        database.insert(
            "Troxill".to_string(),
            CheatInfo {
                directories: vec!["ru/zdcoder/troxill/".to_string(), "the/dmkn/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![1457.0, 165.0, 557.0, 167.0, 603.0],
                description: "Troxill Crack".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // AutoBuy
        database.insert(
            "AutoBuy".to_string(),
            CheatInfo {
                directories: vec![
                    "me/lithium/autobuy/".to_string(),
                    "com/ch0ffaindustries/ch0ffa_mod/".to_string(),
                    "ru/xorek/nbtautobuy/".to_string(),
                    "dev/sxmurxy/".to_string(),
                ],
                classes: vec!["autobuy.class".to_string(), "buyhelper.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![143.0, 301.0, 398.0, 7310.0, 269.0, 2830.0, 2243.0],
                description: "AutoBuy читы".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // WindyAutoMyst
        database.insert(
            "WindyAutoMyst".to_string(),
            CheatInfo {
                directories: vec!["dev/windymyst/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![93.0, 111.0],
                description: "WindyAutoMyst".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // HorekAutoBuy
        database.insert(
            "HorekAutoBuy".to_string(),
            CheatInfo {
                directories: vec!["bre2el/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![144.0, 136.0],
                description: "HorekAutoBuy (под fpsreducer)".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Inventory Walk
        database.insert(
            "Inventory Walk".to_string(),
            CheatInfo {
                directories: vec!["me/pieking1215/invmove/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![119.0, 122.0, 123.0, 125.0, 126.0],
                description: "Inventory Walk".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // WorldDownloader
        database.insert(
            "WorldDownloader".to_string(),
            CheatInfo {
                directories: vec!["wdl/".to_string()],
                classes: vec!["WorldBackup.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![574.0],
                description: "WorldDownloader".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Ezhitboxes
        database.insert(
            "Ezhitboxes".to_string(),
            CheatInfo {
                directories: vec!["me/bushroot/hb/".to_string(), "me/bush1root/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![8.0, 9.0, 10.0, 20.0, 21.0, 66.0],
                description: "Ezhitboxes/bush1root".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Ch0ffa
        database.insert(
            "Ch0ffa".to_string(),
            CheatInfo {
                directories: vec![
                    "com/ch0ffaindustries/ch0ffa_box/".to_string(),
                    "ch0ffaindustries/ch0ffa_box/".to_string(),
                ],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![58.0, 67.0],
                description: "Ch0ffa client".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // RastyPaster
        database.insert(
            "RastyPaster".to_string(),
            CheatInfo {
                directories: vec!["ua/RastyPaster/".to_string()],
                classes: vec!["RastyLegit".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![118.0, 138.0],
                description: "RastyPaster".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Minced
        database.insert(
            "Minced".to_string(),
            CheatInfo {
                directories: vec!["free/minced/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![1610.0],
                description: "Minced".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // ShareX
        database.insert(
            "ShareX".to_string(),
            CheatInfo {
                directories: vec!["ru/centbrowser/sharex/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![32.0, 76.0, 45.0],
                description: "ShareX".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Rolleron
        database.insert(
            "Rolleron".to_string(),
            CheatInfo {
                directories: vec!["me/rolleron/launch/".to_string()],
                classes: vec!["This.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![30.0, 31.0, 32.0, 33.0, 34.0, 41.0, 43.0, 55.0, 64.0, 52.0],
                description: "Rolleron GH".to_string(),
                strict_mode: true,
                min_conditions: 2,
            },
        );

        // Bedrock Bricker
        database.insert(
            "Bedrock Bricker".to_string(),
            CheatInfo {
                directories: vec!["net/mcreator/bedrockmod/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![41.8],
                description: "Bedrock Bricker".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Double Hotbar
        database.insert(
            "Double Hotbar".to_string(),
            CheatInfo {
                directories: vec!["com/sidezbros/double_hotbar/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![29.0, 35.0, 36.0, 37.0, 42.0, 43.0],
                description: "Double Hotbar".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Elytra Swap
        database.insert(
            "Elytra Swap".to_string(),
            CheatInfo {
                directories: vec!["net/szum123321/elytra_swap/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![568.0],
                description: "Elytra Swap".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Armor Hotswap
        database.insert(
            "Armor Hotswap".to_string(),
            CheatInfo {
                directories: vec!["com/loucaskreger/armorhotswap/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![19.0, 20.0, 21.0, 28.0, 29.0],
                description: "Armor Hotswap".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // GUMBALLOFFMODE
        database.insert(
            "GUMBALLOFFMODE".to_string(),
            CheatInfo {
                directories: vec!["com/moandjiezana/toml/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![2701.0],
                description: "GUMBALLOFFMODE".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Librarian Trade Finder
        database.insert(
            "Librarian Trade Finder".to_string(),
            CheatInfo {
                directories: vec!["de/greenman999/Librarian/".to_string()],
                classes: vec!["Trade.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![94.0, 100.0, 101.0, 3203.0],
                description: "Librarian Trade Finder".to_string(),
                strict_mode: true,
                min_conditions: 2,
            },
        );

        // Auto Attack
        database.insert(
            "Auto Attack".to_string(),
            CheatInfo {
                directories: vec!["com/tfar/autoattack/".to_string(), "vin35/autoattack/".to_string()],
                classes: vec!["AutoAttack.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![4.0, 77.0],
                description: "Auto Attack".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Entity Outliner
        database.insert(
            "Entity Outliner".to_string(),
            CheatInfo {
                directories: vec!["net/entityoutliner/".to_string()],
                classes: vec!["EntityOutliner.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![32.0, 33.0, 39.0, 41.0],
                description: "Entity Outliner".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Camera Utils
        database.insert(
            "Camera Utils".to_string(),
            CheatInfo {
                directories: vec!["de/maxhenkel/camerautils/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![88.0, 296.0, 317.0, 344.0, 348.0],
                description: "Camera Utils".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Wall-Jump
        database.insert(
            "Wall-Jump".to_string(),
            CheatInfo {
                directories: vec!["com/jahirtrap/walljump/".to_string(), "genandnic/walljump/".to_string()],
                classes: vec!["WallJump.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![155.0, 159.0, 160.0, 161.0, 162.0, 163.0, 165.0],
                description: "Wall-Jump TXF".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // CrystalOptimizer
        database.insert(
            "CrystalOptimizer".to_string(),
            CheatInfo {
                directories: vec!["com/marlowcrystal/marlowcrystal/".to_string()],
                classes: vec!["MarlowCrystal.class".to_string(), "CrystalOptimizer.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![90.0, 97.0],
                description: "CrystalOptimizer".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // ClickCrystals
        database.insert(
            "ClickCrystals".to_string(),
            CheatInfo {
                directories: vec!["io/github/itzispyder/clickcrystals/".to_string()],
                classes: vec!["ClickCrystals.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![2800.0, 3000.0, 3200.0, 3500.0, 4000.0],
                description: "ClickCrystals".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // TAKKER
        database.insert(
            "TAKKER".to_string(),
            CheatInfo {
                directories: vec!["com/example/examplemod/Modules/".to_string()],
                classes: vec!["AfkTaker.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![9.0],
                description: "TAKKER (AfkTaker)".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Cortezz
        database.insert(
            "Cortezz".to_string(),
            CheatInfo {
                directories: vec!["client/cortezz/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![3599.0],
                description: "Cortezz client".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // DezC BetterFPS
        database.insert(
            "DezC BetterFPS".to_string(),
            CheatInfo {
                directories: vec!["com/dezc/betterfps/modules/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![52.0],
                description: "DezC BetterFPS HB".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // NeverVulcan
        database.insert(
            "NeverVulcan".to_string(),
            CheatInfo {
                directories: vec!["ru/nedan/vulcan/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![1232.0],
                description: "NeverVulcan".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // ArbuzMyst
        database.insert(
            "ArbuzMyst".to_string(),
            CheatInfo {
                directories: vec!["me/leansani/phasma/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![293.0, 298.0],
                description: "ArbuzMyst/Arbuz GH".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // SevenMyst
        database.insert(
            "SevenMyst".to_string(),
            CheatInfo {
                directories: vec!["assets/automyst/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![991.0, 992.0, 2346.0],
                description: "SevenMyst AutoMyst".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Francium
        database.insert(
            "Francium".to_string(),
            CheatInfo {
                directories: vec!["dev/jnic/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![875.0, 3041.0, 1283.0],
                description: "Francium".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // BetterHUD
        database.insert(
            "BetterHUD".to_string(),
            CheatInfo {
                directories: vec!["assets/minecraft/fragment/events/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![3557.0],
                description: "BetterHUD HB".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Waohitboxes
        database.insert(
            "Waohitboxes".to_string(),
            CheatInfo {
                directories: vec!["com/wao/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![36.0],
                description: "Waohitboxes".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // MinecraftOptimization
        database.insert(
            "MinecraftOptimization".to_string(),
            CheatInfo {
                directories: vec!["dev/minecraftoptimization/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![69.0],
                description: "MinecraftOptimization HB".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Jeed
        database.insert(
            "Jeed".to_string(),
            CheatInfo {
                directories: vec![],
                classes: vec!["mixins.jeed".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![43.0],
                description: "Jeed".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // ViaVersion
        database.insert(
            "ViaVersion".to_string(),
            CheatInfo {
                directories: vec!["com/viaversion/".to_string()],
                classes: vec![],
                exclude_dirs: vec![],
                sizes_kb: vec![5031.0],
                description: "ViaVersion".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // NoHurtCam DanilSimX
        database.insert(
            "NoHurtCam DanilSimX".to_string(),
            CheatInfo {
                directories: vec!["nohurtcam/".to_string()],
                classes: vec!["ML.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![95.0],
                description: "NoHurtCam DanilSimX".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        // Fabric hits
        database.insert(
            "Fabric hits".to_string(),
            CheatInfo {
                directories: vec!["net/fabricmc/example/mixin".to_string()],
                classes: vec!["RenderMixin.class".to_string()],
                exclude_dirs: vec![],
                sizes_kb: vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0],
                description: "Fabric hits".to_string(),
                strict_mode: false,
                min_conditions: 2,
            },
        );

        Self { database }
    }

    fn check_weight_match(&self, file_size_kb: f32, cheat_sizes: &[f32], tolerance: f32) -> bool {
        if cheat_sizes.is_empty() {
            return false;
        }
        cheat_sizes.iter().any(|&target_size| {
            let min_size = target_size * (1.0 - tolerance);
            let max_size = target_size * (1.0 + tolerance);
            file_size_kb >= min_size && file_size_kb <= max_size
        })
    }

    fn has_legit_libraries(&self, file_list: &[String], exclude_dirs: &[String]) -> bool {
        if exclude_dirs.is_empty() {
            return false;
        }
        file_list.iter().any(|filepath| {
            exclude_dirs
                .iter()
                .any(|exclude| filepath.to_lowercase().contains(&exclude.to_lowercase()))
        })
    }

    fn check_jar_file(&self, jar_path: &Path) -> Option<ThreatResult> {
        let file_size = std::fs::metadata(jar_path).ok()?.len();
        let file_size_kb = file_size as f32 / 1024.0;

        let file = File::open(jar_path).ok()?;
        let reader = BufReader::new(file);
        let mut archive = ZipArchive::new(reader).ok()?;

        // Оптимизированный сбор файлов - только первые 100 файлов
        let mut file_list = Vec::with_capacity(100);
        let max_files = archive.len().min(100);

        for i in 0..max_files {
            if let Ok(file) = archive.by_index(i) {
                file_list.push(file.name().to_lowercase());
            }
        }

        for (cheat_name, cheat_info) in &self.database {
            let mut conditions_met = Vec::new();
            let mut found_items = Vec::new();

            // Проверка легитимных библиотек для strict_mode
            if cheat_info.strict_mode
                && self.has_legit_libraries(&file_list, &cheat_info.exclude_dirs)
            {
                continue;
            }

            // 1. Проверка директорий
            let mut dir_found = false;
            if !cheat_info.directories.is_empty() {
                for directory in &cheat_info.directories {
                    let dir_lower = directory.to_lowercase();
                    if file_list.iter().any(|fp| fp.contains(&dir_lower)) {
                        dir_found = true;
                        found_items.push(format!("DIR: {}", directory));
                        break;
                    }
                }
                if dir_found {
                    conditions_met.push("directory".to_string());
                }
            }

            // 2. Проверка классов
            let mut class_found = false;
            if !cheat_info.classes.is_empty() {
                for class_name in &cheat_info.classes {
                    let class_lower = class_name.to_lowercase();
                    if file_list.iter().any(|fp| fp.contains(&class_lower)) {
                        class_found = true;
                        found_items.push(format!("CLASS: {}", class_name));
                        break;
                    }
                }
                if class_found {
                    conditions_met.push("class".to_string());
                }
            }

            // 3. Проверка веса
            if !cheat_info.sizes_kb.is_empty() {
                if self.check_weight_match(file_size_kb, &cheat_info.sizes_kb, 0.05) {
                    conditions_met.push("weight".to_string());
                    found_items.push(format!("WEIGHT: {:.1}KB", file_size_kb));
                }
            }

            // Проверка условий
            let is_threat = if cheat_info.strict_mode {
                let mut required = Vec::new();
                if !cheat_info.directories.is_empty() {
                    required.push("directory");
                }
                if !cheat_info.classes.is_empty() {
                    required.push("class");
                }
                required
                    .iter()
                    .all(|&cond| conditions_met.contains(&cond.to_string()))
            } else {
                conditions_met.len() >= cheat_info.min_conditions
            };

            if is_threat {
                let mut details = vec![
                    format!("🚨 {}", cheat_info.description),
                    format!("Размер: {:.1} KB", file_size_kb),
                    format!("Совпадений: {}", conditions_met.len()),
                    format!("Условия: {}", conditions_met.join(", ")),
                ];

                if cheat_info.strict_mode {
                    details.push("⚠️ СТРОГИЙ РЕЖИМ".to_string());
                }

                return Some(ThreatResult {
                    path: jar_path.display().to_string(),
                    name: jar_path.file_name()?.to_str()?.to_string(),
                    size: file_size,
                    cheat_type: cheat_name.clone(),
                    details,
                    found_signatures: found_items,
                    match_score: conditions_met.len(),
                    conditions_met,
                });
            }
        }

        None
    }

    fn find_jar_files(&self, search_path: &Path) -> Vec<PathBuf> {
        let mut jar_files = Vec::new();

        let walker = WalkDir::new(search_path)
            .max_depth(usize::MAX) // Без ограничений по глубине
            .follow_links(true)    // Следуем за символическими ссылками
            .same_file_system(false); // Сканируем разные файловые системы

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // Проверяем расширение файла
            if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
                if extension.eq_ignore_ascii_case("jar") {
                    if let Ok(metadata) = std::fs::metadata(path) {
                        let size = metadata.len();
                        // Принимаем файлы от 1KB до 500MB
                        if size >= 1024 && size <= 500 * 1024 * 1024 {
                            jar_files.push(path.to_path_buf());
                        }
                    }
                }
            }
        }
        jar_files
    }

    fn scan_files(
        &self,
        jar_files: Vec<PathBuf>,
        stats: Arc<Mutex<ScanStats>>,
        settings: &PerformanceSettings,
    ) -> Vec<ThreatResult> {
        // Настраиваем количество потоков
        rayon::ThreadPoolBuilder::new()
            .num_threads(settings.max_threads)
            .build_global()
            .unwrap_or_default();

        let results: Vec<_> = if settings.low_memory_mode {
            // Режим низкой памяти - последовательная обработка
            jar_files
                .iter()
                .filter_map(|path| {
                    let result = self.check_jar_file(path);

                    let mut stats = stats.lock().unwrap();
                    stats.checked += 1;
                    if result.is_some() {
                        stats.found += 1;
                    } else {
                        stats.clean += 1;
                    }

                    result
                })
                .collect()
        } else {
            // Обычный режим - параллельная обработка с батчами
            jar_files
                .chunks(settings.batch_size)
                .flat_map(|chunk| {
                    chunk
                        .par_iter()
                        .filter_map(|path| {
                            let result = self.check_jar_file(path);

                            let mut stats = stats.lock().unwrap();
                            stats.checked += 1;
                            if result.is_some() {
                                stats.found += 1;
                            } else {
                                stats.clean += 1;
                            }

                            result
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };

        results
    }
}

// ===================== GUI =====================

struct CheatDetectorApp {
    detector: CheatDetector,
    search_path: String,
    scanning: bool,
    stats: ScanStats,
    threats: Vec<ThreatResult>,
    log_messages: Vec<(String, String)>, // (message, level)
    scan_start_time: Option<Instant>,
    performance_settings: PerformanceSettings,
    show_performance_settings: bool,
    progress: f32,
    cancel_scan: bool,
    scan_receiver: Option<mpsc::Receiver<ScanMessage>>,
    scan_thread: Option<thread::JoinHandle<()>>,
}

impl Default for CheatDetectorApp {
    fn default() -> Self {
        Self {
            detector: CheatDetector::new(),
            search_path: dirs::home_dir()
                .unwrap_or_default()
                .join(".minecraft/mods")
                .display()
                .to_string(),
            scanning: false,
            stats: ScanStats::default(),
            threats: Vec::new(),
            log_messages: vec![(
                "🛡️ Cheat Detector v5.0 (Rust Edition) готов к работе".to_string(),
                "success".to_string(),
            )],
            scan_start_time: None,
            performance_settings: PerformanceSettings::default(),
            show_performance_settings: false,
            progress: 0.0,
            cancel_scan: false,
            scan_receiver: None,
            scan_thread: None,
        }
    }
}

impl eframe::App for CheatDetectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Обработка сообщений от потока сканирования
        if let Some(ref receiver) = self.scan_receiver {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    ScanMessage::Progress(progress) => {
                        self.progress = progress;
                    }
                    ScanMessage::FileFound(filename) => {
                        if filename.starts_with("📦 Найдено") {
                            // Это сообщение о количестве файлов
                            self.stats.total = filename.split_whitespace()
                                .find_map(|s| s.parse::<usize>().ok())
                                .unwrap_or(0);
                            self.log_messages.push((
                                format!("{}", filename),
                                "success".to_string(),
                            ));
                        } else if filename.starts_with("🔍 Ищем") {
                            // Это сообщение о начале поиска
                            self.log_messages.push((
                                format!("{}", filename),
                                "info".to_string(),
                            ));
                        } else if filename.starts_with("Проверено файлов") {
                            // Это финальная статистика
                            self.stats.checked = filename.split_whitespace()
                                .find_map(|s| s.parse::<usize>().ok())
                                .unwrap_or(0);
                            self.stats.clean = self.stats.checked - self.stats.found;
                            self.log_messages.push((
                                format!("📊 {}", filename),
                                "success".to_string(),
                            ));
                        } else {
                            // Это сообщение о проверке файла
                            self.log_messages.push((
                                format!("🔍 Проверяем: {}", filename),
                                "info".to_string(),
                            ));
                        }
                    }
                    ScanMessage::ThreatFound(threat) => {
                        self.threats.push(threat);
                        self.stats.found += 1;
                        // НЕ обновляем checked здесь, это делается в финальной статистике
                    }
                    ScanMessage::Complete => {
                        self.scanning = false;
                        self.progress = 1.0;

                        // Останавливаем таймер
                        if let Some(start_time) = self.scan_start_time {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            self.log_messages.push((
                                format!("✅ Сканирование завершено за {:.1}с! Найдено угроз: {}", elapsed, self.stats.found),
                                if self.stats.found > 0 { "warning" } else { "success" }.to_string(),
                            ));
                        } else {
                            self.log_messages.push((
                                format!("✅ Сканирование завершено! Найдено угроз: {}", self.stats.found),
                                if self.stats.found > 0 { "warning" } else { "success" }.to_string(),
                            ));
                        }

                        // Сбрасываем таймер
                        self.scan_start_time = None;
                    }
                    ScanMessage::Error(error) => {
                        self.scanning = false;
                        self.log_messages.push((
                            format!("❌ Ошибка: {}", error),
                            "error".to_string(),
                        ));
                    }
                }
            }
        }

        // Отключаем отладочные сообщения egui
        ctx.set_visuals(egui::Visuals::dark());

        // Полностью отключаем отладочные сообщения
        ctx.memory_mut(|mem| {
            mem.options.repaint_on_widget_change = false;
            mem.options.screen_reader = false;
        });

        // Отключаем отладочные наложения
        // ctx.set_debug_on_hover(false); // Метод не существует в этой версии

        // Стилизация
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(12.0);
        ctx.set_style(style);

        egui::CentralPanel::default().show(ctx, |ui| {
            // Заголовок с градиентным фоном
            ui.vertical(|ui| {
                ui.add_space(10.0);

                // Заголовок
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                    ui.heading(
                        egui::RichText::new("🛡️ SB|Cheats Detector")
                            .size(24.0)
                            .strong()
                    );
                    ui.label(
                        egui::RichText::new("Строгая проверка | База: 50+ читов")
                            .color(egui::Color32::from_rgb(160, 160, 160))
                    );
                });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                // Панель управления
                ui.group(|ui| {
                    ui.label(egui::RichText::new("📁 Панель управления").strong().size(16.0));
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Путь сканирования:");
                        ui.add(egui::TextEdit::singleline(&mut self.search_path)
                            .desired_width(ui.available_width() - 150.0)
                            .hint_text("Укажите путь к папке mods..."));
                    });

                    // Предупреждение о больших дисках
                    if self.search_path == "C:\\" || self.search_path == "C:" {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("⚠️ ВНИМАНИЕ: Сканирование всего диска C: может занять много времени!")
                            .color(egui::Color32::from_rgb(248, 180, 73))
                            .strong());
                        ui.label(egui::RichText::new("Рекомендуется сканировать только папку .minecraft/mods")
                            .color(egui::Color32::from_rgb(160, 160, 160))
                            .small());
                        ui.label(egui::RichText::new("💡 Для полного сканирования: используйте папку Downloads или Desktop")
                            .color(egui::Color32::from_rgb(100, 150, 200))
                            .small());
                    } else if self.search_path.contains("Downloads") || self.search_path.contains("Desktop") {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("✅ Хороший выбор! Папка с загрузками/рабочим столом")
                            .color(egui::Color32::from_rgb(63, 185, 80))
                            .small());
                    }

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        if ui.add_sized(
                            [120.0, 36.0],
                            egui::Button::new("📁 Обзор папки")
                                .fill(egui::Color32::from_rgb(70, 130, 180))
                        ).clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.search_path = path.display().to_string();
                            }
                        }

                        let scan_button = if self.scanning {
                            egui::Button::new("⏳ Сканирование...")
                                .fill(egui::Color32::from_rgb(210, 180, 40))
                        } else {
                            egui::Button::new("🔍 Начать сканирование")
                                .fill(egui::Color32::from_rgb(65, 185, 85))
                        };

                        if ui.add_sized([180.0, 36.0], scan_button).clicked() && !self.scanning {
                            self.start_scan();
                        }

                        if self.scanning {
                            if ui.add_sized(
                                [120.0, 36.0],
                                egui::Button::new("❌ Отмена")
                                    .fill(egui::Color32::from_rgb(248, 81, 73))
                            ).clicked() {
                                self.cancel_scan = true;
                            }
                        }

                        if ui.add_sized(
                            [120.0, 36.0],
                            egui::Button::new("💾 Экспорт отчета")
                                .fill(egui::Color32::from_rgb(100, 100, 160))
                        ).clicked() {
                            self.export_results();
                        }

                        if ui.add_sized(
                            [140.0, 36.0],
                            egui::Button::new("⚙️ Настройки")
                                .fill(egui::Color32::from_rgb(120, 120, 120))
                        ).clicked() {
                            self.show_performance_settings = !self.show_performance_settings;
                        }
                    });

                    // Прогресс-бар
                    if self.scanning {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("Прогресс:");
                            ui.add(egui::ProgressBar::new(self.progress)
                                .text(format!("{:.1}%", self.progress * 100.0)));
                        });
                    }
                });

                ui.add_space(15.0);

                // Статистика в карточках
                ui.group(|ui| {
                    ui.label(egui::RichText::new("📊 Статистика сканирования").strong().size(16.0));
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        let stats_card = |ui: &mut egui::Ui, label: &str, value: usize, color: egui::Color32| {
                            ui.vertical(|ui| {
                                ui.group(|ui| {
                                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(label).small().color(egui::Color32::from_rgb(160, 160, 160)));
                                        ui.add_space(4.0);
                                        ui.heading(egui::RichText::new(value.to_string()).color(color).size(20.0));
                                    });
                                });
                            });
                        };

                        // Статистика с фиксированными размерами
                        ui.allocate_ui_with_layout(
                            egui::vec2(150.0, 80.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                stats_card(ui, "📦 Всего файлов", self.stats.total, egui::Color32::from_rgb(88, 166, 255));
                            }
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(150.0, 80.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                stats_card(ui, "✅ Проверено", self.stats.checked, egui::Color32::from_rgb(63, 185, 80));
                            }
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(150.0, 80.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                stats_card(ui, "⚠️ Угроз найдено", self.stats.found, egui::Color32::from_rgb(248, 180, 73));
                            }
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(150.0, 80.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                stats_card(ui, "✔️ Чистых файлов", self.stats.clean, egui::Color32::from_rgb(63, 185, 80));
                            }
                        );

                        if let Some(start_time) = self.scan_start_time {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            ui.vertical(|ui| {
                                ui.group(|ui| {
                                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new("⏱️ Время").small().color(egui::Color32::from_rgb(160, 160, 160)));
                                        ui.add_space(4.0);
                                        ui.heading(egui::RichText::new(format!("{:.1}s", elapsed)).color(egui::Color32::from_rgb(180, 100, 220)).size(20.0));
                                    });
                                });
                            });
                        }
                    });
                });

                ui.add_space(15.0);

                // Настройки производительности
                if self.show_performance_settings {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new("⚙️ Настройки производительности").strong().size(16.0));
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label("Максимум потоков:");
                            ui.add(egui::Slider::new(&mut self.performance_settings.max_threads, 1..=num_cpus::get())
                                .text("потоков"));
                        });

                        ui.horizontal(|ui| {
                            ui.label("Размер батча:");
                            ui.add(egui::Slider::new(&mut self.performance_settings.batch_size, 1..=50)
                                .text("файлов"));
                        });

                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.performance_settings.low_memory_mode, "Режим низкой памяти");
                            ui.label("(для слабых компьютеров)");
                        });

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("💡 Рекомендации:")
                            .color(egui::Color32::from_rgb(160, 160, 160))
                            .small());
                        ui.label("• Слабые ПК: 1-2 потока, режим низкой памяти");
                        ui.label("• Средние ПК: 2-4 потока, батч 10-20");
                        ui.label("• Мощные ПК: 4+ потоков, батч 20-50");
                    });
                    ui.add_space(10.0);
                }

                // Таблица угроз (исправленная)
                if !self.threats.is_empty() {
                    ui.group(|ui| {
                        ui.label(
                            egui::RichText::new(format!("⚠️ Обнаруженные угрозы: {}", self.threats.len()))
                                .strong()
                                .size(16.0)
                                .color(egui::Color32::from_rgb(248, 100, 73))
                        );
                        ui.add_space(8.0);

                        egui::ScrollArea::vertical()
                            .id_source("threats_scroll")
                            .max_height(300.0)
                            .show(ui, |ui| {
                                for threat in &self.threats {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(&threat.name)
                                                            .strong()
                                                            .color(egui::Color32::from_rgb(248, 81, 73))
                                                    );
                                                    ui.add_space(10.0);
                                                    ui.label(
                                                        egui::RichText::new(&threat.cheat_type)
                                                            .color(egui::Color32::from_rgb(210, 153, 34))
                                                            .small()
                                                    );
                                                });

                                                ui.label(
                                                    egui::RichText::new(format!("Совпадений: {}/3 | Размер: {:.1} KB",
                                                                                threat.match_score, threat.size as f32 / 1024.0))
                                                        .small()
                                                        .color(egui::Color32::from_rgb(160, 160, 160))
                                                );

                                                ui.label(
                                                    egui::RichText::new(format!("Условия: {}", threat.conditions_met.join(", ")))
                                                        .small()
                                                );

                                                ui.label(
                                                    egui::RichText::new(&threat.path)
                                                        .small()
                                                        .color(egui::Color32::from_rgb(120, 120, 120))
                                                );
                                            });
                                        });
                                    });
                                    ui.add_space(5.0);
                                }
                            });
                    });
                    ui.add_space(10.0);
                }

                // Лог (исправленный)
                ui.group(|ui| {
                    ui.label(egui::RichText::new("📋 Лог событий").strong().size(16.0));
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical()
                        .id_source("log_scroll")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for (msg, level) in &self.log_messages {
                                let (icon, color) = match level.as_str() {
                                    "success" => ("✅", egui::Color32::from_rgb(63, 185, 80)),
                                    "warning" => ("⚠️", egui::Color32::from_rgb(210, 153, 34)),
                                    "error" => ("❌", egui::Color32::from_rgb(248, 81, 73)),
                                    _ => ("ℹ️", egui::Color32::from_rgb(88, 166, 255)),
                                };

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(icon).color(color));
                                    ui.label(egui::RichText::new(msg).color(color));
                                });
                            }
                        });
                });
            });
        });

        if self.scanning {
            ctx.request_repaint();
        }
    }
}

impl CheatDetectorApp {
    fn start_scan(&mut self) {
        // Очищаем предыдущие результаты
        self.threats.clear();
        self.stats = ScanStats::default();
        self.progress = 0.0;
        self.scanning = true;
        self.cancel_scan = false;
        self.scan_start_time = Some(Instant::now());

        let path = PathBuf::from(&self.search_path.clone());
        let detector = self.detector.clone();
        let _settings = self.performance_settings.clone();

        // Создаем канал для связи между потоками
        let (sender, receiver) = mpsc::channel();
        self.scan_receiver = Some(receiver);

        // Запускаем сканирование в отдельном потоке
        let path_clone = path.clone();
        let handle = thread::spawn(move || {
            sender.send(ScanMessage::Progress(0.1)).unwrap_or_default();
            sender.send(ScanMessage::FileFound(format!("🔍 Ищем JAR файлы в {:?}...", path_clone))).unwrap_or_default();

            let jar_files = detector.find_jar_files(&path_clone);
            sender.send(ScanMessage::Progress(0.2)).unwrap_or_default();
            sender.send(ScanMessage::FileFound(format!("📦 Найдено {} JAR файлов", jar_files.len()))).unwrap_or_default();

            if jar_files.is_empty() {
                sender.send(ScanMessage::Complete).unwrap_or_default();
                return;
            }

            // Сканирование файлов
            let total_files = jar_files.len();
            let mut checked = 0;
            let mut found = 0;

            for (i, jar_path) in jar_files.iter().enumerate() {
                // Отправляем информацию о текущем файле
                if let Some(filename) = jar_path.file_name().and_then(|n| n.to_str()) {
                    sender.send(ScanMessage::FileFound(filename.to_string())).unwrap_or_default();
                }

                // Проверяем файл
                checked += 1;
                if let Some(threat) = detector.check_jar_file(jar_path) {
                    found += 1;
                    sender.send(ScanMessage::ThreatFound(threat)).unwrap_or_default();
                }

                let progress = 0.2 + (i as f32 / total_files as f32) * 0.8;
                sender.send(ScanMessage::Progress(progress)).unwrap_or_default();
            }

            // Отправляем финальную статистику
            sender.send(ScanMessage::FileFound(format!("Проверено файлов: {}, найдено угроз: {}", checked, found))).unwrap_or_default();

            sender.send(ScanMessage::Complete).unwrap_or_default();
        });

        self.scan_thread = Some(handle);
        self.log_messages.push((
            format!("🚀 Начинаем асинхронное сканирование в {:?}...", path),
            "info".to_string(),
        ));
    }

    fn export_results(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("cheat_scan_report.json")
            .save_file()
        {
            let report = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "scan_path": self.search_path,
                "stats": {
                    "total": self.stats.total,
                    "checked": self.stats.checked,
                    "found": self.stats.found,
                    "clean": self.stats.clean,
                },
                "threats": self.threats,
            });

            if let Ok(mut file) = File::create(&path) {
                if file
                    .write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
                    .is_ok()
                {
                    self.log_messages.push((
                        format!("💾 Отчет успешно экспортирован: {:?}", path),
                        "success".to_string(),
                    ));
                }
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    // Полностью отключаем отладочные сообщения egui
    std::env::set_var("RUST_LOG", "off");
    std::env::set_var("EFRAME_LOG_LEVEL", "off");
    std::env::set_var("EGUI_LOG_LEVEL", "off");
    std::env::set_var("RUST_BACKTRACE", "0");
    std::env::set_var("EGUI_DEBUG", "false");
    std::env::set_var("EFRAME_DEBUG", "false");
    std::env::set_var("RUST_LOG_STYLE", "never");
    std::env::set_var("RUST_LOG_FILTER", "");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("🛡️ Cheat Detector v5.0 - Rust Edition")
            .with_resizable(true),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "Cheat Detector",
        options,
        Box::new(|_cc| Box::<CheatDetectorApp>::default()),
    )
}