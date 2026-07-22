# Runbook: Docker Image Layer Caching para CI

## Descripcion

Este runbook describe la arquitectura, operacion y troubleshooting del sistema de
Docker layer caching implementado en el pipeline CI de Lumina-Core.

El sistema cachea las layers de Docker del proceso de build de Rust/WASM en GitHub
Actions usando el backend `type=gha` con `mode=max`, reduciendo los tiempos de CI
de 25-40 minutos a 3-6 minutos en runs calidos.

Dashboard: `monitoring/grafana/dashboards/ci-cache-dashboard.json`
Alertas: `monitoring/prometheus/alerts/ci-cache-alerts.yml`

---

## Arquitectura

### Stages del Dockerfile

El `Dockerfile` tiene 5 stages encadenadas. Cada una invalida su cache de forma
independiente:

| Stage | Invalida cuando | Tiempo cold | Tiempo warm |
|---|---|---|---|
| `base-toolchain` | `RUST_VERSION` cambia | ~3 min | ~5s |
| `stellar-cli` | `STELLAR_CLI_VERSION` cambia | ~8 min | ~5s |
| `deps-cache` | `Cargo.lock` cambia | ~20 min | ~30s |
| `build` | cualquier fuente cambia | ~5 min | ~2 min |
| `final` | output de `build` cambia | ~30s | ~10s |

### Cache Scopes (GitHub Actions)

Cada job en CI usa un scope independiente para evitar contaminacion cruzada.
El backend es `type=gha,mode=max` — `mode=max` es obligatorio para cachear
las stages intermedias (especialmente `deps-cache`).

| Scope | Job(s) | Invalida cuando |
|---|---|---|
| `lumina-base-toolchain` | lint | `RUST_VERSION` ARG cambia |
| `lumina-core-infra` | core-infra, integration | `Cargo.lock` cambia |
| `lumina-wasm-contracts-a` | contracts-a | source de contratos cambia |
| `lumina-wasm-{contrato}` | build-wasm matrix | source del contrato especifico |
| `lumina-backend-analytics` | backends (analytics) | `analytics/Cargo.lock` |
| `lumina-backend-social` | backends (social) | `social/Cargo.lock` |
| `lumina-release-final` | build-release | cualquier fuente o Cargo.lock |

### Metricas

Las metricas se emiten al Prometheus Pushgateway via `scripts/push-cache-metrics.sh`
al finalizar cada Docker build step en CI. La variable de entorno `PUSHGATEWAY_URL`
debe estar configurada como secret en el repositorio.

Metricas disponibles:
- `ci_docker_cache_hits_total{scope, stage}` — layers servidas desde cache
- `ci_docker_cache_misses_total{scope, stage}` — layers compiladas desde cero
- `ci_docker_build_duration_seconds{scope, stage}` — duracion del build
- `ci_docker_cache_last_push_timestamp_seconds{scope}` — ultimo refresh exitoso

---

## Alertas

### CIDockerCacheHitRateLow (warning)
Hit rate < 80% por 30 minutos.

**Respuesta:**
1. Verificar si `Cargo.lock` tuvo cambios frecuentes en las ultimas horas (`git log --oneline Cargo.lock`).
2. Revisar que los nombres de scope en los workflows no cambiaron.
3. Verificar el uso del cache GHA — si supera 10 GB, GitHub evicta caches automaticamente.

### CIDockerBuildDurationHigh (warning)
P99 de `deps-cache` > 10 minutos por 15 minutos.

**Respuesta:**
1. Es esperado si `Cargo.lock` cambio (full rebuild de deps).
2. Si es recurrente, revisar si Dependabot esta haciendo updates batch.
3. Considerar separar el cache de `stellar-cli` del de las deps de Cargo.

### CIDockerCacheStale (warning)
`lumina-base-toolchain` sin refresh en 24h.

**Respuesta:**
1. Revisar el historial de ejecuciones de CI en GitHub Actions.
2. Si los workflows estan fallando antes del Docker build step, el cache no se actualiza.
3. Disparar `workflow_dispatch` manualmente en `ci.yml` para forzar un refresh.

### CIDockerLayerCacheCriticalMiss (critical)
Build de release corriendo completamente en frio.

**Respuesta:**
1. Investigar inmediatamente — un cold build de release puede demorar 40+ minutos.
2. Verificar que `docker/setup-buildx-action@v3` esta antes de `docker/build-push-action@v6`.
3. Confirmar que los scopes en `cache-from` y `cache-to` son identicos.
4. Revisar si el cache de GHA fue borrado manualmente (Settings > Actions > Caches).

