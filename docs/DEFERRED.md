# 미뤄진/간소화된 구현 항목

## Phase 2

- **PNG 디코더 자체 구현** — deflate + 필터링. 현재 BMP만 지원.
- **JPG 디코더 자체 구현** — Huffman + DCT. 현재 미구현.
- **glTF 파서** — OBJ만 지원 중.

## Phase 3a

- **Archetype 기반 스토리지** → SparseSet 사용 중. 대규모 씬에서 성능 이슈 시 전환.
- **시스템 스케줄러** — 의존성 기반 실행 순서/병렬 실행 미구현. 시스템은 함수 호출.
- **쿼리 필터** — With, Without, Changed 미구현. query/query2만 존재.
- **query3+** — query2까지만 있음.

## Phase 3b

- **JSON 직렬화** → 커스텀 .vscn 텍스트 포맷 사용.
- **바이너리 씬 포맷** — 미구현.
- **임의 컴포넌트 직렬화** — Transform/Parent/Tag만 지원.

## Phase 3c

- **비동기 로딩** — 동기 insert만.
- **핫 리로드** — 파일 변경 감지 미구현.

## Phase 4a

- **Metallic/Roughness/AO 텍스처 맵** → 파라미터 값만 사용. 텍스처 샘플링 미구현.
- **Emissive 맵** — 미구현.

## Phase 4b

- **CSM (Cascaded Shadow Maps)** → 단일 캐스케이드만. 원거리 그림자 해상도 낮음.
- **Point Light Shadow (큐브맵)** — 미구현.
- **Spot Light Shadow** — 미구현.
- **라이트 컬링** — 타일/클러스터 기반 미구현.

## Phase 4c

- **HDR 큐브맵 환경맵** → 프로시저럴 sky 함수로 대체.
- **Irradiance/Prefiltered Map 컨볼루션** → 프로시저럴 근사.
- **GPU 컴퓨트 BRDF LUT** → CPU 생성 (256x256).

## 렌더링 한계

- **per-entity dynamic UBO** — 수천 개 이상은 인스턴싱 필요.
- **max_bind_groups=4** — IBL을 shadow group에 합쳐서 해결. 추가 group 필요 시 리소스 합치거나 bindless 활용.
