# 먼저 읽을 문서 찾기(선택)

`fullops-work`의 참조 파일이다. 지시서의 `먼저 읽을 문서`를 채울 후보가 필요할 때 읽는다.

`task new` 뒤, 배정 전에 쓴다. 둘 다 추천이고 건너뛰어도 배정이 막히지 않는다. 결과 파일은 커밋되지 않는 Git 공용 디렉터리(`<git-common-dir>/fullops/explore/`)에 생기고 모든 워크트리가 함께 본다. 고른 경로는 지시서의 "먼저 읽을 문서"에 적어 worker에게 전한다.

1. `fullops find <KEY> --role <역할>`로 코드 후보를, `fullops find <KEY> --role <역할> --scope documents`로 문서 후보를 찾는다.
2. `fullops context <KEY> --role <역할> --paths <경로>... [--required <경로>]...`로 후보 문서를 분류한다. 후보는 20개 이하로 좁혀 준다.
3. 같은 입력이면 호출 없이 이전 결과를 다시 쓴다. 입력이 달라졌으면 `--force`로 갱신한다(이전 결과는 `history/`에 남는다). Jev 키가 없거나 `FULLOPS_OFFLINE=1`이면 로컬 검색 후보를 돌려준다. 이때 의미 탐색을 했다고 보고하지 않는다.
