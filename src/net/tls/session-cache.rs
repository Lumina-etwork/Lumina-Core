use std::collections::HashMap;
use std::time::{Duration, Instant};

const MAX_CACHE_ENTRIES: usize = 1024;
const DEFAULT_TTL_SECS: u64 = 300;
const MAX_SESSIONS_PER_TENANT: usize = 16;

#[derive(Clone, Debug)]
pub struct SessionEntry {
    pub compound_key: String,
    pub tenant_id: String,
    pub original_sni: String,
    pub session_data: Vec<u8>,
    pub created_at: Instant,
    pub ttl_secs: u64,
}

pub struct SessionCache {
    entries: HashMap<String, SessionEntry>,
    tenant_session_count: HashMap<String, usize>,
    pub collision_attempts: u64,
}

impl SessionCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), tenant_session_count: HashMap::new(), collision_attempts: 0 }
    }
    pub fn compound_key(tenant_id: &str, original_sni: &str) -> String {
        format!("{}:{}", tenant_id, original_sni)
    }
    pub fn store(&mut self, tenant_id: &str, original_sni: &str, session_data: Vec<u8>, ttl_secs: Option<u64>) -> Result<(), CacheError> {
        let count = self.tenant_session_count.get(tenant_id).unwrap_or(&0);
        if *count >= MAX_SESSIONS_PER_TENANT { return Err(CacheError::TenantLimitExceeded); }
        let compound = Self::compound_key(tenant_id, original_sni);
        if self.entries.len() >= MAX_CACHE_ENTRIES && !self.entries.contains_key(&compound) { self.evict_oldest(); }
        self.entries.insert(compound.clone(), SessionEntry {
            compound_key: compound, tenant_id: tenant_id.to_string(), original_sni: original_sni.to_string(),
            session_data, created_at: Instant::now(), ttl_secs: ttl_secs.unwrap_or(DEFAULT_TTL_SECS),
        });
        *self.tenant_session_count.entry(tenant_id.to_string()).or_insert(0) += 1;
        Ok(())
    }
    pub fn lookup(&mut self, tenant_id: &str, original_sni: &str) -> Option<&SessionEntry> {
        let compound = Self::compound_key(tenant_id, original_sni);
        let entry = self.entries.get(&compound)?;
        if entry.created_at.elapsed() > Duration::from_secs(entry.ttl_secs) { return None; }
        if entry.tenant_id != tenant_id { self.collision_attempts += 1; return None; }
        Some(entry)
    }
    fn evict_oldest(&mut self) {
        let mut oldest_key: Option<String> = None;
        let mut oldest_time = Instant::now();
        for (key, entry) in &self.entries { if entry.created_at < oldest_time { oldest_time = entry.created_at; oldest_key = Some(key.clone()); } }
        if let Some(key) = oldest_key { if let Some(entry) = self.entries.remove(&key) { if let Some(count) = self.tenant_session_count.get_mut(&entry.tenant_id) { *count = count.saturating_sub(1); } } }
    }
}

#[derive(Debug)]
pub enum CacheError { TenantLimitExceeded }
