# Kubernetes deployment (self-hosted: minikube / kind)

Runs the exact same container images as Docker Compose — no code changes.
Verified on minikube v1.38 (k8s v1.35, docker driver).

## Run it

```bash
# 1. cluster
minikube start --driver=docker --memory=4096 --cpus=2   # or: kind create cluster

# 2. images — either load your locally-built images (no registry needed):
minikube image load inferwatch-backend:latest inferwatch-frontend:latest postgres:16-alpine
#    ...or build them straight into minikube's docker:
#    eval $(minikube docker-env) && docker compose build

# 3. secret (never committed) — gemini key, or openai key, or both
kubectl create namespace inferwatch
kubectl -n inferwatch create secret generic llm-secrets \
  --from-literal=gemini-api-key=YOUR_KEY          # required if LLM_PROVIDER=gemini
# --from-literal=openai-api-key=YOUR_KEY          # required if LLM_PROVIDER=openai

# 4. apply
kubectl apply -f k8s/inferwatch.yaml

# 5. expose (frontend calls backend at localhost:3001 — identical to compose)
kubectl -n inferwatch port-forward svc/backend 3001:3001 &
kubectl -n inferwatch port-forward svc/frontend 5173:5173 &
# → http://localhost:5173
```

Teardown: `minikube delete` (or `kubectl delete namespace inferwatch`).

## What's inside `inferwatch.yaml`

| Resource | Role |
|---|---|
| `Namespace inferwatch` | isolation |
| `ConfigMap backend-config` | provider/model selection, `DATABASE_URL` pointing at the `postgres` Service |
| `Deployment postgres` + `PVC` + `Service` | persistent Postgres (minikube's default StorageClass provisions the PV) |
| `Deployment backend` + `Service` | initContainer waits for `pg_isready`, then the binary runs **SQLx migrations on startup** — no job needed; liveness probe on `/health` |
| `Deployment frontend` + `Service` | adapter-node SSR server |
| `Secret llm-secrets` (created by you, step 3) | provider API keys, injected via `secretKeyRef` (`optional: true` so a single-provider deployment needs only its own key) |

## Notes / honest limitations

- **Image arch**: images are arch-specific — `minikube image load` works on the machine that built them; on a different arch (e.g. amd64 CI), rebuild with `eval $(minikube docker-env)` first.
- Postgres runs as a single replica with `Recreate` strategy — fine for a demo; production would be a StatefulSet or managed DB.
- `imagePullPolicy: IfNotPresent` everywhere + a real registry push is the production path (the demo never needs Docker Hub).
- Scaling the backend >1 replica works as-is (stateless); the single ingestion worker per pod stays independent per pod — that's the "scaling notes" story in ARCHITECTURE.md (shared durable queue when a second consumer appears).
