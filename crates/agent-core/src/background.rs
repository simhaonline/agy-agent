use common::error::CommonError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    pub name: String,
    pub command: String,
    pub status: TaskStatus,
    pub started_at: String,
    pub pid: Option<u32>,
}

pub struct BackgroundTaskManager {
    pub store_path: PathBuf,
}

impl BackgroundTaskManager {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let store_path = workspace_root.as_ref().join(".agy").join("tasks.json");
        BackgroundTaskManager { store_path }
    }

    pub fn list_tasks(&self) -> Vec<BackgroundTask> {
        if self.store_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.store_path) {
                if let Ok(tasks) = serde_json::from_str::<Vec<BackgroundTask>>(&content) {
                    return tasks;
                }
            }
        }
        Vec::new()
    }

    pub fn save_tasks(&self, tasks: &[BackgroundTask]) -> Result<(), CommonError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = File::create(&self.store_path)?;
        let json = serde_json::to_string_pretty(tasks)?;
        f.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn spawn_task(&self, name: &str, command: &str) -> Result<BackgroundTask, CommonError> {
        let mut tasks = self.list_tasks();
        let id = uuid::Uuid::new_v4().to_string();
        let task = BackgroundTask {
            id,
            name: name.to_string(),
            command: command.to_string(),
            status: TaskStatus::Running,
            started_at: chrono::Utc::now().to_rfc3339(),
            pid: Some(std::process::id()),
        };
        tasks.push(task.clone());
        self.save_tasks(&tasks)?;
        Ok(task)
    }

    pub fn stop_task(&self, id: &str) -> Result<bool, CommonError> {
        let mut tasks = self.list_tasks();
        let mut found = false;
        for t in &mut tasks {
            if t.id == id {
                t.status = TaskStatus::Stopped;
                found = true;
            }
        }
        if found {
            self.save_tasks(&tasks)?;
        }
        Ok(found)
    }
}
