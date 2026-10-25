#!/usr/bin/env bash
# Apply infra/ to MiniStack and round-trip S3, DynamoDB, SQS, and the VPC.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

terraform_version=1.17.0

if ! command -v terraform >/dev/null 2>&1; then
  zip=/tmp/terraform_${terraform_version}_linux_amd64.zip
  sums=/tmp/terraform_${terraform_version}_SHA256SUMS
  curl -fsSL -o "$zip" "https://releases.hashicorp.com/terraform/${terraform_version}/terraform_${terraform_version}_linux_amd64.zip"
  curl -fsSL -o "$sums" "https://releases.hashicorp.com/terraform/${terraform_version}/terraform_${terraform_version}_SHA256SUMS"
  (cd /tmp && sha256sum -c --ignore-missing "$sums")
  sudo python3 -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extract("terraform", "/usr/local/bin")' "$zip"
  sudo chmod 755 /usr/local/bin/terraform
fi

if ! python3 -c 'import ensurepip' >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq python3-venv
fi

ministack_venv=$root/.venv
probe_venv=$root/.venv-probe

if [[ ! -x $ministack_venv/bin/ministack ]]; then
  python3 -m venv "$ministack_venv"
  "$ministack_venv/bin/pip" install -q ministack
fi

if [[ ! -x $probe_venv/bin/python ]] || ! "$probe_venv/bin/python" -c 'import boto3' >/dev/null 2>&1; then
  python3 -m venv "$probe_venv"
  "$probe_venv/bin/pip" install -q boto3
fi

if ! curl -fsS http://127.0.0.1:4566/_ministack/health >/dev/null; then
  "$ministack_venv/bin/ministack" -d
  for _ in $(seq 1 50); do
    if curl -fsS http://127.0.0.1:4566/_ministack/health >/dev/null; then
      break
    fi
    sleep 0.2
  done
fi

curl -fsS http://127.0.0.1:4566/_ministack/health >/dev/null

terraform -chdir=infra init -input=false
terraform -chdir=infra apply -input=false -auto-approve -var-file=ministack.tfvars
"$probe_venv/bin/python" scripts/check_ministack.py
terraform -chdir=infra plan -input=false -detailed-exitcode -var-file=ministack.tfvars

cargo test
MINISTACK_ENDPOINT=http://127.0.0.1:4566 cargo test -- --ignored ministack_notes_follow_the_login_flow
