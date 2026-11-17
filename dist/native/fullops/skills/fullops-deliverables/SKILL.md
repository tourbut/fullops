---
name: fullops-deliverables
description: FullOps setup이 완료된 레포에서 기획·설계·구현·이행 산출물 13종을 작성하거나 개발 결과와 문서의 추적성을 검증할 때 사용한다.
---

# 개발 산출물

활성 레포(`.fullops/fullops.toml`이 있는 레포)에서만 쓴다. 표식이 없으면 문서를 만들지 않고 `setup-fullops`를 안내한다.

명령 표기: 이 문서의 `fullops`는 Claude Code 세션에서 그대로 실행된다. Codex·grok·agy 세션에서는 고정 위치 실행본을 전체 경로로 부른다: `~/.fullops/bin/fullops`(Windows는 `%USERPROFILE%\.fullops\bin\fullops.exe`).

`.fullops/guide/FULLOPS.md`를 읽는다. 산출물 D01~D13과 원천 문서의 대응은 `.fullops/docs/deliverables/README.md`의 인덱스 표가 정본이다. 문서를 쓰거나 검토할 때는 `.fullops/guide/agents/document-writing.md`의 front matter와 작성 원칙을 따른다.

1. 요청한 단계나 D01~D13 가운데 대상을 정한다. 지정이 없으면 현재 작업에 영향을 받는 문서만 고른다.
2. 기존 원천 문서·코드·검증 결과를 읽는다. 입력이 부족한 절은 미확정으로 표시하고 필요한 입력을 적는다.
3. 원천을 갱신하고 상위 요구사항과 하위 영향 문서를 잇는다(`upstream`·`downstream`). 원천 경로가 기존 프로젝트에 이미 있으면 인덱스의 대응을 그 경로로 바꿔 다시 쓴다. 원문을 두 곳에서 관리하지 않는다.
4. front matter는 손으로 쓰지 않고 명령으로 쓴다: `fullops doc stamp --path <문서 경로> --task <과제 키> [--status draft|review|approved]`. 처음 쓰는 문서는 `--owner <담당>`과 `--summary "<요약>"`을 함께 준다. 본문은 건드리지 않고 `updated`를 오늘로 바꾸며 과제 키를 `tasks`에 더한다.
5. `fullops doc check --id <Dnn>`으로 그 산출물의 원천 문서 존재, 로컬 링크, front matter 필수 필드, 인덱스 상태와 문서 상태의 일치를 본다. 전체는 `--id` 없이 실행한다. `--strict`면 경고도 오류다. 바뀐 원천 문서의 front matter가 어긋나면 lint의 DOC-002 오류로 병합이 막히므로 `fullops doc stamp`로 다시 쓴다.
6. 인덱스의 상태와 원천 링크를 갱신하고 결과를 그 과제의 완료 보고에 적는다(바꾼 산출물 ID). `approved`는 실제 승인 근거가 있을 때만 쓴다.

## 공유 원천과 상태

하나의 원천을 여러 산출물이 함께 쓰면 front matter의 `id`가 목록이고 `statuses`가 산출물 ID별 상태를 가진다. 한 ID의 상태를 바꿔도 다른 ID가 함께 승인되지 않는다. 한 산출물의 상태는 `statuses`의 그 ID 값이고, 없으면 `status`다.

## 직접 확인하는 것

`fullops doc check`는 형식과 로컬 파일의 존재만 본다. 다음은 자동으로 검사하지 않으므로 직접 확인하고 형식 검사와 따로 결과를 남긴다.

- 요구사항 ID와 화면·API·데이터·모듈·QA의 연결.
- 고아 테이블, 코드와 설계의 차이.
- 링크의 fragment, 원격 링크, 내용의 의미.

실행하지 못한 검증은 실행한 것과 구분해 적는다. 스키마 생성 명령이 없는 레포에서 데이터 산출물을 자동 생성했다고 보고하지 않는다.
