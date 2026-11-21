# 배정 거절과 복구

`fullops-orca`의 참조 파일이다. `fullops dispatch`가 거절됐을 때, 역할 워크트리나 환경 파일 연결이 빠졌을 때, worker가 도중에 끊겼을 때 읽는다.

## 자주 걸리는 거절

모델을 정하지 않았다(D-09. 거절 문구에 그 자리의 후보가 나온다), 목록 밖 조합에 `--why`가 없다(D-14), 분류가 없다(`fullops route`), 인박스가 없거나 키가 다르다(`fullops task new`), 다른 키의 통합 대기가 있다(`fullops merge <그 키>` 또는 `fullops hold`), 그 역할에 끝나지 않은 다른 과제가 있다(끝난 뒤 배정하거나 `fullops plan add`), 대상 워크트리가 더럽다(그 worker의 완료 뒤 `fullops sync --role <역할>`).

끝난 worker의 터미널은 Orca가 곧 놓는다. `--terminal`로 준 배정이 거절되면 `--terminal`을 빼고 다시 배정한다.

## 날것 git으로 통합한 경우

날것 `git merge`·`git push`로 통합해도 조상 관계로 인식한다. 원격에 이미 들어간 것을 한꺼번에 확인하려면 `fullops settle`을 쓴다.

## bootstrap

역할 워크트리는 Orca로 만든다. 역할 브랜치는 `fullops/<역할>`이고 `fullops init`이 원격에 만들어 둔다. 그 브랜치를 체크아웃한 워크트리가 없으면 배정이 거절된다. 역할과 책임은 `.fullops/fullops.toml`의 `[roles]`, 모델 후보는 `[models]`에 있다. 같은 worker에 터미널을 둘 띄우지 않는다. 진행 중인 worker를 다른 과제에 다시 쓰려면 그 과제가 끝나고 세션이 분리됐는지 먼저 확인한다. grok worker는 원본 레포의 폴더 신뢰가 필요하고 사용자가 grok에서 승인한다.

역할 워크트리의 환경 파일(`.fullops/.env*`)은 `fullops dispatch`와 `fullops task recover`가 주 체크아웃에서 연결한다. 배정 전에 그 워크트리에서 직접 쓸 일이 있거나 연결이 빠졌으면 그 워크트리에서 `fullops env link`를 실행한다. 추적 파일과 이미 있는 파일은 건드리지 않는다.

## 끊긴 worker 복구

worker가 도중에 끊겼으면(터미널 종료 등) 다시 배정하기 전에 그 역할의 워크트리를 살린다: `fullops task recover --role <역할> --key <KEY> --repo <역할 워크트리>`. 미커밋 변경을 WIP 커밋으로 보존하고 기본 브랜치를 반영한다. 충돌하면 병합을 물리고 종료 코드 3으로 끝나므로 사람이 정한다.
