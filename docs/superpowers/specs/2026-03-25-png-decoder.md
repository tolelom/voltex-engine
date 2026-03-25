# PNG Decoder — Design Spec

## Overview

voltex_renderer에 PNG 디코더를 추가한다. Deflate(Huffman+LZ77) 자체 구현으로 외부 의존 없이 PNG 파일을 파싱.

## Scope

- PNG 시그니처 + 청크 파싱 (IHDR, IDAT, IEND)
- 8-bit RGB (color_type=2), 8-bit RGBA (color_type=6)
- Deflate 디코더 (zlib wrapper + fixed/dynamic Huffman + LZ77)
- PNG 필터 복원 (None, Sub, Up, Average, Paeth)

## Out of Scope

- 인터레이스 (Adam7)
- 16-bit depth
- 팔레트 (color_type=3)
- 그레이스케일 (color_type=0, 4)
- Adler32 검증

## Files

- `crates/voltex_renderer/src/deflate.rs` — inflate (Create)
- `crates/voltex_renderer/src/png.rs` — parse_png (Create)
- `crates/voltex_renderer/src/lib.rs` — 모듈 등록 (Modify)

## Test Plan

- deflate: inflate fixed Huffman compressed data
- png filters: each filter type reconstruction
- parse_png: manually crafted 2x2 PNG roundtrip
- RGB→RGBA conversion
