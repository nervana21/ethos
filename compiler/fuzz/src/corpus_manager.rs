//! Corpus Management for Differential Fuzzing
//!
//! This module handles corpus promotion, minimization, and stable corpus management
//! for differential fuzzing. It implements the logic to promote test cases that
//! all adapters agree on to a stable corpus, and minimize crash cases.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use serde_json::{json, Value};
use thiserror::Error;

use ethos_analysis::differential::DifferentialResult;
use fuzz_types::FuzzCase;

/// Errors that can occur during corpus management
#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Corpus directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("Minimization failed: {0}")]
    MinimizationFailed(String),
}

/// Corpus manager for handling test case promotion and minimization
pub struct CorpusManager {
    /// Base directory for corpus data
    base_dir: PathBuf,
    /// Stable corpus directory
    stable_dir: PathBuf,
    /// Divergences directory
    divergences_dir: PathBuf,
    /// Crashes directory
    crashes_dir: PathBuf,
}

impl CorpusManager {
    /// Create a new corpus manager
    pub fn new(base_dir: impl AsRef<Path>) -> Result<Self, CorpusError> {
        let base_dir = base_dir.as_ref().to_path_buf();
        let stable_dir = base_dir.join("stable");
        let divergences_dir = base_dir.join("divergences");
        let crashes_dir = base_dir.join("crashes");

        // Create directories if they don't exist
        fs::create_dir_all(&stable_dir)?;
        fs::create_dir_all(&divergences_dir)?;
        fs::create_dir_all(&crashes_dir)?;

        Ok(Self {
            base_dir,
            stable_dir,
            divergences_dir,
            crashes_dir,
        })
    }

    /// Create corpus manager from environment variables
    pub fn from_env() -> Result<Self, CorpusError> {
        let base_dir = std::env::var("ARTIFACT_DIR")
            .unwrap_or_else(|_| "/corpus_data".to_string());
        Self::new(base_dir)
    }

    /// Process a differential result and handle corpus promotion/minimization
    pub fn process_result(&self, result: &DifferentialResult) -> Result<(), CorpusError> {
        if result.equivalent {
            // All adapters agree - promote to stable corpus
            self.promote_to_stable_corpus(result)?;
        } else {
            // Found divergences - record and minimize
            self.record_divergence(result)?;
            self.minimize_crash_case(result)?;
        }

        Ok(())
    }

    /// Promote a test case to the stable corpus
    fn promote_to_stable_corpus(&self, result: &DifferentialResult) -> Result<(), CorpusError> {
        let case_id = self.generate_case_id(&result.fuzz_case);
        let stable_path = self.stable_dir.join(format!("{}.json", case_id));

        let stable_record = json!({
            "case_id": case_id,
            "fuzz_case": result.fuzz_case,
            "adapter_results": result.adapter_results,
            "promoted_at": chrono::Utc::now().to_rfc3339(),
            "equivalent": true,
            "summary": result.summary
        });

        fs::write(&stable_path, serde_json::to_string_pretty(&stable_record)?)?;

        println!("✓ Promoted test case {} to stable corpus", case_id);
        Ok(())
    }

    /// Record a divergence between adapters
    fn record_divergence(&self, result: &DifferentialResult) -> Result<(), CorpusError> {
        let case_id = self.generate_case_id(&result.fuzz_case);
        let divergence_path = self.divergences_dir.join(format!("{}.json", case_id));

        let divergence_record = json!({
            "case_id": case_id,
            "fuzz_case": result.fuzz_case,
            "adapter_results": result.adapter_results,
            "differences": result.differences,
            "equivalent": false,
            "summary": result.summary,
            "recorded_at": chrono::Utc::now().to_rfc3339()
        });

        fs::write(&divergence_path, serde_json::to_string_pretty(&divergence_record)?)?;

        println!("⚠️  Recorded divergence for test case {}", case_id);
        Ok(())
    }

    /// Minimize a crash case to create a minimal reproducer
    fn minimize_crash_case(&self, result: &DifferentialResult) -> Result<(), CorpusError> {
        let case_id = self.generate_case_id(&result.fuzz_case);
        let crash_path = self.crashes_dir.join(format!("{}.json", case_id));

        // Create a minimized version of the test case
        let minimized_case = self.create_minimized_case(&result.fuzz_case);

        let crash_record = json!({
            "case_id": case_id,
            "original_case": result.fuzz_case,
            "minimized_case": minimized_case,
            "adapter_results": result.adapter_results,
            "differences": result.differences,
            "minimized_at": chrono::Utc::now().to_rfc3339()
        });

        fs::write(&crash_path, serde_json::to_string_pretty(&crash_record)?)?;

        println!("🔍 Minimized crash case {}", case_id);
        Ok(())
    }

