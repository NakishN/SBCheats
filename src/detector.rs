// detector.rs - Оптимизированное ядро детектора
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

    // КРИТИЧЕСКАЯ ОПТИМИЗАЦИЯ: Читаем только первые 20 файлов вместо 100
    pub fn check_jar_file(&self, jar_path: &Path) -> Option<ThreatResult> {
        let file_size = std::fs::metadata(jar_path).ok()?.len();
        let file_size_kb = file_size as f32 / 1024.0;

        let file = File::open(jar_path).ok()?;
        let reader = BufReader::new(file);
        let mut archive = ZipArchive::new(reader).ok()?;

        let max_files = archive.len().min(20);
        let mut file_list = Vec::with_capacity(max_files);

        for i in 0..max_files {
            if let Ok(file) = archive.by_index(i) {
                file_list.push(file.name().to_lowercase());
            }
        }

        // Быстрая фильтрация по весу
        let mut weight_matches: Vec<&String> = Vec::new();
        for (cheat_name, cheat_info) in &self.database {
            if !cheat_info.sizes_kb.is_empty()
                && self.check_weight_match(file_size_kb, &cheat_info.sizes_kb, 0.05) {
                weight_matches.push(cheat_name);
            }
        }

        let scan_all = weight_matches.is_empty();

        for (cheat_name, cheat_info) in &self.database {
            if !scan_all && !weight_matches.contains(&cheat_name) {
                continue;
            }

            let mut conditions_met = Vec::new();

            if cheat_info.strict_mode
                && self.has_legit_libraries(&file_list, &cheat_info.exclude_dirs) {
                continue;
            }

            if !cheat_info.directories.is_empty() {
                let dir_found = cheat_info.directories.iter().any(|directory| {
                    let dir_lower = directory.to_lowercase();
                    file_list.iter().any(|fp| fp.contains(&dir_lower))
                });
                if dir_found {
                    conditions_met.push("directory");
                }
            }

            if !cheat_info.classes.is_empty() {
                let class_found = cheat_info.classes.iter().any(|class_name| {
                    let class_lower = class_name.to_lowercase();
                    file_list.iter().any(|fp| fp.contains(&class_lower))
                });
                if class_found {
                    conditions_met.push("class");
                }
            }

            if !cheat_info.sizes_kb.is_empty()
                && self.check_weight_match(file_size_kb, &cheat_info.sizes_kb, 0.05) {
                conditions_met.push("weight");
            }

            let is_threat = if cheat_info.strict_mode {
                let has_dir = !cheat_info.directories.is_empty()
                    && conditions_met.contains(&"directory");
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
            exclude_dirs.iter().any(|exclude|
                filepath.to_lowercase().contains(&exclude.to_lowercase())
            )
        })
    }

    fn init_database(database: &mut HashMap<String, CheatInfo>) {
        database.insert("DoomsDay".to_string(), CheatInfo {
            directories: vec!["net/java/".to_string()],
            classes: vec!["i.class".to_string()],
            exclude_dirs: vec!["org/apache/".to_string(), "com/google/".to_string()],
            sizes_kb: vec![],
            description: "DoomsDay чит".to_string(),
            strict_mode: true,
            min_conditions: 3,
        });

        database.insert("Freecam".to_string(), CheatInfo {
            directories: vec!["net/xolt/freecam/".to_string()],
            classes: vec!["freecam.class".to_string()],
            exclude_dirs: vec![],
            sizes_kb: vec![42.0, 74.0, 1047.0, 1048.0, 1069.0, 1104.0, 1122.0],
            description: "Freecam мод".to_string(),
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
    }
}