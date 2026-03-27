#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# GCP one-time setup script for opencode-sudoku-rust-app
# Run this ONCE from your local machine after:
#   gcloud auth login
#   gcloud config set project opencode-sudoku-rust-app
#
# After this script completes, add the printed values as GitHub Secrets in:
#   https://github.com/javcasalc/opencode-sudoku-rust-app/settings/secrets/actions
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

PROJECT_ID="opencode-sudoku-rust-app"
REGION="us-central1"
REGISTRY_REPO="sudoku-app"
SERVICE_ACCOUNT_NAME="github-actions-deployer"
SERVICE_ACCOUNT_EMAIL="${SERVICE_ACCOUNT_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"
GITHUB_ORG="javcasalc"
GITHUB_REPO="opencode-sudoku-rust-app"
POOL_NAME="github-actions-pool"
PROVIDER_NAME="github-actions-provider"

echo "==> [1/7] Enabling required GCP APIs..."
gcloud services enable \
  run.googleapis.com \
  artifactregistry.googleapis.com \
  iam.googleapis.com \
  iamcredentials.googleapis.com \
  cloudresourcemanager.googleapis.com \
  --project="${PROJECT_ID}"

echo "==> [2/7] Creating Artifact Registry repository..."
gcloud artifacts repositories create "${REGISTRY_REPO}" \
  --repository-format=docker \
  --location="${REGION}" \
  --description="Sudoku app Docker images" \
  --project="${PROJECT_ID}" || echo "    (already exists, skipping)"

echo "==> [3/7] Creating Service Account for GitHub Actions..."
gcloud iam service-accounts create "${SERVICE_ACCOUNT_NAME}" \
  --display-name="GitHub Actions Deployer" \
  --project="${PROJECT_ID}" || echo "    (already exists, skipping)"

echo "==> [4/7] Granting IAM roles to service account..."
for ROLE in \
  "roles/run.admin" \
  "roles/artifactregistry.writer" \
  "roles/iam.serviceAccountUser" \
  "roles/storage.admin"; do
  gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
    --member="serviceAccount:${SERVICE_ACCOUNT_EMAIL}" \
    --role="${ROLE}" \
    --quiet
done

echo "==> [5/7] Creating Workload Identity Pool..."
gcloud iam workload-identity-pools create "${POOL_NAME}" \
  --location="global" \
  --display-name="GitHub Actions Pool" \
  --project="${PROJECT_ID}" || echo "    (already exists, skipping)"

POOL_ID=$(gcloud iam workload-identity-pools describe "${POOL_NAME}" \
  --location="global" \
  --project="${PROJECT_ID}" \
  --format="value(name)")

echo "==> [6/7] Creating Workload Identity Provider (OIDC for GitHub)..."
gcloud iam workload-identity-pools providers create-oidc "${PROVIDER_NAME}" \
  --location="global" \
  --workload-identity-pool="${POOL_NAME}" \
  --display-name="GitHub OIDC Provider" \
  --issuer-uri="https://token.actions.githubusercontent.com" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" \
  --attribute-condition="assertion.repository=='${GITHUB_ORG}/${GITHUB_REPO}'" \
  --project="${PROJECT_ID}" || echo "    (already exists, skipping)"

PROVIDER_ID=$(gcloud iam workload-identity-pools providers describe "${PROVIDER_NAME}" \
  --location="global" \
  --workload-identity-pool="${POOL_NAME}" \
  --project="${PROJECT_ID}" \
  --format="value(name)")

echo "==> [7/7] Binding service account to Workload Identity Pool..."
gcloud iam service-accounts add-iam-policy-binding "${SERVICE_ACCOUNT_EMAIL}" \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/${POOL_ID}/attribute.repository/${GITHUB_ORG}/${GITHUB_REPO}" \
  --project="${PROJECT_ID}"

echo ""
echo "═══════════════════════════════════════════════════════════════════════════"
echo " GCP setup complete! Add the following as GitHub Secrets:"
echo " https://github.com/${GITHUB_ORG}/${GITHUB_REPO}/settings/secrets/actions"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""
echo " Secret name                       Value"
echo " ─────────────────────────────     ─────────────────────────────────────"
echo " GCP_PROJECT_ID                    ${PROJECT_ID}"
echo " GCP_SERVICE_ACCOUNT               ${SERVICE_ACCOUNT_EMAIL}"
echo " GCP_WORKLOAD_IDENTITY_PROVIDER    ${PROVIDER_ID}"
echo ""
