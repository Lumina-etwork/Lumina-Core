use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// PagerDuty Events API action for an incident automation event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagerDutyAction {
    Trigger,
    Acknowledge,
    Resolve,
}

impl PagerDutyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trigger => "trigger",
            Self::Acknowledge => "acknowledge",
            Self::Resolve => "resolve",
        }
    }
}

/// Incident severity aligned with PagerDuty and SRE escalation policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IncidentSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl IncidentSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

/// Service SLO context used by automation to enrich incidents and dashboards.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSlo {
    pub service: String,
    pub p99_latency_ms: u32,
    pub availability_bps: u32,
}

impl ServiceSlo {
    pub fn new(service: impl Into<String>, p99_latency_ms: u32, availability_bps: u32) -> Self {
        Self {
            service: service.into(),
            p99_latency_ms,
            availability_bps,
        }
    }
}

/// Runbook metadata that is attached to every incident event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunbookStep {
    pub title: String,
    pub command: String,
}

impl RunbookStep {
    pub fn new(title: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            command: command.into(),
        }
    }
}

/// PagerDuty Events API v2 payload assembled by the runbook automation layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagerDutyEvent {
    pub routing_key: String,
    pub event_action: PagerDutyAction,
    pub dedup_key: String,
    pub summary: String,
    pub source: String,
    pub severity: IncidentSeverity,
    pub component: String,
    pub group: String,
    pub custom_details: Vec<(String, String)>,
}

impl PagerDutyEvent {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.routing_key.trim().is_empty() {
            return Err("routing key is required");
        }
        if self.dedup_key.trim().is_empty() {
            return Err("dedup key is required");
        }
        if self.summary.trim().is_empty() {
            return Err("summary is required");
        }
        if self.source.trim().is_empty() {
            return Err("source is required");
        }
        Ok(())
    }

    /// Render a compact JSON request body without logging or exposing secrets.
    pub fn to_json_body(&self) -> Result<String, &'static str> {
        self.validate()?;
        let mut details = String::from("{");
        for (idx, (key, value)) in self.custom_details.iter().enumerate() {
            if idx > 0 {
                details.push(',');
            }
            details.push_str(&format!(
                "\"{}\":\"{}\"",
                escape_json(key),
                escape_json(value)
            ));
        }
        details.push('}');

        Ok(format!(
            "{{\"routing_key\":\"{}\",\"event_action\":\"{}\",\"dedup_key\":\"{}\",\"payload\":{{\"summary\":\"{}\",\"source\":\"{}\",\"severity\":\"{}\",\"component\":\"{}\",\"group\":\"{}\",\"custom_details\":{}}}}}",
            escape_json(&self.routing_key),
            self.event_action.as_str(),
            escape_json(&self.dedup_key),
            escape_json(&self.summary),
            escape_json(&self.source),
            self.severity.as_str(),
            escape_json(&self.component),
            escape_json(&self.group),
            details
        ))
    }
}

/// Creates runbook-driven PagerDuty events for all Lumina services.
pub struct IncidentRunbookAutomation {
    routing_key: String,
    runbook_url: String,
    deployment_strategy: String,
}

impl IncidentRunbookAutomation {
    pub fn new(routing_key: impl Into<String>, runbook_url: impl Into<String>) -> Self {
        Self {
            routing_key: routing_key.into(),
            runbook_url: runbook_url.into(),
            deployment_strategy: "blue-green with 5%/25%/50%/100% canary analysis".to_string(),
        }
    }

