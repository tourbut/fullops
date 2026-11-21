# 모델 고르기

`fullops-orca`의 참조 파일이다. `fullops route`나 `fullops dispatch`에 줄 `--agent --model --effort`를 고를 때 읽는다.

모델은 `fullops.toml`의 `[models].<역할>` 후보에서 고르고 `--agent --model --effort`를 셋 다 준다. 후보는 약한 것부터 적혀 있다. 범위와 완료 기준이 확정된 작은 작업은 앞쪽을, 설계 판단·여러 모듈·위험한 변경은 뒤쪽을 고른다. 사용자나 과제가 조합을 정했으면 그대로 쓰고, 목록 밖이면 `--why "<근거>"`를 붙인다(없으면 RT-06·D-14로 거절된다). 후보가 하나인 역할은 생략해도 dispatch가 그 후보를 쓴다. 후보가 없는 역할은 기획·설계에 고성능 모델을, 확정된 구현에 비용이 낮은 모델을 고르고, 정하지 못하면 사용자에게 묻는다. 리뷰 모델은 `reviewer` 후보에서 고르고, 없으면 가장 강한 구현 역할 후보를 쓴다. 분류 없이 모델만 기록하려면 `fullops route <KEY> --tuple-only --role <역할|reviewer> --agent … --model … --effort …`다.