    /// Create a minimized version of a test case
    fn create_minimized_case(&self, case: &FuzzCase) -> FuzzCase {
        let mut minimized_params = HashMap::new();

        // Keep only essential parameters
        for (key, value) in &case.parameters {
            if self.is_essential_parameter(key, value) {
                minimized_params.insert(key.clone(), value.clone());
            }
        }

        FuzzCase {
            method_name: case.method_name.clone(),
            parameters: minimized_params,
            expected_result_type: case.expected_result_type.clone(),
        }
    }

    /// Check if a parameter is essential for the test case
    fn is_essential_parameter(&self, key: &str, value: &Value) -> bool {
        match key {
            "amount_msat" | "description" | "peer_id" | "id" => true,
            _ => {
                // For other parameters, keep them if they're not empty/default
                match value {
                    Value::String(s) => !s.is_empty() && s != "test",
                    Value::Number(n) => n.as_u64().unwrap_or(0) > 0,
                    _ => true,
                }
            }
        }
    }

    /// Generate a unique case ID
    fn generate_case_id(&self, case: &FuzzCase) -> String {
        use sha2::{Sha256, Digest};

        let case_data = format!("{}{:?}", case.method_name, case.parameters);
        let mut hasher = Sha256::new();
        hasher.update(case_data.as_bytes());
        let hash = hasher.finalize();
        format!("{:x}", hash)[..16].to_string()
    }

    /// Get statistics about the corpus
    pub fn get_corpus_stats(&self) -> Result<CorpusStats, CorpusError> {
        let stable_count = self.count_files_in_dir(&self.stable_dir)?;
        let divergence_count = self.count_files_in_dir(&self.divergences_dir)?;
        let crash_count = self.count_files_in_dir(&self.crashes_dir)?;

        Ok(CorpusStats {
            stable_cases: stable_count,
            divergences: divergence_count,
            crashes: crash_count,
            total_cases: stable_count + divergence_count + crash_count,
        })
    }

    /// Count files in a directory
    fn count_files_in_dir(&self, dir: &Path) -> Result<usize, CorpusError> {
        if !dir.exists() {
            return Ok(0);
        }

        let entries = fs::read_dir(dir)?;
        let count = entries.filter_map(|entry| {
            entry.ok().and_then(|e| {
                e.path().extension().and_then(|ext| {
                    if ext == "json" { Some(()) } else { None }
                })
            })
        }).count();

        Ok(count)
    }

    /// Clean up old corpus files older than specified days
    pub fn cleanup_old_files(&self, days: u64) -> Result<usize, CorpusError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let mut cleaned = 0;

        for dir in &[&self.stable_dir, &self.divergences_dir, &self.crashes_dir] {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if let Ok(metadata) = path.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                let modified_time = chrono::DateTime::<chrono::Utc>::from(modified);
                                if modified_time < cutoff {
                                    if let Ok(_) = fs::remove_file(&path) {
                                        cleaned += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(cleaned)
    }
}

/// Statistics about the corpus
#[derive(Debug, Clone)]
pub struct CorpusStats {
    pub stable_cases: usize,
    pub divergences: usize,
    pub crashes: usize,
    pub total_cases: usize,
}

impl CorpusStats {
    /// Print a summary of corpus statistics
    pub fn print_summary(&self) {
        println!("=== Corpus Statistics ===");
        println!("Stable cases: {}", self.stable_cases);
        println!("Divergences: {}", self.divergences);
        println!("Crashes: {}", self.crashes);
        println!("Total cases: {}", self.total_cases);
        println!("========================");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    #[test]
    fn test_corpus_manager_creation() {
        let temp_dir = std::env::temp_dir().join("test_corpus");
        let manager = CorpusManager::new(&temp_dir).unwrap();

        assert!(manager.stable_dir.exists());
        assert!(manager.divergences_dir.exists());
        assert!(manager.crashes_dir.exists());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_case_id_generation() {
        let temp_dir = std::env::temp_dir().join("test_corpus");
        let manager = CorpusManager::new(&temp_dir).unwrap();

        let case1 = FuzzCase {
            method_name: "getinfo".to_string(),
            parameters: HashMap::new(),
            expected_result_type: None,
        };

        let case2 = FuzzCase {
            method_name: "getinfo".to_string(),
            parameters: HashMap::new(),
            expected_result_type: None,
        };

        let id1 = manager.generate_case_id(&case1);
        let id2 = manager.generate_case_id(&case2);

        assert_eq!(id1, id2); // Same case should have same ID
        assert_eq!(id1.len(), 16); // Should be 16 characters

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
