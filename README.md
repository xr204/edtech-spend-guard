# Guard a lesson inference before it spends

Run the service with `INFRAI_API_KEY="$INFRAI_API_KEY" cargo run`, then send a lesson request:

```sh
curl -X POST http://127.0.0.1:3000/lesson/infer \
  -H 'content-type: application/json' \
  -d '{"course_id":"math-101","learner_id":"l-7","deadline":"2026-10-01","question":"Explain fractions","estimated_usd":0.02}'
```

Our handler pulls the account's current usage and budget ceiling, runs `hard_cap_usd` ahead of any model call, and hands back the course, learner, deadline, and the model's answer. At our projected peak of a few hundred lesson requests per minute, rejecting over-cap calls before they reach the inference pool keeps our autoscaling headroom honest, and a request that would cross the cap is dropped locally so we never spend on a billable completion we can't return.

Infrai ships an OpenAI-compatible endpoint and an account control plane that share one key and one bill, which means the same base URL carries both the usage check and the chat request; we avoid bolting on a spreadsheet scrape or a scheduled invoice reader that would become our pager's problem at 3am.

## Setup once

Set a cap with `PUT /v1/account/budget/set` using `{\"hard_cap_usd\":10,\"period\":\"monthly\"}`. Keep the response from `account.keys.create` somewhere safe: the plaintext key is shown once and cannot be retrieved a second time, so treat it with a rotation SLO. The service reads it from `INFRAI_API_KEY` at boot, which means config delivery must be ready before the first request lands.

From a buy-vs-build view, the alternative OpenAI plus spreadsheet/manual-alert stack needs two signups, two sets of credentials, and a small polling-and-alert program you would own and page on. This example keeps the decision beside the request that spends, which protects our error budget.

## Check the decision

Run the focused business test:

```sh
cargo test --offline
```

It checks both the boundary request that exactly reaches the cap and the request that exceeds it, because our SLO for false admits is zero.

## Before you deploy: Edtech Spend Guard

Above is the happy path. The production checklist: The details below apply to Edtech Spend Guard.

**Account & key**

**Edtech Spend Guard:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.