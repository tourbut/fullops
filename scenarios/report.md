# 비교 보고서: 1.24.0(py) 대 새 판(rs)

`cargo xtask compare`가 쓰는 파일이다. 손으로 고치지 않는다. 방법과 합격 기준은 설계 12.1절과 12.4절, 다시 돌리는 명령은 `docs/bench.md`에 있다.

- 시나리오 150개, 단계 1603개.
- 기준선(py 구동기): 돌렸다.
- 견주는 것: hook 동사는 판정 종류, 흐름 동사(`lead_*`·`worker_*`)는 흐름(이어짐·멈춤). 문구와 파일 바이트는 견주지 않는다.
- 결과 칸: `같음`, `의도한 차이`(그 단계에 적은 `df`가 설계 12.3절에 있다. `df`를 적은 단계가 하나도 없는 시나리오 파일은 그 ID의 12.2절 행에 있는 DF로 가리고 "행:"이라고 적는다. 12.2절 행의 "더 나음"도 여기 든다), `기준선 없음`(1.24.0에 대응이 없거나 12.2절의 1.24.0 칸이 `-`다), `보류`(`pending`), `회귀`(그 밖의 차이).
- 단계의 `py`는 py 구동기가 내야 하는 판정이다. 다르면 결과 칸에 `py 기대와 다름`이 붙고 기대와 다른 단계로 센다.

## 합격 점검 (설계 12.4절)

| 항목 | 값 | 판정 |
|---|---|---|
| 회귀 | 0건 | 합격 |
| 시나리오의 기대와 다른 단계(rs의 `expect`, py의 `py`) | 0건 | 합격 |
| 남은 `pending` | 0건 | 합격 |
| 시나리오 파일이 없는 12.2절 ID | 0개 | 합격 |
| py 구동기가 끝까지 돌지 못한 시나리오 | 0개 | 합격 |

같음 1250건, 의도한 차이 143건(그 가운데 12.2절 행 단위로 가린 것 0건. 단계에 `df`를 적으면 단계 단위가 된다), 기준선 없음 210건, 보류 0건, 회귀 0건.

## 끝까지 간 정도

사람 입력 없이 병합된 과제 수는 통합 기준 ref에 들어간 `<KEY> 구현` 커밋의 키 수이고, lead가 받은 차단 수는 lead가 한 단계 가운데 판정이 차단·거부인 것이다. K4의 합격은 병합된 과제 수가 py 이상이고 차단 수가 py 이하인 것이다. rs 칸의 `-`는 보류 단계가 남아 아직 셀 수 없다는 뜻이다.

| 시나리오 | 병합된 과제(py) | 병합된 과제(rs) | lead 차단(py) | lead 차단(rs) |
|---|---|---|---|---|
| a1 | 0 | 0 | 0 | 0 |
| a2 | 0 | 0 | 0 | 0 |
| a3 | 0 | 0 | 7 | 3 |
| a4 | 0 | 0 | 3 | 2 |
| b1 | 0 | 0 | 0 | 0 |
| b2 | 0 | 0 | 0 | 0 |
| b3 | 0 | 0 | 0 | 0 |
| b4 | 0 | 0 | 0 | 0 |
| b5 | 0 | 0 | 1 | 1 |
| b6 | 0 | 0 | 0 | 0 |
| c1 | 0 | 0 | 1 | 1 |
| c2 | 0 | 0 | 2 | 7 |
| c3 | 0 | 0 | 1 | 5 |
| c4 | 0 | 0 | 3 | 4 |
| c5 | 0 | 0 | 4 | 3 |
| d1 | 0 | 0 | 2 | 2 |
| d2 | 0 | 0 | 2 | 2 |
| d3 | 0 | 1 | 3 | 1 |
| d4 | 0 | 0 | 0 | 1 |
| d5 | 0 | 0 | 2 | 2 |
| d6 | 0 | 0 | 0 | 1 |
| d7-off | 0 | 0 | 2 | 2 |
| d7-on | 0 | 0 | 1 | 0 |
| d7-read | 0 | 0 | 7 | 3 |
| d8 | 0 | 0 | 0 | 0 |
| e1 | 0 | 0 | 1 | 1 |
| e2 | 0 | 0 | 0 | 0 |
| e3 | 0 | 0 | 1 | 1 |
| e4 | 0 | 0 | 3 | 2 |
| e5 | 0 | 0 | 0 | 0 |
| f1 | 0 | 0 | 0 | 0 |
| f2 | 0 | 0 | 0 | 0 |
| g1 | 0 | 0 | 0 | 0 |
| g2 | 0 | 0 | 0 | 0 |
| g3 | 0 | 0 | 0 | 0 |
| g4 | 0 | 0 | 3 | 3 |
| g5 | 0 | 0 | 2 | 3 |
| h1 | 0 | 0 | 0 | 0 |
| h2 | 0 | 0 | 0 | 0 |
| h3 | 0 | 0 | 0 | 0 |
| h4 | 0 | 0 | 0 | 0 |
| h5 | 0 | 0 | 0 | 0 |
| i1 | 0 | 0 | 2 | 1 |
| i2 | 0 | 0 | 0 | 2 |
| i3 | 0 | 0 | 0 | 2 |
| i4-peek | 0 | 1 | 3 | 1 |
| i4 | 1 | 1 | 7 | 4 |
| i5 | 0 | 0 | 0 | 1 |
| i6 | 0 | 0 | 0 | 0 |
| i7 | 0 | 0 | 0 | 0 |
| j1 | 1 | 1 | 2 | 2 |
| j2 | 1 | 1 | 0 | 0 |
| j3 | 0 | 0 | 3 | 3 |
| j4 | 1 | 1 | 3 | 3 |
| j5 | 0 | 1 | 1 | 0 |
| k1 | 0 | 0 | 2 | 2 |
| k2 | 0 | 0 | 1 | 0 |
| k3 | 0 | 0 | 1 | 1 |
| k4 | 2 | 2 | 2 | 1 |
| l1 | 0 | 0 | 0 | 0 |
| l2-decision | 0 | 0 | 0 | 4 |
| l2 | 0 | 1 | 5 | 1 |
| l3 | 0 | 0 | 1 | 2 |
| l4 | 0 | 0 | 1 | 0 |
| l5-conflict | 0 | 0 | 0 | 1 |
| l5-lint-fail | 0 | 0 | 1 | 1 |
| l5-merge-commit | 1 | 1 | 0 | 0 |
| l5-push-fail | 1 | 1 | 1 | 1 |
| l5-rerun | 2 | 2 | 1 | 1 |
| l5 | 1 | 1 | 2 | 2 |
| lint-anti-002 | 0 | 0 | 2 | 2 |
| lint-anti-003 | 0 | 0 | 3 | 3 |
| lint-anti-004 | 0 | 0 | 2 | 2 |
| lint-anti-005 | 0 | 0 | 0 | 0 |
| lint-commands | 0 | 0 | 8 | 10 |
| lint-dep-001 | 0 | 0 | 0 | 0 |
| lint-doc-001 | 0 | 0 | 0 | 0 |
| lint-doc-002 | 0 | 0 | 2 | 2 |
| lint-doc-003 | 0 | 0 | 3 | 2 |
| lint-lint-000 | 0 | 0 | 0 | 0 |
| lint-lint-001 | 0 | 0 | 0 | 0 |
| lint-lint-002-slow | 0 | 0 | 0 | 0 |
| lint-lint-002 | 0 | 0 | 0 | 0 |
| lint-rules | 0 | 0 | 6 | 6 |
| lint-sec-001 | 0 | 0 | 1 | 1 |
| lint-size-001 | 0 | 0 | 0 | 0 |
| lint-size-002 | 0 | 0 | 0 | 0 |
| m1 | 0 | 0 | 0 | 0 |
| m2 | 0 | 0 | 0 | 0 |
| n1 | 0 | 0 | 2 | 1 |
| n2 | 0 | 0 | 0 | 0 |
| n3 | 0 | 0 | 1 | 2 |
| o1-codex | 0 | 0 | 1 | 1 |
| o1 | 0 | 0 | 0 | 0 |
| o2 | 0 | 0 | 0 | 0 |
| o3 | 0 | 0 | 2 | 1 |
| o4 | 0 | 0 | 0 | 0 |
| o5 | 0 | 0 | 0 | 0 |
| p1 | 0 | 0 | 0 | 1 |
| p10 | 0 | 0 | 1 | 1 |
| p11 | 0 | 0 | 0 | 1 |
| p12 | 0 | 0 | 2 | 0 |
| p13 | 0 | 0 | 3 | 5 |
| p14 | 0 | 0 | 1 | 1 |
| p2 | 0 | 0 | 5 | 4 |
| p3 | 0 | 0 | 3 | 4 |
| p4 | 0 | 0 | 3 | 3 |
| p5 | 0 | 0 | 1 | 2 |
| p6 | 0 | 0 | 3 | 2 |
| p7 | 0 | 0 | 0 | 0 |
| p8 | 0 | 0 | 0 | 0 |
| p9 | 0 | 0 | 0 | 0 |
| q1 | 0 | 0 | 0 | 0 |
| q10 | 0 | 0 | 0 | 1 |
| q2-conflict | 0 | 0 | 0 | 0 |
| q2-install | 0 | 0 | 0 | 0 |
| q2-no-package | 0 | 0 | 0 | 0 |
| q2-skill-fail | 0 | 0 | 0 | 0 |
| q3 | 0 | 0 | 0 | 0 |
| q4 | 0 | 0 | 0 | 0 |
| q5-backup | 0 | 0 | 0 | 0 |
| q5-no-npx | 0 | 0 | 0 | 0 |
| q6 | 0 | 0 | 0 | 3 |
| q7-codex | 0 | 0 | 0 | 0 |
| q7 | 0 | 0 | 0 | 0 |
| q8-agy | 0 | 0 | 0 | 0 |
| q8-codex | 0 | 0 | 0 | 0 |
| q8-grok | 0 | 0 | 0 | 0 |
| q8 | 0 | 0 | 0 | 0 |
| q9 | 0 | 0 | 0 | 0 |
| r1-codex | 0 | 0 | 0 | 0 |
| r1-grok | 0 | 0 | 0 | 0 |
| r1-off | 0 | 0 | 0 | 0 |
| r1-on-agent | 0 | 0 | 0 | 0 |
| r1-on | 0 | 0 | 0 | 0 |
| r1-ultra | 0 | 0 | 0 | 0 |
| r2-off | 0 | 0 | 0 | 0 |
| r2-ultra | 0 | 0 | 0 | 0 |
| r2 | 0 | 0 | 0 | 0 |
| r3-off | 0 | 0 | 0 | 0 |
| r3 | 0 | 0 | 0 | 0 |
| r4 | 0 | 0 | 2 | 2 |
| r5 | 0 | 0 | 0 | 0 |
| r6-off | 0 | 0 | 1 | 1 |
| r6 | 0 | 0 | 2 | 0 |
| r7 | 0 | 0 | 0 | 0 |
| r8-full | 0 | 0 | 0 | 0 |
| r8-lite | 0 | 0 | 0 | 0 |
| r8-missing | 0 | 0 | 0 | 0 |
| r8-off | 0 | 0 | 0 | 0 |

## 단계별 결과

