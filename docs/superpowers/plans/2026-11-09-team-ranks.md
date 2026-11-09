# Team ranks implementation plan

Approved design: show five allied champion ranks and AramHex overall win rates aligned with left selection rows; preserve bench badges. Extend existing polling and local cache, avoiding duplicate requests. Empty slots preserve order, missing rates show unavailable. Existing client-relative positioning is retained and requires live alignment verification.

- [x] Test team parsing including empty slots and swaps.
- [x] Extend observation and state, query matching cached rates.
- [x] Render five rows and verify Rust tests and frontend build.

## v0.1.2 release

User explicitly authorized packaging and pushing the update. Package, Tauri and Cargo versions are 0.1.2; README and changelog point to the new portable release. Frontend build and optimized native build passed; Rust library tests: 25 passed, 3 optional integration tests ignored. Independent read-only review found no release blockers. Live client row alignment remains unverified and is documented in release notes. Existing frozen OCR is unchanged and included.
