# Runbook: Dead Letter Queue (DLQ) Monitoring and Alerting

## Overview
This runbook covers procedures to follow when DLQ alerts are triggered.

## Alerts

### 1. High Dropped Message Rate
**Condition**: `dlq.metrics.total_dropped` increases by more than 100 within a 5-minute window.
**Impact**: Messages are being lost because the queue is full, possibly due to a stuck downstream processor or unusually high failure rates.
**Actions**:
1. Check the error reasons for the failed messages.
2. Verify if downstream processing nodes are healthy.
3. Temporarily increase the queue capacity if necessary and safe.

### 2. High Enqueue Rate
**Condition**: Spike in `total_enqueued`.
**Impact**: Elevated message processing failures.
**Actions**:
1. Identify the service producing the failed messages.
2. Check network stability and dependency health.

## Dashboards
- DLQ processing rate
- DLQ depth
- Drop rate over time
