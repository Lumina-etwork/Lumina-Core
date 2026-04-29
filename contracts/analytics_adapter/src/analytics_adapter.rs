use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, Env, Vec,
    xdr::{ ScErrorCode, ScErrorType},
};

pub const MAX_PAGE_SIZE: u32 = 200;
pub const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;

// Error codes — contract-domain errors use ScErrorType::Contract
// Code 1 = QueryTooLarge, Code 2 = DataCorrupted, Code 3 = InvalidCursor
fn err_query_too_large(env: &Env) -> ! {
    env.panic_with_error(soroban_sdk::Error::from_type_and_code(ScErrorType::Contract, ScErrorCode::ArithDomain))
}

fn err_invalid_cursor(env: &Env) -> ! {
    env.panic_with_error(soroban_sdk::Error::from_type_and_code(ScErrorType::Contract, ScErrorCode::IndexBounds))
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    GlobalTvl,
    TotalVestedHistorically,
    VestingSchedules,
    VestingEntry(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct VestingSchedule {
    pub owner: Address,
    pub total_amount: i128,
    pub released_amount: i128,
    pub start_time: u64,
    pub duration_secs: u64,
    pub cliff_secs: u64,
}

impl VestingSchedule {
    pub fn locked(&self) -> i128 {
        self.total_amount.saturating_sub(self.released_amount)
    }
    pub fn emission_rate_per_sec(&self) -> i128 {
        if self.duration_secs == 0 { return 0; }
        self.total_amount / self.duration_secs as i128
    }
}

#[contracttype]
#[derive(Clone)]
pub struct TvlResult {
    pub tvl_raw: i128,
    pub schedules_counted: u32,
    pub next_cursor: u32,
    pub has_next: bool,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct HistoricalVestResult {
    pub total_vested: i128,
    pub completed_schedules: u32,
    pub next_cursor: u32,
    pub has_next: bool,
    pub ledger_sequence: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct ProjectedEmissionsResult {
    pub projected_emissions: i128,
    pub active_schedules: u32,
    pub next_cursor: u32,
    pub has_next: bool,
    pub ledger_sequence: u32,
}

#[contract]
pub struct AnalyticsAdapter;

#[contractimpl]
impl AnalyticsAdapter {
    pub fn get_global_tvl(env: Env, cursor: u32, page_size: u32) -> TvlResult {
        Self::assert_page_size(&env, page_size);
        let schedules = Self::load_schedules(&env);
        let total = schedules.len();
        if cursor > total { err_invalid_cursor(&env); }
        let end = (cursor + page_size).min(total);
        let mut tvl_raw: i128 = 0;
        let mut schedules_counted: u32 = 0;
        for i in cursor..end {
            if let Some(s) = schedules.get(i) {
                tvl_raw = tvl_raw.saturating_add(s.locked());
                schedules_counted += 1;
            }
        }
        let has_next = end < total;
        TvlResult {
            tvl_raw,
            schedules_counted,
            next_cursor: if has_next { end } else { 0 },
            has_next,
            ledger_sequence: env.ledger().sequence(),
        }
    }

    pub fn get_total_vested_historically(env: Env, cursor: u32, page_size: u32) -> HistoricalVestResult {
        Self::assert_page_size(&env, page_size);
        let schedules = Self::load_schedules(&env);
        let total = schedules.len();
        if cursor > total { err_invalid_cursor(&env); }
        let end = (cursor + page_size).min(total);
        let mut total_vested: i128 = 0;
        let mut completed_schedules: u32 = 0;
        for i in cursor..end {
            if let Some(s) = schedules.get(i) {
                if s.released_amount > 0 {
                    total_vested = total_vested.saturating_add(s.released_amount);
                }
                if s.locked() == 0 && s.total_amount > 0 {
                    completed_schedules += 1;
                }
            }
        }
        let has_next = end < total;
        HistoricalVestResult {
            total_vested,
            completed_schedules,
            next_cursor: if has_next { end } else { 0 },
            has_next,
            ledger_sequence: env.ledger().sequence(),
        }
    }

    pub fn get_projected_emissions_30d(env: Env, cursor: u32, page_size: u32) -> ProjectedEmissionsResult {
        Self::assert_page_size(&env, page_size);
        let schedules = Self::load_schedules(&env);
        let total = schedules.len();
        if cursor > total { err_invalid_cursor(&env); }
        let now = env.ledger().timestamp();
        let end = (cursor + page_size).min(total);
        let mut projected_emissions: i128 = 0;
        let mut active_schedules: u32 = 0;
        for i in cursor..end {
            if let Some(s) = schedules.get(i) {
                let schedule_end = s.start_time.saturating_add(s.duration_secs);
                if now >= schedule_end || s.locked() == 0 { continue; }
                let cliff_end = s.start_time.saturating_add(s.cliff_secs);
                if now < cliff_end {
                    let window_start = cliff_end;
                    let window_end = window_start.saturating_add(THIRTY_DAYS_SECS).min(schedule_end);
                    if window_end <= window_start { continue; }
                    let window_secs = (window_end - window_start) as i128;
                    projected_emissions = projected_emissions.saturating_add(
                        s.emission_rate_per_sec().saturating_mul(window_secs)
                    );
                } else {
                    let remaining = schedule_end.saturating_sub(now);
                    let window_secs = remaining.min(THIRTY_DAYS_SECS) as i128;
                    projected_emissions = projected_emissions.saturating_add(
                        s.emission_rate_per_sec().saturating_mul(window_secs)
                    );
                }
                active_schedules += 1;
            }
        }
        let has_next = end < total;
        ProjectedEmissionsResult {
            projected_emissions,
            active_schedules,
            next_cursor: if has_next { end } else { 0 },
            has_next,
            ledger_sequence: env.ledger().sequence(),
        }
    }

    fn assert_page_size(env: &Env, page_size: u32) {
        if page_size == 0 || page_size > MAX_PAGE_SIZE {
            err_query_too_large(env);
        }
    }

    fn load_schedules(env: &Env) -> Vec<VestingSchedule> {
        env.storage()
            .persistent()
            .get::<DataKey, Vec<VestingSchedule>>(&DataKey::VestingSchedules)
            .unwrap_or_else(|| Vec::new(env))
    }
}
