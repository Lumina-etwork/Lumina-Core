use alloc::vec::Vec;
use alloc::string::String;

#[derive(Debug, PartialEq, Clone)]
pub struct Migration {
    pub version: u64,
    pub description: String,
    pub executed: bool,
}

pub struct MigrationManager {
    migrations: Vec<Migration>,
    current_version: u64,
}

impl MigrationManager {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
            current_version: 0,
        }
    }

    pub fn add_migration(&mut self, version: u64, description: &str) -> Result<(), &'static str> {
        if self.migrations.iter().any(|m| m.version == version) {
            return Err("Migration version already exists");
        }
        self.migrations.push(Migration {
            version,
            description: String::from(description),
            executed: false,
        });
        self.migrations.sort_by_key(|m| m.version);
        Ok(())
    }

    pub fn apply_up_to(&mut self, target_version: u64) -> Result<(), &'static str> {
        for migration in self.migrations.iter_mut() {
            if migration.version <= target_version && !migration.executed {
                migration.executed = true;
                self.current_version = migration.version;
            }
        }
        Ok(())
    }

    pub fn rollback_to(&mut self, target_version: u64) -> Result<(), &'static str> {
        for migration in self.migrations.iter_mut().rev() {
            if migration.version > target_version && migration.executed {
                migration.executed = false;
                self.current_version = target_version;
            }
        }
        Ok(())
    }

    pub fn get_current_version(&self) -> u64 {
        self.current_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_apply_migration() {
        let mut manager = MigrationManager::new();
        assert!(manager.add_migration(1, "Init DB").is_ok());
        assert!(manager.add_migration(2, "Add Users").is_ok());
        
        assert_eq!(manager.get_current_version(), 0);
        
        assert!(manager.apply_up_to(2).is_ok());
        assert_eq!(manager.get_current_version(), 2);
    }

    #[test]
    fn test_rollback_migration() {
        let mut manager = MigrationManager::new();
        let _ = manager.add_migration(1, "Init DB");
        let _ = manager.add_migration(2, "Add Users");
        let _ = manager.apply_up_to(2);
        
        assert!(manager.rollback_to(1).is_ok());
        assert_eq!(manager.get_current_version(), 1);
        
        assert!(!manager.migrations[1].executed);
        assert!(manager.migrations[0].executed);
    }
}
