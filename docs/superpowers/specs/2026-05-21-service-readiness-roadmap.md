# Voltex v1.0.x — Service-Readiness Roadmap

> 2026-05-21 작성. v1.0.0 release 후 "서비스 배포 관점 완성도" 끌어올리는 다중 사이클 로드맵.
> 자동 루프가 각 사이클을 순차 실행. 1 cycle ≈ 1 commit + push.

## 평가 축

서비스 배포 (= 외부 사용자가 실 게임을 빌드/배포할 때 부딪히는 모든 마찰) 관점에서 4 축으로 분류:

- **P0 사용자 직접 영향** — UX, 크래시, 빌드 산출물 완전성
- **P1 운영 인프라** — 로깅, 보안 스캔, 릴리즈 자동화
- **P2 문서/DX** — 진입장벽 낮추기, reference 완성도
- **P3 품질/회귀** — coverage, missing_docs, API 안정성

## 사이클 목록

### P0: 사용자 직접 영향
| Cycle | 작업 | Why |
|---|---|---|
| v1.0.9 | OGG/Vorbis lib.rs 등록 + clippy 27 → 0 + AudioClip 통합 | Phase 6-1 약속 채움 + STATUS 정직성 |
| v1.0.10 | panic hook + 크래시 로그 파일 (`crash-YYYYMMDD-HHMMSS.txt`) | 배포된 exe 가 죽었을 때 사용자가 보낼 수 있는 단서 |
| v1.0.11 | voltex.toml 스키마 검증 강화 (unknown key reject + path 존재 확인 + 친화 에러) | `voltex build` 가 잘못된 매니페스트로 cryptic cargo 에러 띄우는 것 방지 |
| v1.0.12 | voltex build artifact 에 THIRD-PARTY-LICENSES.md 동봉 | 외부 배포 시 라이선스 고지 의무 자동 충족 |
| v1.0.13 | voltex new → build → run E2E 통합 테스트 (voltex_cli `--test-e2e`) | 5분 quickstart 영구 회귀 방지 |

### P1: 운영 인프라
| Cycle | 작업 | Why |
|---|---|---|
| v1.0.14 | structured logging (자체 mini logger or log crate 채택 결정) | 사용자 게임에서 디버그 정보 추출 가능 |
| v1.0.15 | cargo-deny 통합 (`deny.toml` + CI job) | license/advisory/duplicate 단일 스캔 |
| v1.0.16 | CHANGELOG entry 강제 (PR 검증 스크립트) | 릴리즈 노트 누락 방지 |
| v1.0.17 | SemVer compliance (`cargo-semver-checks` CI advisory) | 1.0 이후 breaking change 자동 감지 |
| v1.0.18 | release artifact 자동화 — tag push → exe 산출 스크립트 | 수동 빌드 절차 제거 |

### P2: 문서/DX
| Cycle | 작업 | Why |
|---|---|---|
| v1.0.19 | 각 example crate 의 README.md (what/how/key files) | 신규 사용자가 어느 example 부터 봐야 할지 안내 |
| v1.0.20 | `docs/voltex-toml-reference.md` — 모든 필드 + 디폴트 + 예시 | manifest 작성 시 코드 안 보고 가능 |
| v1.0.21 | `voltex --help` / 서브커맨드 도움말 풍부화 + 예시 포함 | CLI 첫 사용 마찰 감소 |
| v1.0.22 | CONTRIBUTING.md — 의존성 정책, 600줄 규칙, error 표준, 테스트 스타일 명시 | 외부 contributor 가이드 |

### P3: 품질/회귀
| Cycle | 작업 | Why |
|---|---|---|
| v1.0.23 | pre-commit hook (`.githooks/pre-commit` + 설치 스크립트) — fmt + clippy + test --lib quick | 로컬에서 회귀 1차 차단 |
| v1.0.24 | voltex_math 100% missing_docs (가장 작고 stable한 crate부터) | 1573 missing_docs 점진 해소 시작 |
| v1.0.25 | voltex_ecs 100% missing_docs | 핵심 API 명세화 |
| v1.0.26 | pub(crate) 다운그레이드 — `docs/v1.0-pub-audit.md` 후보 실 적용 | API surface 축소 |
| v1.0.27 | headless GPU test harness 첫 실험 (voltex_platform → voltex_renderer 순) | coverage 80% gate 의 4 GPU crate 의 첫 발자국 |

## 우선순위 원칙

