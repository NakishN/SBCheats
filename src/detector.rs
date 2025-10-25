// detector.rs - Исправленная логика детектора
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheatInfo {
    pub directories: Vec<String>,
    pub classes: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub sizes_kb: Vec<f32>,
    pub description: String,
    pub strict_mode: bool,
    pub min_conditions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatResult {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub cheat_type: String,
    pub details: Vec<String>,
    pub match_score: usize,
}

#[derive(Clone)]
pub struct CheatDetector {
    database: HashMap<String, CheatInfo>,
}

impl CheatDetector {
    pub fn new() -> Self {
        let mut database = HashMap::new();
        Self::init_database(&mut database);
        Self { database }
    }

    pub fn check_jar_file(&self, jar_path: &Path) -> Option<ThreatResult> {
        let file_size = std::fs::metadata(jar_path).ok()?.len();
        let file_size_kb = file_size as f32 / 1024.0;

        let file = File::open(jar_path).ok()?;
        let reader = BufReader::new(file);
        let mut archive = ZipArchive::new(reader).ok()?;

        // Собираем все имена файлов в архиве
        let mut file_list = Vec::with_capacity(archive.len() as usize);
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                file_list.push(file.name().to_lowercase());
            }
        }

        // ОТЛАДКА для DoomsDay
        let jar_name = jar_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let has_net_java = file_list.iter().any(|f| f.contains("net/java/"));
        let has_i_class = file_list.iter().any(|f| f.contains("i.class"));

        if has_net_java || has_i_class {

            // Проверяем exclude_dirs
            let excludes = vec![
                "org/apache/", "com/google/", "io/netty/",
                "net/minecraft/", "net/minecraftforge/", "optifine/", "javax/"
            ];
            for ex in &excludes {
                if file_list.iter().any(|f| f.contains(ex)) {

                }
            }
        }

        // Проверяем каждый чит из базы
        for (cheat_name, cheat_info) in &self.database {
            // Проверка исключений для strict режима - СНАЧАЛА!
            if cheat_info.strict_mode
                && !cheat_info.exclude_dirs.is_empty()
                && self.has_legit_libraries(&file_list, &cheat_info.exclude_dirs) {
                continue;
            }

            // КРИТЕРИЙ 1: Проверка директории (ГЛАВНЫЙ)
            let has_directory = if !cheat_info.directories.is_empty() {
                cheat_info.directories.iter().any(|dir| {
                    let dir_lower = dir.to_lowercase();
                    file_list.iter().any(|fp| fp.contains(&dir_lower))
                })
            } else {
                false
            };

            // Если нет директории - сразу skip (кроме случаев где директория не указана)
            if !cheat_info.directories.is_empty() && !has_directory {
                continue;
            }

            // КРИТЕРИЙ 2: Проверка класса (ВАЖНЫЙ)
            let has_class = if !cheat_info.classes.is_empty() {
                cheat_info.classes.iter().any(|class| {
                    let class_lower = class.to_lowercase();
                    file_list.iter().any(|fp| fp.contains(&class_lower))
                })
            } else {
                false
            };

            // КРИТЕРИЙ 3: Проверка веса (ВСПОМОГАТЕЛЬНЫЙ, только если есть директория)
            let has_weight = if !cheat_info.sizes_kb.is_empty() && has_directory {
                self.check_weight_match(file_size_kb, &cheat_info.sizes_kb, 0.05)
            } else {
                false
            };

            // ЛОГИКА ОПРЕДЕЛЕНИЯ УГРОЗЫ:
            let is_threat = if cheat_info.strict_mode {
                // Strict: Директория + Класс обязательны
                has_directory && has_class
            } else {
                // Normal: Гибкая логика
                if has_directory && has_class {
                    // Идеально: директория + класс
                    true
                } else if has_directory && has_weight {
                    // Хорошо: директория + вес
                    true
                } else if has_directory && cheat_info.classes.is_empty() {
                    // OK: директория есть, классы не заданы
                    true
                } else if cheat_info.directories.is_empty() && has_class {
                    // Редкий случай: нет директорий в базе, но есть класс
                    true
                } else {
                    false
                }
            };

            if is_threat {
                // Считаем score для информации
                let mut match_score = 0;
                if has_directory { match_score += 1; }
                if has_class { match_score += 1; }
                if has_weight { match_score += 1; }

                return Some(ThreatResult {
                    path: jar_path.display().to_string(),
                    name: jar_path.file_name()?.to_str()?.to_string(),
                    size: file_size,
                    cheat_type: cheat_name.clone(),
                    details: vec![
                        cheat_info.description.clone(),
                        format!("{:.1} KB", file_size_kb),
                        format!("Совпадений: {}/3", match_score),
                    ],
                    match_score,
                });
            }
        }

        None
    }

    fn check_weight_match(&self, file_size_kb: f32, cheat_sizes: &[f32], tolerance: f32) -> bool {
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
        let excludes_lc: Vec<String> = exclude_dirs.iter().map(|e| e.to_lowercase()).collect();
        file_list.iter().any(|filepath| {
            excludes_lc.iter().any(|exclude| filepath.contains(exclude))
        })
    }

    fn init_database(database: &mut HashMap<String, CheatInfo>) {
        database.insert("DoomsDay".to_string(), CheatInfo {
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
            ],
            sizes_kb: vec![],
            description: "DoomsDay чит (опасный)".to_string(),
            strict_mode: true,  // Директория + Класс обязательны
            min_conditions: 2,
        });

        database.insert("Freecam".to_string(), CheatInfo {
            directories: vec!["net/xolt/freecam/".to_string()],
            classes: vec!["freecam.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![42.0, 74.0, 1047.0, 1048.0, 1069.0, 1104.0, 1122.0],
            description: "Freecam мод".to_string(),
            strict_mode: false,  // Гибкая проверка
            min_conditions: 2,
        });

        database.insert("Freecam2".to_string(), CheatInfo {
            directories: vec!["com/zergatul/freecam".to_string()],
            classes: vec!["FreeCam.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![42.0, 74.0, 1047.0, 1048.0, 1069.0, 1104.0, 1122.0],
            description: "Freecam мод (вариант 2)".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("NekoClient".to_string(), CheatInfo {
            directories: vec!["net/redteadev/nekoclient/".to_string()],
            classes: vec!["NekoClient.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![40.0],
            description: "NekoClient Ghost".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("SeedCracker".to_string(), CheatInfo {
            directories: vec!["kaptainwutax/seedcracker/".to_string()],
            classes: vec!["SeedCracker.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![607.0],
            description: "SeedCracker".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("Britva".to_string(), CheatInfo {
            directories: vec!["britva/britva/".to_string(), "me/britva/myst/".to_string()],
            classes: vec!["britva.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![1207.0, 782.0, 24.0, 4503.0],
            description: "Britva Ghost/AutoMyst".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("Inventory Move".to_string(), CheatInfo {
            directories: vec!["me/pieking1215/invmove/".to_string(), "me/pieking1215/".to_string()],
            classes: vec!["InvMove.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![331.0],
            description: "Inventory Move".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("WorldDownloader".to_string(), CheatInfo {
            directories: vec!["wdl/".to_string()],
            classes: vec!["WorldBackup.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![574.0],
            description: "WorldDownloader".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("AutoBuy".to_string(), CheatInfo {
            directories: vec![
                "me/lithium/autobuy/".to_string(),
                "ru/xorek/nbtautobuy/".to_string(),
            ],
            classes: vec!["autobuy.class".to_string(), "buyhelper.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![143.0, 301.0, 398.0, 7310.0],
            description: "AutoBuy читы".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("BedrockBricker".to_string(), CheatInfo {
            directories: vec!["net/mcreator/bedrockmod".to_string(), "net/anawesomguy/breakingbedrock".to_string()],
            classes: vec!["BedrockBlock.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![41.8],
            description: "Bedrock Bricker мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("ViaVersion".to_string(), CheatInfo {
            directories: vec!["com/viaversion/fabric/common".to_string()],
            classes: vec!["ViaFabric.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![5031.0],
            description: "ViaVersion мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("DoubleHotbar".to_string(), CheatInfo {
            directories: vec!["com/sidezbros/double_hotbar".to_string()],
            classes: vec!["DoubleHotbar.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![29.0, 35.0, 36.0, 37.0, 42.0, 43.0],
            description: "Double Hotbar мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("ElytraSwap".to_string(), CheatInfo {
            directories: vec![
                "net/szum123321/elytra_swap".to_string(),
                "com/saolghra/elytraswapper".to_string(),
                "io/github/jumperonjava/jjelytraswap".to_string()
            ],
            classes: vec![
                "ElytraSwap.class".to_string(),
                "Elytraswapper.class".to_string(),
                "ConfigScreen.class".to_string()
            ],
            exclude_dirs: vec![],
            sizes_kb: vec![568.0],
            description: "Elytra Swap мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("ArmorHotswap".to_string(), CheatInfo {
            directories: vec![
                "com/loucaskreger/armorhotswap".to_string(),
                "heyblack/betterarmorswap/mixin".to_string()
            ],
            classes: vec![
                "ArmorHotswap.class".to_string(),
                "ClientPlayerInteractionManagerMixin.class".to_string()
            ],
            exclude_dirs: vec![],
            sizes_kb: vec![19.0, 20.0, 21.0, 28.0, 29.0],
            description: "Armor Hotswap мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("ChestLocator".to_string(), CheatInfo {
            directories: vec!["com/github/hexomod/chestlocator".to_string()],
            classes: vec!["Z.class".to_string(), "z.class".to_string(), "Y.class".to_string(), "y.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![870.0],
            description: "Chest Locator мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("TopkaAutoBuyV1".to_string(), CheatInfo {
            directories: vec!["topka/product".to_string()],
            classes: vec![],
            exclude_dirs: vec![],
            sizes_kb: vec![48.0],
            description: "Topka AutoBuy v1 (бан за хранение)".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("NoHurtCam DanilSimX.jar".to_string(), CheatInfo {
            directories: vec!["nohurtcam/".to_string()],
            classes: vec!["ML.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![95.0],
            description: "NoHurtCam DanilSimX.jar хитбоксы".to_string(),
            strict_mode: false,
            min_conditions: 1,
        });

        database.insert("GUMBALLOFFMODE".to_string(), CheatInfo {
            directories: vec!["com/moandjiezana/toml".to_string()],
            classes: vec!["WriterContext.class".to_string()],
            exclude_dirs: vec!["org/apache/".to_string(),
                               "com/google/".to_string(),
                               "io/netty/".to_string(),
                               "net/minecraft/".to_string(),
                               "net/minecraftforge/".to_string(),
                               "optifine/".to_string(),
                               "javax/".to_string(),],
            sizes_kb: vec![2701.0],
            description: "GUMBALLOFFMODE мод".to_string(),
            strict_mode: true,
            min_conditions: 2,
        });

        database.insert("LibrarianTradeFinder".to_string(), CheatInfo {
            directories: vec!["de/greenman999".to_string()],
            classes: vec!["LibrarianTrade.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![94.0, 100.0, 101.0, 3203.0],
            description: "Librarian Trade Finder мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("AutoAttack".to_string(), CheatInfo {
            directories: vec!["com/tfar/autoattack".to_string(), "vin35/autoattack".to_string()],
            classes: vec!["AutoAttack.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![4.0, 77.0],
            description: "Auto Attack мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("EntityOutliner".to_string(), CheatInfo {
            directories: vec!["net/entityoutliner".to_string()],
            classes: vec!["EntityOutliner.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![32.0, 33.0, 39.0, 41.0],
            description: "Entity Outliner мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("CameraUtils".to_string(), CheatInfo {
            directories: vec!["de/maxhenkel/camerautils".to_string()],
            classes: vec!["CameraUtils.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![88.0, 296.0, 317.0, 344.0, 348.0],
            description: "Camera Utils мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("WallJumpTXF".to_string(), CheatInfo {
            directories: vec!["com/jahirtrap/walljump".to_string(), "genandnic/walljump".to_string()],
            classes: vec!["WallJump.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![155.0, 159.0, 160.0, 161.0, 162.0, 163.0, 165.0],
            description: "Wall-Jump TXF мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("CrystalOptimizer".to_string(), CheatInfo {
            directories: vec!["com/marlowcrystal/marlowcrystal".to_string()],
            classes: vec!["MarlowCrystal.class".to_string(), "CrystalOptimizer.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![90.0, 97.0],
            description: "Crystal Optimizer мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("SoupAPI".to_string(), CheatInfo {
            directories: vec!["org/ChSP/soupapi".to_string()],
            classes: vec!["SoupApi.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![942.0],
            description: "Soup API (бан за хранение)".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("MeteorClient".to_string(), CheatInfo {
            directories: vec![],
            classes: vec![],
            exclude_dirs: vec![],
            sizes_kb: vec![],
            description: "Meteor Client".to_string(),
            strict_mode: false,
            min_conditions: 1,
        });

        database.insert("ClickCrystals".to_string(), CheatInfo {
            directories: vec!["io/github/itzispyder/clickcrystals".to_string()],
            classes: vec!["ClickCrystals.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![2867.2, 4096.0],
            description: "ClickCrystals мод".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("Ezhitboxes".to_string(), CheatInfo {
            directories: vec!["me/bushroot/hb/Modules".to_string()],
            classes: vec!["Hitbox.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![9.0, 10.0, 12.0, 11.0, 13.0, 14.0, 15.0],
            description: "Ezhitboxes хитбокс".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

        database.insert("PseudoNeat".to_string(), CheatInfo {
            directories: vec!["me/bushroot/hb/Modules".to_string()],
            classes: vec!["Hitbox.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![10.0, 17.0, 18.0, 19.0, 23.0, 21.0, 27.0, 28.0, 29.0, 33.0, 34.0, 38.0, 37.0, 71.0],
            description: "PseudoNeat хитбокс".to_string(),
            strict_mode: false,
            min_conditions: 2,
        });

    }
}