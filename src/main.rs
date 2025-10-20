mod detector;

// main.rs 
use eframe::egui;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::thread;
use std::time::Instant;
use walkdir::WalkDir;
use zip::ZipArchive;

// ==================== СТРУКТУРЫ ====================

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
    match_score: usize,
}

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

// ==================== ДЕТЕКТОР ====================

#[derive(Clone)]
struct CheatDetector {
    database: HashMap<String, CheatInfo>,
}

impl CheatDetector {
    fn new() -> Self {
        let mut database = HashMap::new();

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

    fn check_jar_file(&self, jar_path: &Path) -> Option<ThreatResult> {
        let file_size = std::fs::metadata(jar_path).ok()?.len();
        let file_size_kb = file_size as f32 / 1024.0;

        let file = File::open(jar_path).ok()?;
        let reader = BufReader::new(file);
        let mut archive = ZipArchive::new(reader).ok()?;

        let max_files = archive.len().min(15);
        let mut file_list = Vec::with_capacity(max_files);

        for i in 0..max_files {
            if let Ok(file) = archive.by_index(i) {
                file_list.push(file.name().to_lowercase());
            }
        }

        let mut weight_matches = Vec::new();
        for (name, info) in &self.database {
            if !info.sizes_kb.is_empty()
                && self.check_weight_match(file_size_kb, &info.sizes_kb, 0.05) {
                weight_matches.push(name.as_str());
            }
        }

        let scan_all = weight_matches.is_empty();

        for (cheat_name, cheat_info) in &self.database {
            if !scan_all && !weight_matches.contains(&cheat_name.as_str()) {
                continue;
            }

            let mut conditions_met = Vec::new();

            if cheat_info.strict_mode
                && self.has_legit_libraries(&file_list, &cheat_info.exclude_dirs) {
                continue;
            }

            // Проверка директорий
            if !cheat_info.directories.is_empty() {
                if cheat_info.directories.iter().any(|dir| {
                    let dir_lower = dir.to_lowercase();
                    file_list.iter().any(|fp| fp.contains(&dir_lower))
                }) {
                    conditions_met.push("dir");
                }
            }

            // Проверка классов
            if !cheat_info.classes.is_empty() {
                if cheat_info.classes.iter().any(|class| {
                    let class_lower = class.to_lowercase();
                    file_list.iter().any(|fp| fp.contains(&class_lower))
                }) {
                    conditions_met.push("class");
                }
            }

            // Проверка веса
            if !cheat_info.sizes_kb.is_empty()
                && self.check_weight_match(file_size_kb, &cheat_info.sizes_kb, 0.05) {
                conditions_met.push("weight");
            }

            // Проверка условий
            let is_threat = if cheat_info.strict_mode {
                let has_dir = !cheat_info.directories.is_empty()
                    && conditions_met.contains(&"dir");
                let has_class = !cheat_info.classes.is_empty()
                    && conditions_met.contains(&"class");
                has_dir && has_class
            } else {
                conditions_met.len() >= cheat_info.min_conditions
            };

            if is_threat {
                return Some(ThreatResult {
                    path: jar_path.display().to_string(),
                    name: jar_path.file_name()?.to_str()?.to_string(),
                    size: file_size,
                    cheat_type: cheat_name.clone(),
                    details: vec![
                        cheat_info.description.clone(),
                        format!("{:.1} KB", file_size_kb),
                    ],
                    match_score: conditions_met.len(),
                });
            }
        }

        None
    }

    fn check_weight_match(&self, file_size_kb: f32, cheat_sizes: &[f32], tolerance: f32) -> bool {
        cheat_sizes.iter().any(|&target| {
            let min = target * (1.0 - tolerance);
            let max = target * (1.0 + tolerance);
            file_size_kb >= min && file_size_kb <= max
        })
    }

    fn has_legit_libraries(&self, files: &[String], excludes: &[String]) -> bool {
        if excludes.is_empty() { return false; }
        files.iter().any(|fp|
            excludes.iter().any(|ex| fp.contains(&ex.to_lowercase()))
        )
    }
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

                    if let Some(start) = self.scan_start {
                        ui.separator();
                    }
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
                                ui.label(format!("Размер: {:.1} KB", threat.size as f32 / 1024.0));
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
        let detector = CheatDetector::new();
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