| 시나리오 | 단계 | py 판정 | rs 판정 | py 흐름 | rs 흐름 | 결과 | py 시간 | rs 시간 |
|---|---|---|---|---|---|---|---|---|
| a1 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| a1 | 2. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| a1 | 3. session_starts | warn | pass | continues | continues | 의도한 차이(DF-12) | 450ms | 167ms |
| a1 | 4. user_prompts | pass | pass | continues | continues | 같음 | 209ms | 23ms |
| a1 | 5. runs_shell | pass | pass | continues | continues | 같음 | 228ms | 25ms |
| a1 | 6. writes_file | pass | pass | continues | continues | 같음 | 438ms | 23ms |
| a1 | 7. stops | pass | pass | continues | continues | 같음 | 377ms | 24ms |
| a1 | 8. turn_fails | pass | pass | continues | continues | 같음 | 215ms | 24ms |
| a1 | 9. session_starts | warn | pass | continues | continues | 의도한 차이(DF-12) | 525ms | 27ms |
| a2 | 1. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 22ms |
| a2 | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 22ms |
| a2 | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 22ms |
| a2 | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 20ms |
| a2 | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 20ms |
| a2 | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 21ms |
| a3 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 977ms | 44ms |
| a3 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| a3 | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 606ms | 31ms |
| a3 | 4. writes_file | deny | deny | stopped | stopped | 같음 | 311ms | 29ms |
| a3 | 5. runs_shell | deny | deny | stopped | stopped | 같음 | 302ms | 24ms |
| a3 | 6. runs_shell | deny | pass | stopped | continues | 의도한 차이(DF-26) | 310ms | 29ms |
| a3 | 7. runs_shell | deny | deny | stopped | stopped | 같음 | 304ms | 22ms |
| a3 | 8. user_prompts | warn | context | continues | continues | 의도한 차이(DF-25) | 303ms | 33ms |
| a3 | 9. user_prompts | warn | pass | continues | continues | 의도한 차이(DF-25) | 315ms | 30ms |
| a3 | 10. writes_file | deny | pass | stopped | continues | 의도한 차이(DF-26) | 843ms | 24ms |
| a3 | 11. writes_file | deny | pass | stopped | continues | 의도한 차이(DF-26) | 306ms | 24ms |
| a3 | 12. stops | block | pass | stopped | continues | 의도한 차이(DF-26) | 575ms | 104ms |
| a4 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 936ms | 43ms |
| a4 | 2. runs_shell | pass | pass | continues | continues | 같음 | 352ms | 25ms |
| a4 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| a4 | 4. runs_shell | deny | deny | stopped | stopped | 같음 | 307ms | 24ms |
| a4 | 5. session_starts | pass | context | continues | continues | 의도한 차이(DF-25) | 346ms | 25ms |
| a4 | 6. runs_shell | deny | pass | stopped | continues | 의도한 차이(DF-26) | 297ms | 30ms |
| a4 | 7. stops | block | block | stopped | stopped | 같음 | 826ms | 36ms |
| a4 | 8. stops | warn | warn | continues | continues | 같음 | 745ms | 24ms |
| a4 | 9. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 924ms | 39ms |
| a4 | 10. runs_shell | pass | pass | continues | continues | 같음 | 352ms | 26ms |
| a4 | 11. stops | pass | pass | continues | continues | 같음 | 795ms | 24ms |
| b1 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 918ms | 356ms |
| b1 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 923ms | 357ms |
| b1 | 3. session_starts | pass | pass | continues | continues | 같음 | 306ms | 342ms |
| b2 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 912ms | 64ms |
| b2 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 1007ms | 42ms |
| b2 | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 926ms | 38ms |
| b2 | 4. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 909ms | 38ms |
| b3 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 998ms | 42ms |
| b3 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 1061ms | 45ms |
| b3 | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 1013ms | 40ms |
| b3 | 4. preamble_arrives | context | context | continues | continues | 같음 | 359ms | 35ms |
| b3 | 5. preamble_arrives | pass | pass | continues | continues | 같음 | 303ms | 31ms |
| b4 | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b4 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 1024ms | 44ms |
| b4 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b4 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b4 | 5. stops | block | pass | stopped | continues | 의도한 차이(DF-4) | 1275ms | 25ms |
| b4 | 6. runs_shell | deny | deny | stopped | stopped | 같음 | 403ms | 24ms |
| b4 | 7. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b4 | 8. worker_reports | pass | pass | continues | continues | 같음 | 13257ms | 439ms |
| b4 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 390ms |
| b5 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b5 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 938ms | 44ms |
| b5 | 3. preamble_arrives | pass | pass | continues | continues | 같음 | 307ms | 33ms |
| b5 | 4. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b5 | 5. stops | block | block | stopped | stopped | 같음 | 788ms | 95ms |
| b5 | 6. stops | warn | warn | continues | continues | 같음 | 784ms | 91ms |
| b6 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| b6 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 987ms | 40ms |
| b6 | 3. session_starts | pass | context | continues | continues | 의도한 차이(DF-27) | 401ms | 26ms |
| b6 | 4. session_starts | pass | context | continues | continues | 의도한 차이(DF-27) | 384ms | 26ms |
| b6 | 5. session_starts | pass | pass | continues | continues | 같음 | 391ms | 24ms |
| c1 | 1. runs_shell | deny | deny | stopped | stopped | 같음 | 348ms | 28ms |
| c1 | 2. runs_shell | deny | deny | stopped | stopped | 같음 | 359ms | 31ms |
| c1 | 3. runs_shell | pass | pass | continues | continues | 같음 | 347ms | 22ms |
| c1 | 4. runs_shell | pass | pass | continues | continues | 같음 | 356ms | 25ms |
| c1 | 5. runs_shell | pass | pass | continues | continues | 같음 | 348ms | 22ms |
| c1 | 6. runs_shell | pass | pass | continues | continues | 같음 | 356ms | 22ms |
| c1 | 7. runs_shell | pass | pass | continues | continues | 같음 | 362ms | 22ms |
| c2 | 1. runs_shell | deny | deny | stopped | stopped | 같음 | 363ms | 27ms |
| c2 | 2. runs_shell | deny | deny | stopped | stopped | 같음 | 354ms | 30ms |
| c2 | 3. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-1) | 348ms | 24ms |
| c2 | 4. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-1) | 363ms | 23ms |
| c2 | 5. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-1) | 346ms | 22ms |
| c2 | 6. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-1) | 348ms | 26ms |
| c2 | 7. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-1) | 360ms | 23ms |
| c3 | 1. runs_shell | deny | deny | stopped | stopped | 같음 | 358ms | 27ms |
| c3 | 2. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-5) | 355ms | 27ms |
| c3 | 3. runs_shell | pass | pass | continues | continues | 같음 | 359ms | 22ms |
| c3 | 4. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-5) | 396ms | 24ms |
| c3 | 5. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-5) | 377ms | 23ms |
| c3 | 6. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-5) | 357ms | 25ms |
| c3 | 7. runs_shell | pass | pass | continues | continues | 같음 | 354ms | 24ms |
| c4 | 1. runs_shell | deny | deny | stopped | stopped | 같음 | 364ms | 28ms |
| c4 | 2. runs_shell | deny | deny | stopped | stopped | 같음 | 359ms | 26ms |
| c4 | 3. runs_shell | deny | deny | stopped | stopped | 같음 | 350ms | 24ms |
| c4 | 4. runs_shell | pass | pass | continues | continues | 같음 | 364ms | 22ms |
| c4 | 5. runs_shell | pass | pass | continues | continues | 같음 | 361ms | 24ms |
| c4 | 6. runs_shell | deny | deny | stopped | stopped | 같음 | 351ms | 25ms |
| c4 | 7. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-5) | 363ms | 24ms |
| c5 | 1. runs_shell | deny | deny | stopped | stopped | 같음 | 353ms | 27ms |
| c5 | 2. runs_shell | deny | deny | stopped | stopped | 같음 | 364ms | 26ms |
| c5 | 3. runs_shell | deny | pass | stopped | continues | 의도한 차이(DF-28) | 349ms | 23ms |
| c5 | 4. runs_shell | deny | pass | stopped | continues | 의도한 차이(DF-28) | 352ms | 22ms |
| c5 | 5. runs_shell | pass | deny | continues | stopped | 의도한 차이(DF-1) | 350ms | 23ms |
| c5 | 6. runs_shell | pass | pass | continues | continues | 같음 | 362ms | 25ms |
| c5 | 7. lead_routes | pass | pass | continues | continues | 같음 | 1268ms | 121ms |
| c5 | 8. runs_shell | pass | pass | continues | continues | 같음 | 416ms | 23ms |
| d1 | 1. lead_dispatches | deny | deny | stopped | stopped | 같음 | 1085ms | 97ms |
| d1 | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 128ms |
| d1 | 3. lead_dispatches | deny | deny | stopped | stopped | 같음 | 369ms | 92ms |
| d1 | 4. lead_dispatches | pass | pass | continues | continues | 같음 | 5357ms | 1236ms |
| d2 | 1. lead_routes | pass | pass | continues | continues | 같음 | 1059ms | 124ms |
| d2 | 2. lead_writes_task | pass | pass | continues | continues | 같음 | 895ms | 145ms |
| d2 | 3. lead_routes | pass | pass | continues | continues | 같음 | 1047ms | 116ms |
| d2 | 4. lead_dispatches | deny | deny | stopped | stopped | 같음 | 1109ms | 92ms |
| d2 | 5. lead_writes_task | deny | deny | stopped | stopped | 같음 | 875ms | 53ms |
| d3 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6147ms | 1223ms |
| d3 | 2. worker_works | pass | pass | continues | continues | 같음 | 1487ms | 0ms |
| d3 | 3. edits_file | pass | pass | continues | continues | 같음 | 724ms | 27ms |
| d3 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| d3 | 5. worker_lints | pass | pass | continues | continues | 같음 | 1623ms | 435ms |
| d3 | 6. worker_finishes | pass | pass | continues | continues | 같음 | 2055ms | 1061ms |
| d3 | 7. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| d3 | 8. lead_waits | pass | pass | continues | continues | 같음 | 961ms | 321ms |
| d3 | 9. lead_dispatches | deny | deny | stopped | stopped | 같음 | 4729ms | 119ms |
| d3 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 653ms |
| d3 | 11. lead_dispatches | pass | pass | continues | continues | 같음 | 4618ms | 1525ms |
| d3 | 12. lead_merges | deny | pass | stopped | continues | 의도한 차이(DF-13) | 948ms | 1915ms |
| d3 | 13. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-13) | 2639ms | 1238ms |
| d4 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| d4 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| d4 | 3. lead_dispatches | pass | pass | continues | continues | 같음 | 6413ms | 1281ms |
| d4 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| d4 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| d4 | 6. lead_dispatches | pass | deny | continues | stopped | 의도한 차이(DF-13) | 5434ms | 966ms |
| d5 | 1. lead_routes | pass | pass | continues | continues | 같음 | 1050ms | 123ms |
| d5 | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 62ms |
| d5 | 3. lead_dispatches | deny | deny | stopped | stopped | 같음 | 1163ms | 54ms |
| d5 | 4. lead_dispatches | deny | deny | stopped | stopped | 같음 | 479ms | 54ms |
| d6 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6132ms | 1310ms |
| d6 | 2. lead_dispatches | pass | deny | continues | stopped | 의도한 차이(DF-8) | 3270ms | 124ms |
| d6 | 3. lead_dispatches | pass | pass | continues | continues | 같음 | 3192ms | 434ms |
| d6 | 4. lead_dispatches | pass | pass | continues | continues | 같음 | 3172ms | 447ms |
| d6 | 5. lead_dispatches | pass | human | continues | stopped | 의도한 차이(DF-8) | 3178ms | 136ms |
| d6 | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 100ms |
| d7-off | 1. spawns_subagent | deny | deny | stopped | stopped | 같음 | 400ms | 27ms |
| d7-off | 2. spawns_subagent | deny | deny | stopped | stopped | 같음 | 402ms | 25ms |
| d7-off | 3. spawns_subagent | deny | deny | stopped | stopped | 같음 | 421ms | 25ms |
| d7-off | 4. runs_workflow | pass | pass | continues | continues | 같음 | 0ms | 22ms |
| d7-on | 1. spawns_subagent | pass | pass | continues | continues | 같음 | 409ms | 26ms |
| d7-on | 2. spawns_subagent | deny | pass | stopped | continues | 의도한 차이(DF-22) | 435ms | 26ms |
| d7-on | 3. spawns_subagent | pass | pass | continues | continues | 같음 | 415ms | 27ms |
| d7-on | 4. runs_workflow | pass | pass | continues | continues | 같음 | 0ms | 24ms |
| d7-read | 1. spawns_subagent | deny | pass | stopped | continues | 의도한 차이(DF-6) | 410ms | 25ms |
| d7-read | 2. spawns_subagent | deny | deny | stopped | stopped | 같음 | 399ms | 26ms |
| d7-read | 3. spawns_subagent | deny | pass | stopped | continues | 의도한 차이(DF-6) | 398ms | 23ms |
| d7-read | 4. spawns_subagent | deny | pass | stopped | continues | 의도한 차이(DF-6) | 414ms | 23ms |
| d7-read | 5. spawns_subagent | deny | deny | stopped | stopped | 같음 | 404ms | 22ms |
| d7-read | 6. spawns_subagent | deny | deny | stopped | stopped | 같음 | 405ms | 22ms |
| d7-read | 7. spawns_subagent | deny | pass | stopped | continues | 의도한 차이(DF-6) | 411ms | 25ms |
| d8 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6066ms | 1230ms |
| d8 | 2. worker_subdispatches | pass | pass | continues | continues | 같음 | 2796ms | 156ms |
| d8 | 3. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 56ms |
| e1 | 1. writes_file | deny | deny | stopped | stopped | 같음 | 361ms | 27ms |
| e1 | 2. writes_file | pass | deny | continues | stopped | 의도한 차이(DF-9) | 730ms | 30ms |
| e1 | 3. lead_routes | pass | pass | continues | continues | 같음 | 1052ms | 116ms |
| e1 | 4. lead_writes_task | pass | pass | continues | continues | 같음 | 902ms | 137ms |
| e2 | 1. writes_file | deny | deny | stopped | stopped | 같음 | 404ms | 28ms |
| e2 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e2 | 3. deletes_file | deny | deny | stopped | stopped | 같음 | 375ms | 28ms |
| e2 | 4. writes_file | deny | deny | stopped | stopped | 같음 | 352ms | 28ms |
| e2 | 5. writes_file | pass | deny | continues | stopped | 의도한 차이(DF-30) | 714ms | 24ms |
| e2 | 6. edits_file | pass | pass | continues | continues | 같음 | 725ms | 23ms |
| e2 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e2 | 8. deletes_file | pass | pass | continues | continues | 같음 | 710ms | 23ms |
| e2 | 9. writes_file | pass | pass | continues | continues | 같음 | 722ms | 24ms |
| e2 | 10. writes_file | pass | pass | continues | continues | 같음 | 723ms | 23ms |
| e3 | 1. writes_file | deny | deny | stopped | stopped | 같음 | 357ms | 26ms |
| e3 | 2. writes_file | deny | deny | stopped | stopped | 같음 | 353ms | 29ms |
| e3 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e3 | 4. deletes_file | deny | deny | stopped | stopped | 같음 | 349ms | 24ms |
| e3 | 5. writes_file | deny | deny | stopped | stopped | 같음 | 358ms | 25ms |
| e3 | 6. writes_file | pass | deny | continues | stopped | 의도한 차이(DF-30) | 723ms | 25ms |
| e3 | 7. edits_file | pass | pass | continues | continues | 같음 | 728ms | 25ms |
| e3 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e3 | 9. deletes_file | pass | pass | continues | continues | 같음 | 773ms | 23ms |
| e3 | 10. writes_file | pass | pass | continues | continues | 같음 | 727ms | 23ms |
| e3 | 11. writes_file | pass | pass | continues | continues | 같음 | 712ms | 23ms |
| e4 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e4 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e4 | 3. edits_file | deny | deny | stopped | stopped | 같음 | 365ms | 27ms |
| e4 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e4 | 5. edits_file | deny | pass | stopped | continues | 의도한 차이(DF-31) | 355ms | 25ms |
| e4 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e4 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| e4 | 8. edits_file | pass | pass | continues | continues | 같음 | 715ms | 24ms |
| e4 | 9. lead_routes | pass | pass | continues | continues | 같음 | 1046ms | 119ms |
| e4 | 10. lead_writes_task | deny | deny | stopped | stopped | 같음 | 354ms | 54ms |
| e5 | 1. writes_file | deny | deny | stopped | stopped | 같음 | 365ms | 29ms |
| e5 | 2. writes_file | pass | pass | continues | continues | 같음 | 720ms | 26ms |
| e5 | 3. writes_file | pass | pass | continues | continues | 같음 | 706ms | 26ms |
| e5 | 4. writes_file | pass | pass | continues | continues | 같음 | 719ms | 25ms |
| f1 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f1 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f1 | 3. runs_shell | pass | pass | continues | continues | 같음 | 455ms | 77ms |
| f1 | 4. runs_shell | deny | deny | stopped | stopped | 같음 | 453ms | 69ms |
| f1 | 5. runs_shell | pass | pass | continues | continues | 같음 | 448ms | 62ms |
| f1 | 6. runs_shell | deny | deny | stopped | stopped | 같음 | 453ms | 64ms |
| f1 | 7. runs_shell | deny | deny | stopped | stopped | 같음 | 445ms | 65ms |
| f1 | 8. runs_shell | pass | pass | continues | continues | 같음 | 461ms | 63ms |
| f1 | 9. runs_shell | deny | deny | stopped | stopped | 같음 | 491ms | 64ms |
| f1 | 10. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f1 | 11. runs_shell | deny | deny | stopped | stopped | 같음 | 467ms | 65ms |
| f1 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f1 | 13. runs_shell | pass | pass | continues | continues | 같음 | 451ms | 66ms |
| f1 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f1 | 15. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f1 | 16. runs_shell | pass | pass | continues | continues | 같음 | 453ms | 74ms |
| f2 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| f2 | 2. writes_file | deny | deny | stopped | stopped | 같음 | 357ms | 32ms |
| f2 | 3. writes_file | deny | deny | stopped | stopped | 같음 | 356ms | 27ms |
| f2 | 4. writes_file | pass | pass | continues | continues | 같음 | 731ms | 24ms |
| f2 | 5. writes_file | pass | pass | continues | continues | 같음 | 717ms | 23ms |
| f2 | 6. writes_file | pass | pass | continues | continues | 같음 | 725ms | 23ms |
| g1 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 920ms | 37ms |
| g1 | 2. stops | pass | pass | continues | continues | 같음 | 828ms | 69ms |
| g1 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g1 | 4. stops | block | block | stopped | stopped | 같음 | 949ms | 107ms |
| g1 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g1 | 6. stops | block | block | stopped | stopped | 같음 | 966ms | 171ms |
| g1 | 7. stops | warn | warn | continues | continues | 같음 | 949ms | 173ms |
| g1 | 8. user_prompts | pass | pass | continues | continues | 같음 | 351ms | 31ms |
| g1 | 9. stops | block | block | stopped | stopped | 같음 | 965ms | 170ms |
| g1 | 10. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 908ms | 36ms |
| g1 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g1 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g1 | 13. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g1 | 14. stops | pass | pass | continues | continues | 같음 | 805ms | 229ms |
| g2 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 971ms | 38ms |
| g2 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g2 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g2 | 4. worker_lints | pass | pass | continues | continues | 같음 | 1632ms | 397ms |
| g2 | 5. stops | pass | pass | continues | continues | 같음 | 896ms | 166ms |
| g2 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g2 | 7. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g2 | 8. stops | pass | pass | continues | continues | 같음 | 1099ms | 236ms |
| g2 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g2 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g2 | 11. stops | block | block | stopped | stopped | 같음 | 1057ms | 243ms |
| g3 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 7. runs_fullops | deny | deny | stopped | stopped | 같음 | 1707ms | 402ms |
| g3 | 8. runs_fullops | deny | deny | stopped | stopped | 같음 | 1751ms | 399ms |
| g3 | 9. runs_fullops | deny | deny | stopped | stopped | 같음 | 1681ms | 397ms |
| g3 | 10. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 12. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g3 | 13. runs_fullops | pass | pass | continues | continues | 같음 | 1690ms | 423ms |
| g4 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 1002ms | 39ms |
| g4 | 2. stops | pass | pass | continues | continues | 같음 | 1062ms | 75ms |
| g4 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g4 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g4 | 5. stops | block | block | stopped | stopped | 같음 | 1290ms | 164ms |
| g4 | 6. stops | block | block | stopped | stopped | 같음 | 1222ms | 160ms |
| g4 | 7. stops | block | block | stopped | stopped | 같음 | 1239ms | 162ms |
| g5 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 1012ms | 38ms |
| g5 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g5 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| g5 | 4. stops | block | block | stopped | stopped | 같음 | 1306ms | 162ms |
| g5 | 5. runs_fullops | pass | pass | continues | continues | 같음 | 1629ms | 388ms |
| g5 | 6. stops | block | block | stopped | stopped | 같음 | 1606ms | 196ms |
| g5 | 7. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 153ms |
| g5 | 8. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 95ms |
| g5 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 99ms |
| g5 | 10. runs_fullops | pass | pass | continues | continues | 같음 | 1596ms | 257ms |
| g5 | 11. stops | pass | pass | continues | continues | 같음 | 1608ms | 195ms |
| h1 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 982ms | 38ms |
| h1 | 2. preamble_arrives | pass | pass | continues | continues | 같음 | 308ms | 35ms |
| h1 | 3. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h1 | 4. stops | block | block | stopped | stopped | 같음 | 858ms | 108ms |
| h1 | 5. stops | warn | warn | continues | continues | 같음 | 785ms | 95ms |
| h1 | 6. user_prompts | pass | pass | continues | continues | 같음 | 303ms | 31ms |
| h1 | 7. stops | block | block | stopped | stopped | 같음 | 790ms | 96ms |
| h2 | 1. preamble_arrives | pass | pass | continues | continues | 같음 | 318ms | 42ms |
| h2 | 2. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h2 | 3. stops | pass | pass | continues | continues | 같음 | 645ms | 91ms |
| h2 | 4. stops | pass | pass | continues | continues | 같음 | 617ms | 90ms |
| h3 | 1. preamble_arrives | pass | pass | continues | continues | 같음 | 312ms | 44ms |
| h3 | 2. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h3 | 3. stops | block | pass | stopped | continues | 의도한 차이(DF-32) | 5644ms | 3284ms |
| h3 | 4. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h3 | 5. stops | warn | pass | continues | continues | 의도한 차이(DF-32) | 644ms | 91ms |
| h3 | 6. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h3 | 7. stops | block | block | stopped | stopped | 같음 | 710ms | 104ms |
| h4 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6218ms | 1260ms |
| h4 | 2. worker_works | pass | pass | continues | continues | 같음 | 1501ms | 0ms |
| h4 | 3. worker_finishes | pass | deny | continues | stopped | 기준선 없음(12.2절 H4) | 2038ms | 350ms |
| h4 | 4. worker_finishes | deny | deny | stopped | stopped | 같음 | 1752ms | 340ms |
| h4 | 5. edits_file | pass | pass | continues | continues | 같음 | 713ms | 24ms |
| h4 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h4 | 7. worker_lints | deny | pass | stopped | continues | 기준선 없음(12.2절 H4) | 1664ms | 429ms |
| h4 | 8. worker_finishes | deny | pass | stopped | continues | 기준선 없음(12.2절 H4) | 1752ms | 1054ms |
| h4 | 9. worker_finishes | deny | pass | stopped | continues | 기준선 없음(12.2절 H4) | 1802ms | 451ms |
| h4 | 10. worker_finishes | deny | pass | stopped | continues | 기준선 없음(12.2절 H4) | 1736ms | 447ms |
| h5 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6067ms | 1237ms |
| h5 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 3. edits_file | pass | pass | continues | continues | 같음 | 723ms | 53ms |
| h5 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1003ms |
| h5 | 6. lead_dispatches | pass | pass | continues | continues | 같음 | 5381ms | 1222ms |
| h5 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 8. edits_file | pass | pass | continues | continues | 같음 | 713ms | 28ms |
| h5 | 9. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 994ms |
| h5 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1040ms | 121ms |
| h5 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 899ms | 142ms |
| h5 | 13. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 14. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 15. edits_file | pass | pass | continues | continues | 같음 | 721ms | 26ms |
| h5 | 16. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 17. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 989ms |
| h5 | 18. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 19. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| h5 | 20. lead_waits | pass | pass | continues | continues | 같음 | 978ms | 480ms |
| i1 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 917ms | 37ms |
| i1 | 2. lead_routes | pass | pass | continues | continues | 같음 | 1064ms | 123ms |
| i1 | 3. stops | block | block | stopped | stopped | 같음 | 1060ms | 75ms |
| i1 | 4. stops | warn | warn | continues | continues | 같음 | 1073ms | 65ms |
| i1 | 5. lead_plans | pass | pass | continues | continues | 같음 | 720ms | 58ms |
| i1 | 6. user_prompts | pass | pass | continues | continues | 같음 | 349ms | 31ms |
| i1 | 7. stops | block | pass | stopped | continues | 의도한 차이(DF-3) | 1020ms | 65ms |
| i2 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 934ms | 39ms |
| i2 | 2. lead_routes | pass | pass | continues | continues | 같음 | 1095ms | 122ms |
| i2 | 3. lead_writes_task | pass | pass | continues | continues | 같음 | 922ms | 135ms |
| i2 | 4. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i2 | 5. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i2 | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1218ms |
| i2 | 7. stops | pass | pass | continues | continues | 같음 | 1026ms | 138ms |
| i2 | 8. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 909ms | 38ms |
| i2 | 9. stops | pass | block | continues | stopped | 의도한 차이(DF-15) | 1025ms | 73ms |
| i2 | 10. stops | pass | warn | continues | continues | 의도한 차이(DF-15) | 1025ms | 66ms |
| i2 | 11. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i2 | 12. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 274ms |
| i2 | 13. user_prompts | pass | pass | continues | continues | 같음 | 358ms | 32ms |
| i2 | 14. stops | pass | block | continues | stopped | 의도한 차이(DF-15) | 992ms | 76ms |
| i3 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 927ms | 38ms |
| i3 | 2. lead_routes | pass | pass | continues | continues | 같음 | 1060ms | 127ms |
| i3 | 3. lead_writes_task | pass | pass | continues | continues | 같음 | 902ms | 168ms |
| i3 | 4. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i3 | 5. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i3 | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1252ms |
| i3 | 7. stops | pass | pass | continues | continues | 같음 | 1026ms | 137ms |
| i3 | 8. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 923ms | 38ms |
| i3 | 9. stops | pass | block | continues | stopped | 의도한 차이(DF-15) | 1011ms | 73ms |
| i3 | 10. stops | pass | warn | continues | continues | 의도한 차이(DF-15) | 1009ms | 65ms |
| i3 | 11. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i3 | 12. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 286ms |
| i3 | 13. user_prompts | pass | pass | continues | continues | 같음 | 357ms | 33ms |
| i3 | 14. stops | pass | block | continues | stopped | 의도한 차이(DF-15) | 1052ms | 77ms |
| i4-peek | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 924ms | 39ms |
| i4-peek | 2. lead_dispatches | pass | pass | continues | continues | 같음 | 6116ms | 1268ms |
| i4-peek | 3. worker_works | pass | pass | continues | continues | 같음 | 1496ms | 0ms |
| i4-peek | 4. edits_file | pass | pass | continues | continues | 같음 | 706ms | 26ms |
| i4-peek | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i4-peek | 6. worker_lints | pass | pass | continues | continues | 같음 | 1634ms | 425ms |
| i4-peek | 7. worker_finishes | pass | pass | continues | continues | 같음 | 2029ms | 1038ms |
| i4-peek | 8. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i4-peek | 9. runs_shell | pass | pass | continues | continues | 같음 | 829ms | 59ms |
| i4-peek | 10. stops | block | block | stopped | stopped | 같음 | 1108ms | 203ms |
| i4-peek | 11. lead_merges | deny | pass | stopped | continues | 의도한 차이(DF-13) | 922ms | 1735ms |
| i4-peek | 12. user_prompts | pass | pass | continues | continues | 같음 | 383ms | 33ms |
| i4-peek | 13. stops | block | pass | stopped | continues | 의도한 차이(DF-13) | 1103ms | 149ms |
| i4 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 920ms | 41ms |
| i4 | 2. worker_reports | pass | pass | continues | continues | 같음 | 13409ms | 440ms |
| i4 | 3. stops | block | block | stopped | stopped | 같음 | 1193ms | 200ms |
| i4 | 4. lead_merges | pass | pass | continues | continues | 같음 | 7560ms | 1726ms |
| i4 | 5. user_prompts | pass | pass | continues | continues | 같음 | 364ms | 33ms |
| i4 | 6. stops | pass | pass | continues | continues | 같음 | 1212ms | 151ms |
| i4 | 7. lead_dispatches | pass | pass | continues | continues | 같음 | 4361ms | 1167ms |
| i4 | 8. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i4 | 9. lead_waits | pass | pass | continues | continues | 같음 | 967ms | 219ms |
| i4 | 10. stops | block | block | stopped | stopped | 같음 | 1134ms | 135ms |
| i4 | 11. lead_waits | pass | pass | continues | continues | 같음 | 899ms | 131ms |
| i4 | 12. user_prompts | pass | pass | continues | continues | 같음 | 356ms | 34ms |
| i4 | 13. stops | block | pass | stopped | continues | 의도한 차이(DF-18) | 1120ms | 145ms |
| i4 | 14. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-18) | 4669ms | 1213ms |
| i4 | 15. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i4 | 16. lead_waits | pass | pass | continues | continues | 같음 | 938ms | 244ms |
| i4 | 17. stops | block | block | stopped | stopped | 같음 | 1352ms | 140ms |
| i4 | 18. lead_waits | pass | pass | continues | continues | 같음 | 934ms | 196ms |
| i4 | 19. user_prompts | pass | pass | continues | continues | 같음 | 347ms | 34ms |
| i4 | 20. stops | block | block | stopped | stopped | 같음 | 1129ms | 167ms |
| i4 | 21. lead_plans | pass | pass | continues | continues | 같음 | 720ms | 66ms |
| i4 | 22. stops | block | pass | stopped | continues | 의도한 차이(DF-18) | 1284ms | 130ms |
| i5 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6180ms | 1360ms |
| i5 | 2. stops | pass | block | continues | stopped | 의도한 차이(DF-13) | 866ms | 48ms |
| i5 | 3. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i5 | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 208ms |
| i5 | 5. stops | pass | pass | continues | continues | 같음 | 868ms | 67ms |
| i5 | 6. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i5 | 7. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 316ms |
| i6 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6149ms | 1222ms |
| i6 | 2. worker_works | pass | pass | continues | continues | 같음 | 1508ms | 0ms |
| i6 | 3. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i6 | 4. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i6 | 5. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i6 | 6. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i6 | 7. lead_waits | pass | pass | continues | continues | 같음 | 966ms | 250ms |
| i6 | 8. lead_waits | pass | pass | continues | continues | 같음 | 919ms | 156ms |
| i6 | 9. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i6 | 10. lead_waits | pass | pass | continues | continues | 같음 | 907ms | 252ms |
| i6 | 11. lead_waits | pass | pass | continues | continues | 같음 | 959ms | 167ms |
| i7 | 1. lead_routes | pass | pass | continues | continues | 같음 | 1055ms | 122ms |
| i7 | 2. lead_writes_task | pass | pass | continues | continues | 같음 | 892ms | 134ms |
| i7 | 3. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i7 | 4. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i7 | 5. runs_fullops | n/a | external | - | stopped | 기준선 없음 | 0ms | 1241ms |
| i7 | 6. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i7 | 7. runs_fullops | n/a | external | - | stopped | 기준선 없음 | 0ms | 439ms |
| i7 | 8. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| i7 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 429ms |
| i7 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 101ms |
| j1 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 914ms | 37ms |
| j1 | 2. worker_reports | pass | pass | continues | continues | 같음 | 13360ms | 442ms |
| j1 | 3. lead_waits | pass | pass | continues | continues | 같음 | 969ms | 162ms |
| j1 | 4. stops | block | block | stopped | stopped | 같음 | 1234ms | 204ms |
| j1 | 5. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j1 | 6. stops | block | block | stopped | stopped | 같음 | 1203ms | 263ms |
| j1 | 7. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j1 | 8. stops | pass | pass | continues | continues | 같음 | 1155ms | 167ms |
| j2 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 915ms | 39ms |
| j2 | 2. worker_reports | pass | pass | continues | continues | 같음 | 13396ms | 428ms |
| j2 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j2 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j2 | 5. stops | pass | pass | continues | continues | 같음 | 1153ms | 172ms |
| j3 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 943ms | 40ms |
| j3 | 2. worker_reports | pass | pass | continues | continues | 같음 | 13422ms | 443ms |
| j3 | 3. lead_waits | pass | pass | continues | continues | 같음 | 971ms | 163ms |
| j3 | 4. stops | block | block | stopped | stopped | 같음 | 1205ms | 204ms |
| j3 | 5. stops | block | block | stopped | stopped | 같음 | 1216ms | 190ms |
| j3 | 6. stops | block | block | stopped | stopped | 같음 | 1183ms | 195ms |
| j4 | 1. worker_reports | pass | pass | continues | continues | 같음 | 13410ms | 436ms |
| j4 | 2. stops | block | block | stopped | stopped | 같음 | 1007ms | 109ms |
| j4 | 3. stops | block | block | stopped | stopped | 같음 | 1020ms | 126ms |
| j4 | 4. runs_fullops | pass | pass | continues | continues | 같음 | 1346ms | 64ms |
| j4 | 5. stops | pass | pass | continues | continues | 같음 | 1029ms | 147ms |
| j4 | 6. lead_merges | deny | deny | stopped | stopped | 같음 | 1015ms | 60ms |
| j4 | 7. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j4 | 8. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j4 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 134ms |
| j4 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 72ms |
| j4 | 11. stops | pass | pass | continues | continues | 같음 | 975ms | 136ms |
| j5 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6182ms | 1212ms |
| j5 | 2. worker_works | pass | pass | continues | continues | 같음 | 1504ms | 0ms |
| j5 | 3. worker_lints | pass | pass | continues | continues | 같음 | 1655ms | 421ms |
| j5 | 4. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| j5 | 5. lead_waits | pass | pass | continues | continues | 같음 | 966ms | 246ms |
| j5 | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 164ms |
| j5 | 7. lead_merges | deny | pass | stopped | continues | 의도한 차이(DF-13) | 920ms | 1681ms |
| k1 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| k1 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 929ms | 39ms |
| k1 | 3. stops | block | block | stopped | stopped | 같음 | 1021ms | 79ms |
| k1 | 4. stops | warn | warn | continues | continues | 같음 | 1020ms | 66ms |
| k1 | 5. user_prompts | pass | pass | continues | continues | 같음 | 364ms | 32ms |
| k1 | 6. stops | block | block | stopped | stopped | 같음 | 997ms | 69ms |
| k1 | 7. stops | pass | pass | continues | continues | 같음 | 616ms | 26ms |
| k2 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| k2 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| k2 | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 936ms | 42ms |
| k2 | 4. stops | pass | pass | continues | continues | 같음 | 1032ms | 71ms |
| k2 | 5. lead_routes | pass | pass | continues | continues | 같음 | 1048ms | 124ms |
| k2 | 6. lead_writes_task | pass | pass | continues | continues | 같음 | 897ms | 138ms |
| k2 | 7. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| k2 | 8. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| k2 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1249ms |
| k2 | 10. lead_plans | pass | pass | continues | continues | 같음 | 705ms | 61ms |
| k2 | 11. stops | block | pass | stopped | continues | 의도한 차이(DF-3) | 1024ms | 130ms |
| k3 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| k3 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 924ms | 40ms |
| k3 | 3. stops | block | block | stopped | stopped | 같음 | 1031ms | 75ms |
| k4 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 912ms | 40ms |
| k4 | 2. user_prompts | pass | pass | continues | continues | 같음 | 354ms | 35ms |
| k4 | 3. lead_plans | pass | pass | continues | continues | 같음 | 742ms | 62ms |
| k4 | 4. lead_plans | pass | pass | continues | continues | 같음 | 746ms | 58ms |
| k4 | 5. lead_plans | pass | pass | continues | continues | 같음 | 710ms | 58ms |
| k4 | 6. worker_reports | pass | pass | continues | continues | 같음 | 13349ms | 490ms |
| k4 | 7. lead_merges | pass | pass | continues | continues | 같음 | 8677ms | 1999ms |
| k4 | 8. stops | block | block | stopped | stopped | 같음 | 1163ms | 161ms |
| k4 | 9. worker_reports | pass | pass | continues | continues | 같음 | 12468ms | 500ms |
| k4 | 10. lead_merges | pass | pass | continues | continues | 같음 | 9117ms | 1921ms |
| k4 | 11. stops | block | pass | stopped | continues | 기준선 없음(12.2절 K4) | 1206ms | 151ms |
| k4 | 12. stops | pass | pass | continues | continues | 같음 | 1157ms | 130ms |
| l1 | 1. worker_works | pass | pass | continues | continues | 같음 | 314ms | 0ms |
| l1 | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 222ms |
| l1 | 3. worker_works | pass | pass | continues | continues | 같음 | 198ms | 0ms |
| l1 | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 214ms |
| l1 | 5. worker_works | pass | pass | continues | continues | 같음 | 304ms | 0ms |
| l1 | 6. worker_works | pass | pass | continues | continues | 같음 | 183ms | 0ms |
| l1 | 7. worker_works | pass | pass | continues | continues | 같음 | 188ms | 0ms |
| l1 | 8. worker_works | pass | pass | continues | continues | 같음 | 190ms | 0ms |
| l1 | 9. worker_works | pass | pass | continues | continues | 같음 | 188ms | 0ms |
| l1 | 10. worker_works | pass | pass | continues | continues | 같음 | 192ms | 0ms |
| l1 | 11. worker_works | pass | pass | continues | continues | 같음 | 200ms | 0ms |
| l1 | 12. worker_works | pass | pass | continues | continues | 같음 | 184ms | 0ms |
| l1 | 13. worker_works | pass | pass | continues | continues | 같음 | 198ms | 0ms |
| l1 | 14. worker_works | pass | pass | continues | continues | 같음 | 191ms | 0ms |
| l1 | 15. worker_works | pass | pass | continues | continues | 같음 | 194ms | 0ms |
| l1 | 16. worker_works | pass | pass | continues | continues | 같음 | 192ms | 0ms |
| l1 | 17. worker_works | pass | pass | continues | continues | 같음 | 189ms | 0ms |
| l1 | 18. worker_works | pass | pass | continues | continues | 같음 | 191ms | 0ms |
| l1 | 19. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 226ms |
| l1 | 20. worker_works | pass | pass | continues | continues | 같음 | 191ms | 0ms |
| l1 | 21. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 215ms |
| l1 | 22. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 221ms |
| l1 | 23. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 24. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 25. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 26. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 27. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 28. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 225ms |
| l1 | 29. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 30. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 31. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l1 | 32. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 257ms |
| l2-decision | 1. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 94ms |
| l2-decision | 2. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 59ms |
| l2-decision | 3. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 57ms |
| l2-decision | 4. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 55ms |
| l2-decision | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 53ms |
| l2-decision | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 64ms |
| l2-decision | 7. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 50ms |
| l2-decision | 8. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 50ms |
| l2-decision | 9. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 50ms |
| l2 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6136ms | 1291ms |
| l2 | 2. worker_works | pass | pass | continues | continues | 같음 | 1495ms | 0ms |
| l2 | 3. edits_file | pass | pass | continues | continues | 같음 | 722ms | 27ms |
| l2 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l2 | 5. worker_lints | pass | pass | continues | continues | 같음 | 1713ms | 440ms |
| l2 | 6. worker_finishes | pass | pass | continues | continues | 같음 | 2028ms | 1047ms |
| l2 | 7. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l2 | 8. lead_waits | pass | pass | continues | continues | 같음 | 974ms | 258ms |
| l2 | 9. lead_merges | deny | human | stopped | stopped | 같음 | 936ms | 526ms |
| l2 | 10. lead_merges | deny | human | stopped | stopped | 같음 | 932ms | 521ms |
| l2 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 100ms |
| l2 | 12. lead_merges | deny | deny | stopped | stopped | 같음 | 943ms | 521ms |
| l2 | 13. runs_fullops | pass | pass | continues | continues | 같음 | 1573ms | 192ms |
| l2 | 14. lead_merges | deny | pass | stopped | continues | 의도한 차이(DF-13) | 926ms | 1876ms |
| l2 | 15. stops | block | pass | stopped | continues | 의도한 차이(DF-13) | 904ms | 41ms |
| l3 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6239ms | 1209ms |
| l3 | 2. worker_works | pass | pass | continues | continues | 같음 | 1541ms | 0ms |
| l3 | 3. edits_file | pass | pass | continues | continues | 같음 | 721ms | 29ms |
| l3 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l3 | 5. worker_lints | pass | pass | continues | continues | 같음 | 1657ms | 440ms |
| l3 | 6. worker_finishes | pass | pass | continues | continues | 같음 | 2054ms | 1042ms |
| l3 | 7. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l3 | 8. lead_waits | pass | pass | continues | continues | 같음 | 958ms | 260ms |
| l3 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 388ms |
| l3 | 10. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 265ms |
| l3 | 11. lead_merges | deny | deny | stopped | stopped | 같음 | 935ms | 529ms |
| l4 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6214ms | 1217ms |
| l4 | 2. worker_works | pass | pass | continues | continues | 같음 | 1497ms | 0ms |
| l4 | 3. edits_file | pass | pass | continues | continues | 같음 | 713ms | 26ms |
| l4 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l4 | 5. worker_lints | pass | pass | continues | continues | 같음 | 1655ms | 440ms |
| l4 | 6. worker_finishes | pass | pass | continues | continues | 같음 | 2045ms | 1056ms |
| l4 | 7. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l4 | 8. lead_waits | pass | pass | continues | continues | 같음 | 970ms | 291ms |
| l4 | 9. lead_merges | deny | human | stopped | stopped | 같음 | 930ms | 559ms |
| l4 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 99ms |
| l4 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 398ms |
| l4 | 12. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 579ms |
| l4 | 13. lead_dispatches | pass | pass | continues | continues | 같음 | 2189ms | 1497ms |
| l4 | 14. worker_works | pass | pass | continues | continues | 같음 | 1445ms | 0ms |
| l4 | 15. edits_file | pass | pass | continues | continues | 같음 | 763ms | 25ms |
| l4 | 16. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l4 | 17. worker_lints | pass | pass | continues | continues | 같음 | 1687ms | 442ms |
| l4 | 18. worker_finishes | pass | pass | continues | continues | 같음 | 2043ms | 1084ms |
| l4 | 19. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l4 | 20. lead_waits | pass | pass | continues | continues | 같음 | 941ms | 257ms |
| l4 | 21. runs_fullops | n/a | human | - | stopped | 기준선 없음 | 0ms | 532ms |
| l5-conflict | 1. worker_reports | pass | pass | continues | continues | 같음 | 13381ms | 429ms |
| l5-conflict | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-conflict | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-conflict | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-conflict | 5. lead_merges | human | human | stopped | stopped | 같음 | 3957ms | 771ms |
| l5-conflict | 6. lead_merges | human | deny | stopped | stopped | 같음 | 3915ms | 117ms |
| l5-conflict | 7. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 178ms |
| l5-conflict | 8. runs_fullops | pass | pass | continues | continues | 같음 | 1333ms | 62ms |
| l5-lint-fail | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 5. worker_reports | pass | pass | continues | continues | 같음 | 13494ms | 431ms |
| l5-lint-fail | 6. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 7. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 8. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-lint-fail | 9. lead_merges | deny | deny | stopped | stopped | 같음 | 5996ms | 1232ms |
| l5-lint-fail | 10. runs_fullops | pass | pass | continues | continues | 같음 | 1329ms | 71ms |
| l5-lint-fail | 11. stops | pass | pass | continues | continues | 같음 | 976ms | 108ms |
| l5-merge-commit | 1. worker_reports | pass | pass | continues | continues | 같음 | 13401ms | 441ms |
| l5-merge-commit | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-merge-commit | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-merge-commit | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-merge-commit | 5. lead_merges | pass | pass | continues | continues | 같음 | 9386ms | 2178ms |
| l5-push-fail | 1. worker_reports | pass | pass | continues | continues | 같음 | 13408ms | 441ms |
| l5-push-fail | 2. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-push-fail | 3. lead_merges | human | external | stopped | stopped | 같음 | 3598ms | 1129ms |
| l5-push-fail | 4. stops | block | block | stopped | stopped | 같음 | 1075ms | 112ms |
| l5-push-fail | 5. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-push-fail | 6. lead_merges | pass | pass | continues | continues | 같음 | 7600ms | 1124ms |
| l5-push-fail | 7. stops | pass | pass | continues | continues | 같음 | 993ms | 84ms |
| l5-rerun | 1. worker_reports | pass | pass | continues | continues | 같음 | 13359ms | 445ms |
| l5-rerun | 2. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-rerun | 3. lead_merges | pass | pass | continues | continues | 같음 | 7589ms | 1623ms |
| l5-rerun | 4. worker_reports | pass | pass | continues | continues | 같음 | 12553ms | 445ms |
| l5-rerun | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1339ms |
| l5-rerun | 6. stops | block | block | stopped | stopped | 같음 | 1027ms | 110ms |
| l5-rerun | 7. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| l5-rerun | 8. lead_merges | pass | pass | continues | continues | 같음 | 7575ms | 948ms |
| l5-rerun | 9. stops | pass | pass | continues | continues | 같음 | 1001ms | 87ms |
| l5 | 1. worker_reports | pass | pass | continues | continues | 같음 | 13427ms | 444ms |
| l5 | 2. stops | block | block | stopped | stopped | 같음 | 1011ms | 111ms |
| l5 | 3. lead_merges | pass | pass | continues | continues | 같음 | 7698ms | 1723ms |
| l5 | 4. stops | pass | pass | continues | continues | 같음 | 1000ms | 155ms |
| l5 | 5. lead_merges | deny | deny | stopped | stopped | 같음 | 963ms | 55ms |
| lint-anti-002 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 6. runs_fullops | deny | deny | stopped | stopped | 같음 | 1660ms | 366ms |
| lint-anti-002 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 9. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 10. runs_fullops | deny | deny | stopped | stopped | 같음 | 1729ms | 363ms |
| lint-anti-002 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 14. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-002 | 15. runs_fullops | pass | pass | continues | continues | 같음 | 1677ms | 414ms |
| lint-anti-003 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 6. runs_fullops | deny | deny | stopped | stopped | 같음 | 1676ms | 366ms |
| lint-anti-003 | 7. runs_fullops | deny | deny | stopped | stopped | 같음 | 1671ms | 350ms |
| lint-anti-003 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 11. runs_fullops | deny | deny | stopped | stopped | 같음 | 1667ms | 371ms |
| lint-anti-003 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 14. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-003 | 15. runs_fullops | pass | pass | continues | continues | 같음 | 1727ms | 393ms |
| lint-anti-004 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 6. runs_fullops | deny | deny | stopped | stopped | 같음 | 1669ms | 372ms |
| lint-anti-004 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 11. runs_fullops | deny | deny | stopped | stopped | 같음 | 1676ms | 373ms |
| lint-anti-004 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 15. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-004 | 16. runs_fullops | pass | pass | continues | continues | 같음 | 1664ms | 415ms |
| lint-anti-005 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1687ms | 407ms |
| lint-anti-005 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 9. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 10. runs_fullops | pass | pass | continues | continues | 같음 | 1657ms | 414ms |
| lint-anti-005 | 11. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 12. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-anti-005 | 13. runs_fullops | pass | pass | continues | continues | 같음 | 1663ms | 355ms |
| lint-commands | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 6. runs_fullops | pass | pass | continues | continues | 같음 | 1719ms | 429ms |
| lint-commands | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1696ms | 424ms |
| lint-commands | 8. runs_fullops | pass | pass | continues | continues | 같음 | 1773ms | 433ms |
| lint-commands | 9. runs_fullops | pass | pass | continues | continues | 같음 | 1684ms | 437ms |
| lint-commands | 10. runs_fullops | pass | pass | continues | continues | 같음 | 1704ms | 434ms |
| lint-commands | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 12. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 13. runs_fullops | pass | pass | continues | continues | 같음 | 1713ms | 452ms |
| lint-commands | 14. runs_fullops | pass | pass | continues | continues | 같음 | 1705ms | 449ms |
| lint-commands | 15. runs_fullops | pass | pass | continues | continues | 같음 | 1701ms | 462ms |
| lint-commands | 16. runs_fullops | pass | pass | continues | continues | 같음 | 1675ms | 453ms |
| lint-commands | 17. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 18. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1814ms | 501ms |
| lint-commands | 20. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 21. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 22. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 23. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 24. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 25. runs_fullops | deny | deny | stopped | stopped | 같음 | 1786ms | 464ms |
| lint-commands | 26. runs_fullops | deny | deny | stopped | stopped | 같음 | 1708ms | 413ms |
| lint-commands | 27. runs_fullops | deny | deny | stopped | stopped | 같음 | 1709ms | 397ms |
| lint-commands | 28. runs_fullops | deny | deny | stopped | stopped | 같음 | 1681ms | 409ms |
| lint-commands | 29. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 30. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 31. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 32. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 33. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 34. runs_fullops | deny | deny | stopped | stopped | 같음 | 1688ms | 399ms |
| lint-commands | 35. runs_fullops | deny | deny | stopped | stopped | 같음 | 1677ms | 376ms |
| lint-commands | 36. runs_fullops | deny | deny | stopped | stopped | 같음 | 1666ms | 388ms |
| lint-commands | 37. runs_fullops | deny | deny | stopped | stopped | 같음 | 1746ms | 377ms |
| lint-commands | 38. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 39. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 40. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 41. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 42. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 43. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 44. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-commands | 45. runs_fullops | pass | deny | continues | stopped | 의도한 차이(DF-34) | 61725ms | 2547ms |
| lint-commands | 46. runs_fullops | pass | deny | continues | stopped | 의도한 차이(DF-34) | 61656ms | 2536ms |
| lint-dep-001 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1654ms | 385ms |
| lint-dep-001 | 8. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 9. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 11. runs_fullops | pass | pass | continues | continues | 같음 | 1663ms | 383ms |
| lint-dep-001 | 12. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 13. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 14. runs_fullops | pass | pass | continues | continues | 같음 | 1635ms | 352ms |
| lint-dep-001 | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 16. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 17. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 18. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-dep-001 | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1736ms | 382ms |
| lint-doc-001 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1677ms | 403ms |
| lint-doc-001 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 12. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-001 | 13. runs_fullops | pass | pass | continues | continues | 같음 | 1672ms | 405ms |
| lint-doc-002 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 7. runs_fullops | deny | deny | stopped | stopped | 같음 | 1773ms | 363ms |
| lint-doc-002 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 9. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 10. runs_fullops | deny | deny | stopped | stopped | 같음 | 1653ms | 351ms |
| lint-doc-002 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 13. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-002 | 14. runs_fullops | pass | pass | continues | continues | 같음 | 1666ms | 396ms |
| lint-doc-003 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 6. runs_fullops | deny | deny | stopped | stopped | 같음 | 1674ms | 350ms |
| lint-doc-003 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 8. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 9. runs_fullops | deny | deny | stopped | stopped | 같음 | 1634ms | 342ms |
| lint-doc-003 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 16. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 17. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 18. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-doc-003 | 19. runs_fullops | deny | pass | stopped | continues | 의도한 차이(DF-33) | 1752ms | 419ms |
| lint-lint-000 | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 4. runs_fullops | pass | pass | continues | continues | 같음 | 1635ms | 346ms |
| lint-lint-000 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 7. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 9. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-000 | 10. runs_fullops | pass | pass | continues | continues | 같음 | 1658ms | 394ms |
| lint-lint-001 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 6. runs_fullops | pass | pass | continues | continues | 같음 | 1734ms | 403ms |
| lint-lint-001 | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1661ms | 372ms |
| lint-lint-001 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-001 | 11. runs_fullops | pass | pass | continues | continues | 같음 | 1686ms | 396ms |
| lint-lint-002-slow | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 2. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 5. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 7. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002-slow | 8. runs_fullops | pass | pass | continues | continues | 같음 | 122658ms | 121402ms |
| lint-lint-002 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-lint-002 | 6. runs_fullops | pass | pass | continues | continues | 같음 | 1700ms | 424ms |
| lint-rules | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 5. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 8. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 9. runs_fullops | deny | deny | stopped | stopped | 같음 | 1641ms | 340ms |
| lint-rules | 10. runs_fullops | deny | deny | stopped | stopped | 같음 | 1612ms | 335ms |
| lint-rules | 11. runs_fullops | deny | deny | stopped | stopped | 같음 | 1622ms | 335ms |
| lint-rules | 12. runs_fullops | deny | deny | stopped | stopped | 같음 | 1697ms | 330ms |
| lint-rules | 13. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 16. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 17. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 18. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 19. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 20. runs_fullops | pass | pass | continues | continues | 같음 | 1637ms | 381ms |
| lint-rules | 21. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 22. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 23. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 24. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 25. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-rules | 26. runs_fullops | deny | deny | stopped | stopped | 같음 | 1443ms | 340ms |
| lint-rules | 27. runs_fullops | deny | deny | stopped | stopped | 같음 | 1439ms | 328ms |
| lint-sec-001 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 6. runs_fullops | deny | deny | stopped | stopped | 같음 | 1638ms | 366ms |
| lint-sec-001 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 11. runs_fullops | pass | pass | continues | continues | 같음 | 1696ms | 384ms |
| lint-sec-001 | 12. runs_fullops | pass | pass | continues | continues | 같음 | 1635ms | 402ms |
| lint-sec-001 | 13. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 15. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-sec-001 | 16. runs_fullops | pass | pass | continues | continues | 같음 | 1629ms | 391ms |
| lint-size-001 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 3. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 4. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1646ms | 393ms |
| lint-size-001 | 8. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 11. runs_fullops | pass | pass | continues | continues | 같음 | 1627ms | 379ms |
| lint-size-001 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 15. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-001 | 16. runs_fullops | pass | pass | continues | continues | 같음 | 1678ms | 393ms |
| lint-size-002 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 3. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 6. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 7. runs_fullops | pass | pass | continues | continues | 같음 | 1651ms | 386ms |
| lint-size-002 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| lint-size-002 | 11. runs_fullops | pass | pass | continues | continues | 같음 | 1863ms | 385ms |
| m1 | 1. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| m1 | 2. preamble_arrives | pass | pass | continues | continues | 같음 | 320ms | 42ms |
| m1 | 3. turn_fails | pass | pass | continues | continues | 같음 | 335ms | 56ms |
| m1 | 4. turn_fails | pass | pass | continues | continues | 같음 | 298ms | 24ms |
| m1 | 5. turn_fails | pass | pass | continues | continues | 같음 | 311ms | 24ms |
| m2 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 907ms | 38ms |
| m2 | 2. user_prompts | pass | pass | continues | continues | 같음 | 353ms | 34ms |
| m2 | 3. user_prompts | pass | pass | continues | continues | 같음 | 324ms | 29ms |
| m2 | 4. user_prompts | pass | pass | continues | continues | 같음 | 348ms | 29ms |
| m2 | 5. preamble_arrives | pass | pass | continues | continues | 같음 | 305ms | 28ms |
| n1 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n1 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n1 | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 899ms | 46ms |
| n1 | 4. runs_shell | pass | pass | continues | continues | 같음 | 350ms | 24ms |
| n1 | 5. runs_shell | deny | deny | stopped | stopped | 같음 | 349ms | 27ms |
| n1 | 6. stops | block | pass | stopped | continues | 기준선 없음(12.2절 N1) | 992ms | 61ms |
| n1 | 7. stops | pass | pass | continues | continues | 같음 | 626ms | 25ms |
| n2 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1627ms |
| n2 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n2 | 12. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 904ms | 1576ms |
| n3 | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 12. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 13. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 101ms | 72ms |
| n3 | 14. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 221ms | 254ms |
| n3 | 15. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 16. runs_shell | pass | deny | continues | stopped | 기준선 없음(12.2절 N3) | 213ms | 27ms |
| n3 | 17. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 23ms | 21ms |
| n3 | 18. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 128ms | 90ms |
| n3 | 19. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| n3 | 20. runs_shell | deny | pass | stopped | continues | 기준선 없음(12.2절 N3) | 369ms | 21ms |
| o1-codex | 1. runs_shell | deny | deny | stopped | stopped | 같음 | 351ms | 25ms |
| o1-codex | 2. writes_file | pass | deny | continues | stopped | 의도한 차이(DF-9) | 718ms | 27ms |
| o1-codex | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 915ms | 33ms |
| o1-codex | 4. turn_fails | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o1 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o1 | 2. edits_file | deny | deny | stopped | stopped | 같음 | 365ms | 28ms |
| o1 | 3. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 382ms | 38ms |
| o1 | 4. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o1 | 5. preamble_arrives | pass | pass | continues | continues | 같음 | 307ms | 29ms |
| o1 | 6. turn_fails | pass | pass | continues | continues | 같음 | 332ms | 52ms |
| o2 | 1. session_starts | pass | warn | continues | continues | 의도한 차이(DF-24) | 346ms | 35ms |
| o2 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o2 | 3. edits_file | pass | deny | continues | stopped | 의도한 차이(DF-30) | 706ms | 29ms |
| o2 | 4. runs_shell | deny | deny | stopped | stopped | 같음 | 357ms | 23ms |
| o2 | 5. spawns_subagent | deny | deny | stopped | stopped | 같음 | 396ms | 23ms |
| o2 | 6. session_starts | warn | warn | continues | continues | 같음 | 389ms | 37ms |
| o2 | 7. user_prompts | pass | pass | continues | continues | 같음 | 356ms | 31ms |
| o2 | 8. preamble_arrives | pass | pass | continues | continues | 같음 | 302ms | 30ms |
| o2 | 9. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o2 | 10. stops | pass | pass | continues | continues | 같음 | 571ms | 21ms |
| o2 | 11. stops | pass | pass | continues | continues | 같음 | 589ms | 24ms |
| o2 | 12. stops | block | block | stopped | stopped | 같음 | 649ms | 150ms |
| o2 | 13. stops | warn | pass | continues | continues | 의도한 차이(DF-24) | 633ms | 139ms |
| o3 | 1. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 19ms |
| o3 | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 19ms |
| o3 | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 19ms |
| o3 | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 20ms |
| o3 | 5. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 901ms | 35ms |
| o3 | 6. runs_shell | deny | pass | stopped | continues | 기준선 없음(12.2절 O3) | 358ms | 19ms |
| o3 | 7. runs_shell | deny | deny | stopped | stopped | 같음 | 357ms | 23ms |
| o4 | 1. session_starts | n/a | context | - | continues | 기준선 없음 | 0ms | 36ms |
| o4 | 2. runs_shell | n/a | pass | - | continues | 기준선 없음 | 0ms | 24ms |
| o4 | 3. runs_shell | n/a | deny | - | stopped | 기준선 없음 | 0ms | 24ms |
| o4 | 4. writes_file | n/a | deny | - | stopped | 기준선 없음 | 0ms | 25ms |
| o4 | 5. preamble_arrives | n/a | pass | - | continues | 기준선 없음 | 0ms | 0ms |
| o4 | 6. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o4 | 7. stops | n/a | pass | - | continues | 기준선 없음 | 0ms | 21ms |
| o4 | 8. stops | n/a | block | - | stopped | 기준선 없음 | 0ms | 97ms |
| o4 | 9. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o4 | 10. turn_fails | n/a | pass | - | continues | 기준선 없음 | 0ms | 52ms |
| o4 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 21ms |
| o4 | 12. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 20ms |
| o4 | 13. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 19ms |
| o5 | 1. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| o5 | 2. preamble_arrives | n/a | pass | - | continues | 기준선 없음 | 0ms | 0ms |
| o5 | 3. runs_shell | n/a | pass | - | continues | 기준선 없음 | 0ms | 40ms |
| o5 | 4. runs_shell | n/a | pass | - | continues | 기준선 없음 | 0ms | 24ms |
| o5 | 5. runs_shell | n/a | pass | - | continues | 기준선 없음 | 0ms | 23ms |
| o5 | 6. stops | n/a | block | - | stopped | 기준선 없음 | 0ms | 96ms |
| o5 | 7. stops | n/a | pass | - | continues | 기준선 없음 | 0ms | 92ms |
| o5 | 8. user_prompts | n/a | pass | - | continues | 기준선 없음 | 0ms | 0ms |
| o5 | 9. stops | n/a | block | - | stopped | 기준선 없음 | 0ms | 94ms |
| o5 | 10. user_prompts | n/a | pass | - | continues | 기준선 없음 | 0ms | 0ms |
| o5 | 11. runs_shell | n/a | pass | - | continues | 기준선 없음 | 0ms | 39ms |
| p1 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 5967ms | 1212ms |
| p1 | 2. worker_works | pass | pass | continues | continues | 같음 | 1517ms | 0ms |
| p1 | 3. edits_file | pass | pass | continues | continues | 같음 | 744ms | 24ms |
| p1 | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p1 | 5. worker_lints | pass | pass | continues | continues | 같음 | 1614ms | 437ms |
| p1 | 6. worker_finishes | pass | pass | continues | continues | 같음 | 2022ms | 1028ms |
| p1 | 7. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p1 | 8. lead_waits | pass | pass | continues | continues | 같음 | 934ms | 247ms |
| p1 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 54ms |
| p1 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 48ms |
| p1 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 370ms |
| p1 | 12. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 264ms |
| p10 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 13. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 14. lead_routes | pass | pass | continues | continues | 같음 | 1106ms | 118ms |
| p10 | 15. lead_writes_task | pass | pass | continues | continues | 같음 | 887ms | 128ms |
| p10 | 16. edits_file | pass | pass | continues | continues | 같음 | 715ms | 23ms |
| p10 | 17. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 18. runs_fullops | deny | deny | stopped | stopped | 같음 | 942ms | 85ms |
| p10 | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1379ms | 175ms |
| p10 | 20. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 21. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 22. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 23. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p10 | 24. runs_fullops | pass | pass | continues | continues | 같음 | 1503ms | 287ms |
| p11 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1043ms | 118ms |
| p11 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 880ms | 135ms |
| p11 | 13. edits_file | pass | pass | continues | continues | 같음 | 708ms | 23ms |
| p11 | 14. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 347ms |
| p11 | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 16. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p11 | 17. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 85ms |
| p11 | 18. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 345ms |
| p11 | 19. deletes_file | pass | pass | continues | continues | 같음 | 696ms | 21ms |
| p11 | 20. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 344ms |
| p12 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 11. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 12. lead_routes | pass | pass | continues | continues | 같음 | 1047ms | 117ms |
| p12 | 13. lead_writes_task | pass | pass | continues | continues | 같음 | 884ms | 130ms |
| p12 | 14. edits_file | pass | pass | continues | continues | 같음 | 726ms | 23ms |
| p12 | 15. runs_fullops | pass | pass | continues | continues | 같음 | 1782ms | 435ms |
| p12 | 16. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 3824ms | 1469ms |
| p12 | 17. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p12 | 18. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1050ms |
| p12 | 19. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 94ms |
| p12 | 20. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 2737ms | 465ms |
| p13 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1037ms | 120ms |
| p13 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 886ms | 133ms |
| p13 | 13. edits_file | pass | pass | continues | continues | 같음 | 706ms | 23ms |
| p13 | 14. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 354ms |
| p13 | 15. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 83ms |
| p13 | 16. runs_fullops | pass | pass | continues | continues | 같음 | 1193ms | 172ms |
| p13 | 17. runs_fullops | pass | pass | continues | continues | 같음 | 999ms | 85ms |
| p13 | 18. runs_fullops | pass | pass | continues | continues | 같음 | 1784ms | 352ms |
| p13 | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1640ms | 254ms |
| p13 | 20. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 21. runs_fullops | deny | deny | stopped | stopped | 같음 | 1618ms | 250ms |
| p13 | 22. runs_fullops | deny | deny | stopped | stopped | 같음 | 946ms | 93ms |
| p13 | 23. runs_fullops | pass | pass | continues | continues | 같음 | 1658ms | 304ms |
| p13 | 24. runs_fullops | pass | pass | continues | continues | 같음 | 1133ms | 176ms |
| p13 | 25. runs_fullops | deny | deny | stopped | stopped | 같음 | 1660ms | 254ms |
| p13 | 26. runs_fullops | pass | pass | continues | continues | 같음 | 1668ms | 291ms |
| p13 | 27. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 28. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 84ms |
| p13 | 29. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 350ms |
| p13 | 30. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 31. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 83ms |
| p13 | 32. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 317ms |
| p13 | 33. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p13 | 34. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 303ms |
| p13 | 35. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 81ms |
| p14 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p14 | 2. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p14 | 3. lead_routes | pass | pass | continues | continues | 같음 | 1039ms | 119ms |
| p14 | 4. lead_writes_task | pass | pass | continues | continues | 같음 | 882ms | 139ms |
| p14 | 5. edits_file | pass | pass | continues | continues | 같음 | 718ms | 23ms |
| p14 | 6. runs_fullops | pass | pass | continues | continues | 같음 | 1771ms | 412ms |
| p14 | 7. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 83ms |
| p14 | 8. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p14 | 9. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 88ms |
| p14 | 10. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p14 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 87ms |
| p14 | 12. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 3762ms | 1343ms |
| p2 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 12. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p2 | 13. lead_routes | pass | pass | continues | continues | 같음 | 1039ms | 120ms |
| p2 | 14. lead_writes_task | pass | pass | continues | continues | 같음 | 902ms | 132ms |
| p2 | 15. edits_file | pass | pass | continues | continues | 같음 | 709ms | 23ms |
| p2 | 16. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 50ms |
| p2 | 17. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 347ms |
| p2 | 18. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 352ms |
| p2 | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1181ms | 178ms |
| p2 | 20. runs_fullops | pass | pass | continues | continues | 같음 | 1771ms | 288ms |
| p2 | 21. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 86ms |
| p2 | 22. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 3900ms | 1569ms |
| p2 | 23. edits_file | pass | pass | continues | continues | 같음 | 722ms | 25ms |
| p2 | 24. edits_file | pass | pass | continues | continues | 같음 | 705ms | 28ms |
| p2 | 25. edits_file | pass | pass | continues | continues | 같음 | 767ms | 24ms |
| p2 | 26. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 90ms |
| p2 | 27. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 2776ms | 1186ms |
| p2 | 28. edits_file | pass | pass | continues | continues | 같음 | 698ms | 24ms |
| p2 | 29. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 56ms |
| p2 | 30. lead_dispatches | deny | deny | stopped | stopped | 같음 | 2762ms | 1103ms |
| p2 | 31. runs_fullops | deny | deny | stopped | stopped | 같음 | 1681ms | 328ms |
| p2 | 32. runs_fullops | pass | pass | continues | continues | 같음 | 1646ms | 292ms |
| p2 | 33. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 86ms |
| p2 | 34. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 2860ms | 1251ms |
| p3 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1027ms | 118ms |
| p3 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 903ms | 135ms |
| p3 | 13. edits_file | pass | pass | continues | continues | 같음 | 723ms | 23ms |
| p3 | 14. runs_fullops | pass | pass | continues | continues | 같음 | 1863ms | 427ms |
| p3 | 15. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 91ms |
| p3 | 16. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 17. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 90ms |
| p3 | 18. runs_fullops | deny | deny | stopped | stopped | 같음 | 1649ms | 246ms |
| p3 | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1730ms | 286ms |
| p3 | 20. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 87ms |
| p3 | 21. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 22. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 97ms |
| p3 | 23. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p3 | 24. lead_dispatches | deny | deny | stopped | stopped | 같음 | 3807ms | 1245ms |
| p3 | 25. runs_fullops | pass | pass | continues | continues | 같음 | 1751ms | 420ms |
| p3 | 26. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 84ms |
| p3 | 27. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 2868ms | 1260ms |
| p4 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1038ms | 120ms |
| p4 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 892ms | 132ms |
| p4 | 13. edits_file | pass | pass | continues | continues | 같음 | 709ms | 23ms |
| p4 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 16. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 17. runs_fullops | deny | deny | stopped | stopped | 같음 | 1821ms | 390ms |
| p4 | 18. runs_fullops | deny | deny | stopped | stopped | 같음 | 1605ms | 246ms |
| p4 | 19. runs_fullops | pass | pass | continues | continues | 같음 | 1672ms | 286ms |
| p4 | 20. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 85ms |
| p4 | 21. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 94ms |
| p4 | 22. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 3855ms | 1501ms |
| p4 | 23. worker_works | pass | pass | continues | continues | 같음 | 220ms | 0ms |
| p4 | 24. edits_file | pass | pass | continues | continues | 같음 | 719ms | 25ms |
| p4 | 25. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 26. worker_finishes | deny | deny | stopped | stopped | 같음 | 1893ms | 373ms |
| p4 | 27. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 28. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p4 | 29. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 95ms |
| p4 | 30. worker_finishes | deny | deny | stopped | stopped | 같음 | 1917ms | 374ms |
| p5 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1048ms | 123ms |
| p5 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 889ms | 133ms |
| p5 | 13. edits_file | pass | pass | continues | continues | 같음 | 720ms | 23ms |
| p5 | 14. runs_fullops | pass | pass | continues | continues | 같음 | 1823ms | 454ms |
| p5 | 15. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 87ms |
| p5 | 16. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 85ms |
| p5 | 17. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 3839ms | 1463ms |
| p5 | 18. worker_works | pass | pass | continues | continues | 같음 | 216ms | 0ms |
| p5 | 19. edits_file | pass | pass | continues | continues | 같음 | 725ms | 26ms |
| p5 | 20. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p5 | 21. worker_lints | pass | pass | continues | continues | 같음 | 1618ms | 424ms |
| p5 | 22. worker_finishes | deny | deny | stopped | stopped | 같음 | 1902ms | 377ms |
| p6 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p6 | 12. lead_routes | deny | pass | stopped | continues | 의도한 차이(DF-29) | 918ms | 119ms |
| p6 | 13. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 115ms |
| p6 | 14. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 50ms |
| p6 | 15. lead_writes_task | pass | pass | continues | continues | 같음 | 895ms | 132ms |
| p6 | 16. edits_file | pass | pass | continues | continues | 같음 | 719ms | 24ms |
| p6 | 17. lead_dispatches | deny | deny | stopped | stopped | 같음 | 3881ms | 1352ms |
| p6 | 18. lead_routes | pass | pass | continues | continues | 같음 | 1035ms | 118ms |
| p6 | 19. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 51ms |
| p6 | 20. lead_dispatches | pass | pass | continues | continues | 같음 | 4265ms | 1299ms |
| p6 | 21. runs_fullops | pass | pass | continues | continues | 같음 | 1787ms | 439ms |
| p6 | 22. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 87ms |
| p6 | 23. lead_dispatches | deny | pass | stopped | continues | 의도한 차이(DF-16) | 2864ms | 1281ms |
| p7 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 11. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 12. lead_routes | pass | pass | continues | continues | 같음 | 1048ms | 119ms |
| p7 | 13. lead_writes_task | pass | pass | continues | continues | 같음 | 886ms | 130ms |
| p7 | 14. edits_file | pass | pass | continues | continues | 같음 | 706ms | 25ms |
| p7 | 15. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 16. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p7 | 17. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 350ms |
| p7 | 18. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 222ms |
| p7 | 19. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 323ms |
| p8 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 16. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 17. lead_routes | pass | pass | continues | continues | 같음 | 1034ms | 120ms |
| p8 | 18. lead_writes_task | pass | pass | continues | continues | 같음 | 891ms | 131ms |
| p8 | 19. edits_file | pass | pass | continues | continues | 같음 | 719ms | 24ms |
| p8 | 20. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 21. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 22. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 360ms |
| p8 | 23. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 339ms |
| p8 | 24. runs_fullops | pass | pass | continues | continues | 같음 | 1182ms | 171ms |
| p8 | 25. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 26. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 27. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 329ms |
| p8 | 28. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p8 | 29. runs_fullops | pass | pass | continues | continues | 같음 | 1227ms | 202ms |
| p9 | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 10. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 11. lead_routes | pass | pass | continues | continues | 같음 | 1042ms | 119ms |
| p9 | 12. lead_writes_task | pass | pass | continues | continues | 같음 | 904ms | 133ms |
| p9 | 13. edits_file | pass | pass | continues | continues | 같음 | 713ms | 23ms |
| p9 | 14. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 345ms |
| p9 | 15. runs_fullops | n/a | error | - | stopped | 기준선 없음 | 0ms | 50ms |
| p9 | 16. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 17. curl_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| p9 | 18. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 327ms |
| q1 | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q1 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 28ms |
| q10 | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1748ms |
| q10 | 12. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 24ms |
| q10 | 13. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q10 | 14. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 60ms |
| q10 | 15. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 52ms |
| q2-conflict | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 11. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-conflict | 12. runs_fullops | n/a | external | - | stopped | 기준선 없음 | 0ms | 2307ms |
| q2-install | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 2401ms |
| q2-install | 12. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-install | 13. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 2439ms |
| q2-no-package | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-no-package | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-no-package | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1752ms |
| q2-skill-fail | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 11. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q2-skill-fail | 12. runs_fullops | n/a | external | - | stopped | 기준선 없음 | 0ms | 2415ms |
| q3 | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q3 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1953ms |
| q4 | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q4 | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 1626ms |
| q5-backup | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-backup | 13. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 2255ms |
| q5-backup | 14. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 2325ms |
| q5-no-npx | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q5-no-npx | 11. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 2034ms |
| q6 | 1. fakes | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 4. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 8. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 9. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 10. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 11. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 12. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 13. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 14. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 15. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 16. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 17. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 2140ms |
| q6 | 18. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 29ms |
| q6 | 19. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 51ms |
| q6 | 20. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 21. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 25ms |
| q6 | 22. host_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 23. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 45ms |
| q6 | 24. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q6 | 25. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 24ms |
| q6 | 26. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 54ms |
| q7-codex | 1. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q7-codex | 2. session_starts | context | context | continues | continues | 같음 | 915ms | 37ms |
| q7 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 914ms | 38ms |
| q7 | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q7 | 3. session_starts | context | context | continues | continues | 같음 | 915ms | 41ms |
| q7 | 4. session_starts | context | context | continues | continues | 같음 | 908ms | 38ms |
| q7 | 5. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| q7 | 6. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 921ms | 37ms |
| q8-agy | 1. session_starts | n/a | context | - | continues | 기준선 없음 | 0ms | 38ms |
| q8-codex | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 910ms | 37ms |
| q8-grok | 1. session_starts | pass | warn | continues | continues | 기준선 없음(12.2절 Q8) | 361ms | 36ms |
| q8-grok | 2. session_starts | pass | warn | continues | continues | 기준선 없음(12.2절 Q8) | 351ms | 40ms |
| q8 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 914ms | 39ms |
| q8 | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 922ms | 38ms |
| r1-codex | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 906ms | 39ms |
| r1-codex | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 919ms | 40ms |
| r1-codex | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 69ms |
| r1-grok | 1. session_starts | pass | warn | continues | continues | 의도한 차이(DF-24) | 357ms | 40ms |
| r1-grok | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 75ms |
| r1-off | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 914ms | 38ms |
| r1-off | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 919ms | 40ms |
| r1-off | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 68ms |
| r1-on-agent | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 924ms | 38ms |
| r1-on-agent | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 918ms | 37ms |
| r1-on-agent | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 66ms |
| r1-on | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 976ms | 39ms |
| r1-on | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 916ms | 38ms |
| r1-on | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 66ms |
| r1-ultra | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 912ms | 40ms |
| r1-ultra | 2. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 919ms | 42ms |
| r1-ultra | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 68ms |
| r2-off | 1. lead_routes | pass | pass | continues | continues | 같음 | 1069ms | 123ms |
| r2-off | 2. lead_writes_task | pass | pass | continues | continues | 같음 | 885ms | 135ms |
| r2-off | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 124ms |
| r2-off | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 119ms |
| r2-ultra | 1. lead_routes | pass | pass | continues | continues | 같음 | 1115ms | 119ms |
| r2-ultra | 2. lead_writes_task | pass | pass | continues | continues | 같음 | 899ms | 136ms |
| r2-ultra | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 123ms |
| r2-ultra | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 119ms |
| r2-ultra | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 119ms |
| r2-ultra | 6. lead_dispatches | pass | pass | continues | continues | 같음 | 4136ms | 1214ms |
| r2 | 1. lead_routes | pass | pass | continues | continues | 같음 | 1042ms | 121ms |
| r2 | 2. lead_writes_task | pass | pass | continues | continues | 같음 | 899ms | 136ms |
| r2 | 3. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 119ms |
| r2 | 4. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 118ms |
| r2 | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 121ms |
| r2 | 6. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 122ms |
| r2 | 7. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 121ms |
| r2 | 8. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 120ms |
| r2 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 121ms |
| r2 | 10. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 119ms |
| r2 | 11. lead_dispatches | pass | pass | continues | continues | 같음 | 4152ms | 1230ms |
| r3-off | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6053ms | 1232ms |
| r3-off | 2. worker_subdispatches | deny | deny | stopped | stopped | 같음 | 2383ms | 86ms |
| r3 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6069ms | 1219ms |
| r3 | 2. worker_subdispatches | pass | pass | continues | continues | 같음 | 2776ms | 159ms |
| r3 | 3. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 57ms |
| r3 | 4. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 54ms |
| r3 | 5. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 79ms |
| r3 | 6. runs_fullops | n/a | error | - | stopped | 기준선 없음 | 0ms | 50ms |
| r4 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6115ms | 1230ms |
| r4 | 2. worker_subdispatches | pass | pass | continues | continues | 같음 | 2772ms | 156ms |
| r4 | 3. worker_works | pass | pass | continues | continues | 같음 | 197ms | 0ms |
| r4 | 4. edits_file | pass | pass | continues | continues | 같음 | 711ms | 25ms |
| r4 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r4 | 6. worker_finishes | pass | deny | continues | stopped | 의도한 차이(DF-23) | 2039ms | 334ms |
| r4 | 7. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r4 | 8. preamble_arrives | pass | pass | continues | continues | 같음 | 315ms | 50ms |
| r4 | 9. stops | block | block | stopped | stopped | 같음 | 641ms | 37ms |
| r4 | 10. stops | block | pass | stopped | continues | 의도한 차이(DF-23) | 990ms | 28ms |
| r4 | 11. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r4 | 12. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 214ms |
| r4 | 13. worker_finishes | deny | deny | stopped | stopped | 같음 | 1748ms | 345ms |
| r4 | 14. stops | block | block | stopped | stopped | 같음 | 1064ms | 32ms |
| r4 | 15. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 131ms |
| r4 | 16. stops | warn | block | continues | stopped | 의도한 차이(DF-23) | 1056ms | 246ms |
| r4 | 17. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r4 | 18. worker_lints | deny | pass | stopped | continues | 의도한 차이(DF-33) | 1685ms | 422ms |
| r4 | 19. worker_finishes | deny | pass | stopped | continues | 의도한 차이(DF-23) | 1720ms | 1062ms |
| r4 | 20. orca_sends | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r4 | 21. lead_waits | pass | pass | continues | continues | 같음 | 972ms | 253ms |
| r4 | 22. stops | block | block | stopped | stopped | 같음 | 919ms | 112ms |
| r4 | 23. stops | block | block | stopped | stopped | 같음 | 918ms | 130ms |
| r5 | 1. lead_dispatches | pass | pass | continues | continues | 같음 | 6058ms | 1230ms |
| r5 | 2. worker_works | pass | pass | continues | continues | 같음 | 1500ms | 0ms |
| r5 | 3. worker_subdispatches | pass | pass | continues | continues | 같음 | 1479ms | 130ms |
| r5 | 4. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 975ms | 52ms |
| r5 | 5. preamble_arrives | pass | pass | continues | continues | 같음 | 313ms | 33ms |
| r5 | 6. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r5 | 7. orca_replies | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r5 | 8. stops | block | block | stopped | stopped | 같음 | 948ms | 56ms |
| r5 | 9. runs_fullops | n/a | deny | - | stopped | 기준선 없음 | 0ms | 54ms |
| r6-off | 1. spawns_subagent | deny | deny | stopped | stopped | 같음 | 413ms | 25ms |
| r6-off | 2. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 68ms |
| r6 | 1. spawns_subagent | pass | pass | continues | continues | 같음 | 412ms | 25ms |
| r6 | 2. spawns_subagent | deny | pass | stopped | continues | 기준선 없음(12.2절 R6) | 433ms | 26ms |
| r6 | 3. spawns_subagent | deny | pass | stopped | continues | 기준선 없음(12.2절 R6) | 430ms | 24ms |
| r6 | 4. runs_workflow | pass | pass | continues | continues | 같음 | 0ms | 24ms |
| r6 | 5. lead_dispatches | pass | pass | continues | continues | 같음 | 6117ms | 1216ms |
| r6 | 6. worker_subdispatches | pass | pass | continues | continues | 같음 | 2769ms | 158ms |
| r6 | 7. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 70ms |
| r6 | 8. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 64ms |
| r6 | 9. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 0ms | 64ms |
| r7 | 1. session_starts | warn | context | continues | continues | 의도한 차이(DF-12) | 930ms | 40ms |
| r7 | 2. spawns_subagent | deny | pass | stopped | continues | 의도한 차이(DF-22) | 426ms | 27ms |
| r7 | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r7 | 4. stops | block | block | stopped | stopped | 같음 | 991ms | 112ms |
| r7 | 5. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r7 | 6. stops | block | block | stopped | stopped | 같음 | 958ms | 173ms |
| r7 | 7. worker_lints | pass | pass | continues | continues | 같음 | 1688ms | 394ms |
| r7 | 8. stops | pass | pass | continues | continues | 같음 | 898ms | 171ms |
| r7 | 9. worker_finishes | n/a | pass | - | continues | 기준선 없음 | 0ms | 102ms |
| r7 | 10. runs_shell | deny | deny | stopped | stopped | 같음 | 362ms | 24ms |
| r8-full | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-full | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-full | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-full | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-full | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 217ms | 265ms |
| r8-lite | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-lite | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-lite | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-lite | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-lite | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 217ms | 264ms |
| r8-missing | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-missing | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-missing | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-missing | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-missing | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 217ms | 270ms |
| r8-off | 1. git | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-off | 2. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-off | 3. file | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-off | 4. commits | pass | pass | continues | continues | 같음 | 0ms | 0ms |
| r8-off | 5. runs_fullops | n/a | pass | - | continues | 기준선 없음 | 213ms | 261ms |
