# 비교 보고서: 1.24.0(py) 대 새 판(rs)

`cargo xtask compare`가 쓰는 파일이다. 손으로 고치지 않는다. 방법과 합격 기준은 설계 12.1절과 12.4절, 다시 돌리는 명령은 `docs/bench.md`에 있다.

- 시나리오 1개, 단계 12개.
- 기준선(py 구동기): 돌렸다.
- 견주는 것: hook 동사는 판정 종류, 흐름 동사(`lead_*`·`worker_*`)는 흐름(이어짐·멈춤). 문구와 파일 바이트는 견주지 않는다.
- 결과 칸: `같음`, `의도한 차이`(설계 12.3절의 DF나 12.2절의 "더 나음"이 그 시나리오 ID의 행에 있다), `기준선 없음`(1.24.0에 대응이 없거나 12.2절의 1.24.0 칸이 `-`다), `보류`(`pending`), `회귀`(그 밖의 차이).

## 합격 점검 (설계 12.4절)

| 항목 | 값 | 판정 |
|---|---|---|
| 회귀 | 0건 | 합격 |
| rs가 시나리오의 기대와 다른 단계 | 0건 | 합격 |
| 남은 `pending` | 11건(묶음별: B 4, C 5, E 2) | 미달 |
| 시나리오 파일이 없는 12.2절 ID | 101개 | 미달 |

시나리오 파일이 없는 ID: A1, A2, A3, A4, B1, B2, B3, B4, B5, B6, C1, C2, C3, C4, C5, D1, D2, D3, D4, D5, D6, D7, D8, E1, E2, E3, E4, E5, F1, F2, G1, G2, G3, G4, G5, H1, H2, H3, H4, H5, I1, I2, I3, I4, I5, I6, I7, J1, J2, J3, J4, J5, K1, K2, K3, L1, L2, L3, L4, L5, M1, M2, N1, N2, N3, O1, O2, O3, O4, O5, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, Q1, Q2, Q3, Q4, Q5, Q6, Q7, Q8, Q9, Q10, R1, R2, R3, R4, R5, R6, R7, R8.

같음 1건, 의도한 차이 0건, 기준선 없음 0건, 보류 11건, 회귀 0건.

## 끝까지 간 정도

사람 입력 없이 병합된 과제 수는 통합 기준 ref에 들어간 `<KEY> 구현` 커밋의 키 수이고, lead가 받은 차단 수는 lead가 한 단계 가운데 판정이 차단·거부인 것이다. K4의 합격은 병합된 과제 수가 py 이상이고 차단 수가 py 이하인 것이다. rs 칸의 `-`는 보류 단계가 남아 아직 셀 수 없다는 뜻이다.

| 시나리오 | 병합된 과제(py) | 병합된 과제(rs) | lead 차단(py) | lead 차단(rs) |
|---|---|---|---|---|
| k4 | 2 | - | 2 | - |

## 단계별 결과

| 시나리오 | 단계 | py 판정 | rs 판정 | py 흐름 | rs 흐름 | 결과 | py 시간 | rs 시간 |
|---|---|---|---|---|---|---|---|---|
| k4 | 1. session_starts | warn | pending | continues | - | 보류(묶음 B) | 1077ms | - |
| k4 | 2. user_prompts | pass | pass | continues | continues | 같음 | 386ms | 66ms |
| k4 | 3. lead_plans | pass | pending | continues | - | 보류(묶음 C) | 772ms | - |
| k4 | 4. lead_plans | pass | pending | continues | - | 보류(묶음 C) | 758ms | - |
| k4 | 5. lead_plans | pass | pending | continues | - | 보류(묶음 C) | 905ms | - |
| k4 | 6. worker_reports | pass | pending | continues | - | 보류(묶음 C) | 16957ms | - |
| k4 | 7. lead_merges | pass | pending | continues | - | 보류(묶음 E) | 10280ms | - |
| k4 | 8. stops | block | pending | stopped | - | 보류(묶음 B) | 1185ms | - |
| k4 | 9. worker_reports | pass | pending | continues | - | 보류(묶음 C) | 12721ms | - |
| k4 | 10. lead_merges | pass | pending | continues | - | 보류(묶음 E) | 13192ms | - |
| k4 | 11. stops | block | pending | stopped | - | 보류(묶음 B) | 1517ms | - |
| k4 | 12. stops | pass | pending | continues | - | 보류(묶음 B) | 1413ms | - |
