# 로컬 릴리스 빌드

ai-stock를 개인용 데스크톱 앱으로 빌드하는 절차. CI·코드 서명은 아직 없으며,
빌드는 각 개발 머신에서 로컬로 수행한다.

## 사전 요구사항

- Rust ≥ 1.77 (`rustup`)
- Node.js ≥ 20
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- `npm install` 을 한 번 실행해 `@tauri-apps/cli` 등 의존성을 설치

## 빌드

```bash
npm run tauri build
```

`npm run build`(프론트엔드) → Rust 릴리스 컴파일 → Tauri 번들 순으로 진행된다.
산출물은 워크스페이스 루트의 `target/release/bundle/` 아래에 생성된다:

- `target/release/bundle/macos/ai-stock.app` — 실행 가능한 앱 번들
- `target/release/bundle/dmg/*.dmg` — 디스크 이미지 (예: `ai-stock_0.1.0_aarch64.dmg`)

## 서명되지 않은 앱 (macOS)

이 빌드는 코드 서명이 되어 있지 않다. 처음 실행할 때 macOS Gatekeeper가
"확인되지 않은 개발자" 경고로 실행을 막을 수 있다. 우회 방법:

- Finder에서 `ai-stock.app`을 **우클릭 → 열기**, 그다음 대화상자에서 **열기**.
  한 번 허용하면 이후로는 일반 실행된다.

코드 서명·공증(notarization)은 공개 배포 시점에 추가할 예정이다.

## Windows / Linux

`tauri.conf.json`의 번들 타깃은 `all`로 설정되어 있어 Windows(`.msi`)·
Linux(`.AppImage`) 번들도 각 OS에서 빌드할 수 있다. 다만 현재 그 두 플랫폼의
빌드는 검증되지 않았다(개발 환경이 macOS).

## 네이버 스크래퍼 contract test

KR 종목 시세는 `finance.naver.com` HTML 스크래핑에 의존한다. 네이버가 마크업을
바꾸면 스크래퍼가 조용히 깨질 수 있다. 이를 감지하는 `#[ignore]` contract
test가 있다 — 네트워크가 필요하고 기본 `cargo test`에서는 제외된다:

```bash
cargo test -p infrastructure -- --ignored contract_real_naver_quote
```

통과하면 스크래퍼가 여전히 동작하는 것이다. 파싱/단언 실패면 네이버가 HTML을
바꿨을 가능성이 높다.
