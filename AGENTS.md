## Regulation

- Grok Bot / Cursor Cloud Agents で開発する
- Rust を用いる
- AWS を用いる　S3/DynamoDB/SQS/EC2 あたり　IaC をする

## Tasks

- [ ] traQ ログインを掛けられる
  - https://github.com/tokio-rs/axum/tree/main/examples/oauth
- [ ] 画像投稿できる
- [ ] 画像加工できる
  - [ ] 画像加工サーバがスケールする
- [ ] 画像加工完了を通知できる

## Code Quality

- Domain-First / API-First で開発する
  - https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust
- [Prove It Works](https://github.com/cursor/plugins/blob/main/pstack/skills/principle-prove-it-works/SKILL.md) し続ける

## Cursor Cloud specific instructions

Terraform は `infra/`。アプリは Rust の `domain` / `inbound` / `outbound`。実 AWS に当てる前に MiniStack（`http://127.0.0.1:4566`）へ `./scripts/verify-infra.sh` を流す。接続先は `infra/ministack.tfvars`。メモの DynamoDB は `NOTES_BACKEND=dynamodb` と `AWS_ENDPOINT_URL`。traQ の `CLIENT_ID` / `CLIENT_SECRET` と実 AWS の秘密情報は Cursor の Secrets に置く。

## Git

To open a PR, spawn a fresh subagent whose entire prompt is `/pr`.