    pub fn trigger(
        &self,
        slo: &ServiceSlo,
        observed_p99_ms: u32,
        observed_availability_bps: u32,
        steps: &[RunbookStep],
    ) -> PagerDutyEvent {
        let severity = classify_severity(slo, observed_p99_ms, observed_availability_bps);
        let mut custom_details = Vec::new();
        custom_details.push(("runbook_url".to_string(), self.runbook_url.clone()));
        custom_details.push((
            "deployment_strategy".to_string(),
            self.deployment_strategy.clone(),
        ));
        custom_details.push(("latency_p99_ms".to_string(), observed_p99_ms.to_string()));
        custom_details.push((
            "availability_bps".to_string(),
            observed_availability_bps.to_string(),
        ));
        for (idx, step) in steps.iter().enumerate() {
            custom_details.push((
                format!("runbook_step_{}", idx + 1),
                format!("{}: {}", step.title, step.command),
            ));
        }

        PagerDutyEvent {
            routing_key: self.routing_key.clone(),
            event_action: PagerDutyAction::Trigger,
            dedup_key: format!("lumina:{}:{}", slo.service, severity.as_str()),
            summary: format!(
                "{} SLO breach: p99={}ms availability={}bps",
                slo.service, observed_p99_ms, observed_availability_bps
            ),
            source: "lumina-core-incident-automation".to_string(),
            severity,
            component: slo.service.clone(),
            group: "lumina-core".to_string(),
            custom_details,
        }
    }
}

pub fn classify_severity(
    slo: &ServiceSlo,
    observed_p99_ms: u32,
    observed_availability_bps: u32,
) -> IncidentSeverity {
    if observed_availability_bps.saturating_add(25) < slo.availability_bps
        || observed_p99_ms >= slo.p99_latency_ms.saturating_mul(4)
    {
        IncidentSeverity::Critical
    } else if observed_availability_bps < slo.availability_bps
        || observed_p99_ms >= slo.p99_latency_ms.saturating_mul(2)
    {
        IncidentSeverity::Error
    } else if observed_p99_ms > slo.p99_latency_ms {
        IncidentSeverity::Warning
    } else {
        IncidentSeverity::Info
    }
}

fn escape_json(input: &str) -> String {
    let mut escaped = String::new();
    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_latency_and_availability_breaches() {
        let slo = ServiceSlo::new("consensus", 100, 9_999);
        assert_eq!(classify_severity(&slo, 99, 9_999), IncidentSeverity::Info);
        assert_eq!(
            classify_severity(&slo, 101, 9_999),
            IncidentSeverity::Warning
        );
        assert_eq!(classify_severity(&slo, 250, 9_999), IncidentSeverity::Error);
        assert_eq!(
            classify_severity(&slo, 401, 9_999),
            IncidentSeverity::Critical
        );
        assert_eq!(classify_severity(&slo, 80, 9_998), IncidentSeverity::Error);
        assert_eq!(
            classify_severity(&slo, 80, 9_900),
            IncidentSeverity::Critical
        );
    }

    #[test]
    fn builds_valid_pagerduty_payload_with_runbook_details() {
        let automation =
            IncidentRunbookAutomation::new("route-key", "https://runbooks.example/incidents");
        let event = automation.trigger(
            &ServiceSlo::new("relay", 100, 9_999),
            275,
            9_997,
            &[RunbookStep::new(
                "Check relay health",
                "scripts/fire_drill.sh relay",
            )],
        );
        let body = event.to_json_body().expect("payload should validate");

        assert_eq!(event.severity, IncidentSeverity::Error);
        assert!(body.contains("\"event_action\":\"trigger\""));
        assert!(body.contains("\"dedup_key\":\"lumina:relay:error\""));
        assert!(body.contains("runbook_step_1"));
        assert!(body.contains("blue-green with 5%/25%/50%/100% canary analysis"));
    }

    #[test]
    fn rejects_empty_routing_key() {
        let event = PagerDutyEvent {
            routing_key: "".to_string(),
            event_action: PagerDutyAction::Trigger,
            dedup_key: "dedup".to_string(),
            summary: "summary".to_string(),
            source: "source".to_string(),
            severity: IncidentSeverity::Critical,
            component: "component".to_string(),
            group: "group".to_string(),
            custom_details: Vec::new(),
        };

        assert_eq!(event.to_json_body(), Err("routing key is required"));
    }
}
