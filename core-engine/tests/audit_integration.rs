use core_engine::audit::CoreAuditTrail;

#[test]
fn core_engine_audit_trail_records_and_verifies_events() {
    let mut audit = CoreAuditTrail::new();
    audit.record("attestation", "ticket_issued", b"relay=relay1;target=peer1");
    audit.record("network", "endpoint_cache_update", b"peer=peer1;port=3478");

    let root_hash = audit.root_hash();
    assert_ne!(root_hash, [0u8; 32]);
    assert_eq!(audit.verify().unwrap(), root_hash);
    assert_eq!(audit.entries().len(), 2);
}

#[test]
fn core_engine_audit_trail_detects_tampering() {
    let mut audit = CoreAuditTrail::new();
    audit.record("attestation", "ticket_issued", b"relay=relay1;target=peer1");
    audit.record("network", "endpoint_cache_update", b"peer=peer1;port=3478");

    let entries = audit.entries_mut();
    entries[1].action = "endpoint_removed".to_string();

    assert!(matches!(audit.verify(), Err(_)));
}