- P0 먼저, 그다음 P1 → P2 → P3 순. 단 P3 의 missing_docs sweep 은 P2 와 인터리브 가능.
- 각 cycle 은 단일 commit (atomic) — STATUS + CHANGELOG + DEFERRED 동시 업데이트.
- 각 cycle 은 자체 spec/plan 파일을 `docs/superpowers/specs/` 또는 `plans/` 에 동봉 (사후 추적용).
- 실패한 cycle 은 commit 안 함. 다음 cycle 로 넘기지 말고 재시도.

## 자동 진행 규약

1. 세션 시작 시 `git fetch && git status` 으로 origin/master 와 동기화 확인.
2. 이 문서의 첫 `[ ]` 사이클 진입. 진행 중에는 `[~]` 로 마킹.
3. Cycle 종료 시: STATUS/CHANGELOG/DEFERRED 업데이트 + atomic commit + push.
4. 이 문서에 `[✅] <hash>` 로 마킹.
5. 다음 cycle 로 즉시 또는 ScheduleWakeup.

## 진행 추적

| # | Cycle | 상태 | Spec/Plan | Commit |
|---|---|---|---|---|
| 1 | v1.0.9 OGG/Vorbis wire | `[✅]` | `plans/2026-05-21-v1.0.9-ogg-vorbis-wire.md` | c9b75ab |
| 2 | v1.0.10 panic hook + crash log | `[✅]` | (inline plan in this doc § v1.0.10) | c98ef6f |
| 3 | v1.0.11 voltex.toml schema | `[✅]` | (inline in STATUS § v1.0.11) | da33040 |
| 4 | v1.0.12 license bundle in build | `[✅]` | (inline STATUS § v1.0.12) | c4ea097 |
| 5 | v1.0.13 CLI help/version richness | `[✅]` | (inline STATUS § v1.0.13) | d64c9ca |
| 5b | E2E new/build/run test | `[ ]` | — | re-numbered (heavy: ~60s cargo build per run, #[ignore]'d) |
| 6 | v1.0.14 CONTRIBUTING + pre-commit hook | `[✅]` | (inline STATUS § v1.0.14) | 5e7b9de |
| 7 | v1.0.15 examples catalog + flagship README | `[✅]` | (inline STATUS § v1.0.15) | e0abe66 |
| 8 | v1.0.16 voltex-toml-reference.md | `[✅]` | (inline STATUS § v1.0.16) | a5b5ec8 |
| 9 | v1.0.17 structured logger | `[✅]` | (inline STATUS § v1.0.17) | de19527 |
| 10 | v1.0.18 panic hook relocation to new() | `[✅]` | (inline STATUS § v1.0.18) | 6f55371 |
| 11 | v1.0.19 voltex_math missing_docs 59→0 | `[✅]` | (inline STATUS § v1.0.19) | 8b44fed |
| 12 | v1.0.20 voltex_platform missing_docs 38→0 | `[✅]` | (inline STATUS § v1.0.20) | ced2333 |
| 13 | v1.0.21 voltex_asset missing_docs 64→0 | `[✅]` | (inline STATUS § v1.0.21) | 338e815 |
| 14 | v1.0.22 voltex_net missing_docs 71→0 | `[✅]` | (inline STATUS § v1.0.22) | 4a26514 |
| 15 | v1.0.23 voltex_ai missing_docs →0 | `[✅]` | (inline STATUS § v1.0.23) | 096a460 |
| 16 | v1.0.24 voltex_audio missing_docs →0 | `[✅]` | (inline STATUS § v1.0.24) | (pending commit) |
| 7 | v1.0.15 cargo-deny | `[ ]` | — | — |
| 8 | v1.0.16 CHANGELOG enforcement | `[ ]` | — | — |
| 9 | v1.0.17 SemVer check | `[ ]` | — | — |
| 10 | v1.0.18 release artifact automation | `[ ]` | — | — |
| 11 | v1.0.19 example READMEs | `[ ]` | — | — |
| 12 | v1.0.20 voltex.toml reference | `[ ]` | — | — |
| 13 | v1.0.21 CLI help richness | `[ ]` | — | — |
| 14 | v1.0.22 CONTRIBUTING.md | `[ ]` | — | — |
| 15 | v1.0.23 pre-commit hook | `[ ]` | — | — |
| 16 | v1.0.24 voltex_math missing_docs | `[ ]` | — | — |
| 17 | v1.0.25 voltex_ecs missing_docs | `[ ]` | — | — |
| 18 | v1.0.26 pub(crate) downgrade | `[ ]` | — | — |
| 19 | v1.0.27 headless GPU harness | `[ ]` | — | — |
