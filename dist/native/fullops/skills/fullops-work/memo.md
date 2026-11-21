# 역할 메모

`fullops-work`의 참조 파일이다. 과제를 끝내며 역할 메모에 남길 것이 있거나 `fullops doctor`가 메모를 알릴 때 읽는다.

`.fullops/guide/roles/<역할>.md`는 다음에 이 역할을 맡을 세션이 같은 실수를 피하고 같은 결정을 다시 하지 않게 하는 메모다. `task new`가 이 경로를 지시서의 `먼저 읽을 문서`에 적는다.

- 그 목적에 필요한 것만 적는다. 과제 경과·수치·경로는 `tasks/<KEY>/`에 이미 있으니 옮겨 적지 않는다.
- 읽은 메모가 이번 과제에서 틀렸으면 그 자리에서 고친다.
- 같은 실수가 두 번째면 `guide/project.md`의 `## 자주 틀리는 것`에 한 줄로 올린다. 정규식으로 잡을 수 있으면 `config/lint.json`의 `rules`에 규칙으로 넣고 메모 줄은 지운다. 승격은 lint 규칙에서 끝난다.
- 150줄이나 10KB를 넘거나 역할 ID가 아닌 파일이 생기면 `fullops doctor`가 알린다. 그러면 `fullops merge`가 정리 과제 행(`MEMO-TIDY-<날짜>`)을 `PLANS.md`에 한 번 더한다. 정리할 때 지난 전문은 `.fullops/archive/contexts/`에 둔다. 고칠 수 없는 경고만 남으면 그 행을 `fullops plan set <KEY> "보류(<사유>)"`로 둔다. 지우면 다시 생긴다.