---

## Troubleshooting

### 1. Cache miss en cada run (nunca cachea)

**Causa mas comun:** `DOCKER_BUILDKIT=1` no esta configurado, o `setup-buildx-action`
no se ejecuto antes del build step.

**Verificacion:**
```bash
# En el log de CI, buscar esta linea en el step de Docker build:
# "importing cache manifest from gha"
# Si no aparece, el cache-from no funciono.
```

**Solucion:** Asegurar que el env `DOCKER_BUILDKIT: "1"` esta en el bloque `env:` global
del workflow y que `docker/setup-buildx-action@v3` es el primer step de Docker.

### 2. `deps-cache` tarda > 10 minutos consistentemente

**Causa:** `Cargo.lock` cambia frecuentemente, invalidando la layer mas costosa.

**Verificacion:**
```bash
git log --oneline --follow Cargo.lock | head -20
```

**Solucion:** Agrupar los PRs de dependencias (Dependabot auto-merge batching) para
reducir la frecuencia de cambios en `Cargo.lock`.

### 3. Artefacto WASM no encontrado despues de `docker cp`

**Causa:** El nombre del container puede tener conflicto si multiples jobs corren en
el mismo runner con el mismo SHA.

**Verificacion:** Revisar el output del step "Extract WASM artifacts from image" en el log.

**Solucion:** El step usa `docker create` + `docker rm` de forma idempotente. Si falla,
el step "Build native fallback" en `contracts.yml` compila el WASM directamente sin Docker.

### 4. La stage `stellar-cli` se recompila aunque la version no cambio

**Causa:** El `ARG STELLAR_CLI_VERSION` debe estar declarado dentro del mismo stage
que lo usa para que BuildKit lo incluya en la clave de cache.

**Verificacion:** Confirmar que el `Dockerfile` tiene `ARG STELLAR_CLI_VERSION` justo
antes del `RUN cargo install` en la stage `stellar-cli`.

### 5. Metricas no aparecen en Grafana

**Verificacion:**
```bash
# Testear el script localmente
export PUSHGATEWAY_URL="http://tu-pushgateway:9091"
./scripts/push-cache-metrics.sh '{"buildkit.cache.hit":5,"buildkit.cache.miss":1}' \
  lumina-core-infra deps-cache 120
```

**Causas comunes:**
- `PUSHGATEWAY_URL` no esta configurado como secret en el repositorio.
- El Pushgateway no es accesible desde los runners de GitHub Actions.
- `python3` no esta instalado en el runner (los runners ubuntu-latest lo incluyen).

---

## Estrategia de Deploy (Blue-Green)

El Docker build se integra en el pipeline blue-green existente de `release.yml`:

1. **`build-release`:** Construye la imagen `final` con cache `lumina-release-final`.
   La imagen se guarda como artefacto (`docker-image-{sha}.tar.gz`) con retention 7 dias.
   El digest de la imagen se propaga como output del job.

2. **`deploy-canary`:** Descarga el tarball, hace `docker load`, y despliega al slot
   target con 10% de trafico. El digest es inmutable — garantiza que canary y produccion
   usan exactamente la misma imagen.

3. **`canary-analysis`:** Gate de 5 minutos con P99 < 100ms y error rate < 0.1%.
   El Docker layer caching no afecta la latencia de runtime (solo la de build).

4. **`promote-production`:** Si canary pasa, swap del slot. El digest queda registrado
   en el job summary para trazabilidad de auditoria.

5. **`rollback`:** Si canary falla, el slot activo no cambia. La imagen Docker del
   commit fallido queda en el cache pero no se promociona.

---

## Limites y Consideraciones

- **Limite de cache GHA:** 10 GB por repositorio. Con 7 scopes de ~1-2 GB cada uno,
  el uso total es ~7-14 GB. GitHub evicta caches por LRU cuando se supera el limite.
  Monitorear via la alerta `CIDockerCacheHitRateLow`.

- **Los caches Cargo existentes se mantienen:** Los `actions/cache@v4` sobre
  `~/.cargo/registry` no se eliminaron. Sirven para builds nativos locales y como
  fallback si Docker no esta disponible.

- **Seguridad:** La imagen `final` corre como usuario no-root `lumina`. El `.dockerignore`
  excluye `.env`, `*.pem`, `*.key`. Ningun secret se pasa como `ARG` al Dockerfile.
  Los digests de imagen son content-addressed e inmutables.
