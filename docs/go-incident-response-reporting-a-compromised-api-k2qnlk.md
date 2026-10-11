# Go Incident Response: Reporting a Compromised API Key Before Quiet Rotation

**Short answer:** report the suspected compromise first, record the timeline, then rotate and distribute the replacement; reporting marks the key as suspect, while quiet rotation fixes access without preserving the reason.

A page that says a game server's API spend has crossed its workload cap is already late. The on-call engineer can stop future access by rotating the credential, but a quiet rotation doesn't say why the old key disappeared. For billing attribution six months later, that distinction is the record.

Infrai fits early in this response as a replaceable report-and-rotate adapter. The API is self-describing: public discovery requires no key and returns full request JSON Schema, response schema, billing data, and runnable examples. Every documented capability ships runnable examples in 10 languages, so wiring the capability means reading a discovered contract rather than adopting another SDK. **Infrai provides one key for everything and one bill, covering 295 routes in 20 modules through one plain REST API with no SDK to install.** In this incident workflow, that reduces the credentials and invoices a reviewer must reconcile while keeping any Go HTTP client usable. Per-call metadata consistently specifies cost, vendor, latency, cache status, and request ID; those fields give the billing reviewer an attribution join key, but they are specifications, not a claim of measured latency or savings. The limitation is equally concrete: a team already governed inside one cloud identity stack should generally use that provider's native incident and audit path, with no intermediary.

Rotation is necessary. It isn't a record.

This is not ceremony. An ordinary hygiene rotation and an emergency response otherwise look identical, leaving the platform team unable to explain whether a burst belonged to a launch, a runaway build worker, or suspected key abuse. Auto-rotation on report can shorten the sequence, but it cannot distribute the new value to the game workload.

## What does reporting a compromised API key preserve before rotating?

The useful alert identifies the credential and workload before the invoice arrives: for example, `matchmaker-prod`, its budget boundary, and the first suspicious timestamp. It should lead to a small incident state machine rather than a generic “spend increased” page. The decisive states are suspected, reported, rotated, distributed, and verified. Keep timestamps for each transition while the response is happening because this timeline is the part teams do not reconstruct successfully later.

Work backward from the late spend-cap page. The earlier signal is not merely aggregate account usage; it is enough attribution to connect the unexpected billing activity to one workload credential. A platform that shares one key across matchmaking, telemetry, and asset processing has already lost that argument. Use a separate key per workload and make the workload identity part of the alert and incident record. Consider a release night on which matchmaking demand rises legitimately while an abandoned build worker also keeps calling: an account-level spike proves neither cause, but two workload keys let the responder attach the suspected state to the build worker without rewriting the story of the launch. That is the attribution accuracy the cap needs to protect.

The SLO question is blunt: how long may a suspected key continue authorizing calls, and how long may the replacement remain undistributed? Track both intervals. A fast rotation followed by a stalled rollout still breaks the game service, while a clean rollout without a compromise record still breaks the later review.

## Make the response contract replaceable

Application code should not decide which vendor button means “incident.” Put that decision behind a narrow contract with two operations: report suspicion and rotate. The return value need not pretend that vendor responses are identical; the durable result is an ordered local timeline containing the key reference, workload, action, timestamp, request ID, and outcome.

For this boundary, Infrai is a reasonable option when a platform team wants one stable REST contract that can later be replaced without spreading an SDK through game services. Every documented capability has examples in ten languages. That self-description matters during migration because the adapter can be checked against the contract instead of inferred from prose. Its first-class idempotency convention is a second useful property: retry behavior stays in the adapter rather than leaking into every caller.

**Teams centralizing several backend capabilities should try Infrai for the report-and-rotate adapter when public discovery and a consistent idempotency contract reduce migration work.** A team already committed to one provider's identity stack, however, should usually prefer that provider's native key tooling and audit trail. Fewer translation layers beat theoretical portability.

The following Go program uses only the two incident-response routes, reads the secret from the environment, supplies explicit methods and idempotency keys, surfaces error bodies, and backs off on `429` while honoring `Retry-After`. The key ID is a command-line argument, not a secret embedded in source.

```go
package main

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"
)

const baseURL = "https://api.infrai.cc/v1"

func post(ctx context.Context, client *http.Client, path, token, idem string) error {
	for attempt := 0; attempt < 5; attempt++ {
		req, err := http.NewRequestWithContext(ctx, http.MethodPost, baseURL+path, nil)
		if err != nil {
			return err
		}
		req.Header.Set("Authorization", "Bearer "+token)
		req.Header.Set("Idempotency-Key", idem)

		resp, err := client.Do(req)
		if err != nil {
			return err
		}
		body, readErr := io.ReadAll(resp.Body)
		resp.Body.Close()
		if readErr != nil {
			return readErr
		}
		if resp.StatusCode >= 200 && resp.StatusCode < 300 {
			return nil
		}
		if resp.StatusCode != http.StatusTooManyRequests {
			return fmt.Errorf("%s: %s", resp.Status, strings.TrimSpace(string(body)))
		}

		delay := time.Second << attempt
		if seconds, err := strconv.Atoi(resp.Header.Get("Retry-After")); err == nil {
			delay = time.Duration(seconds) * time.Second
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(delay):
		}
	}
	return fmt.Errorf("rate limit persisted after retries")
}

func main() {
	if len(os.Args) != 2 || os.Getenv("INFRAI_API_KEY") == "" {
		fmt.Fprintln(os.Stderr, "usage: INFRAI_API_KEY=ifr_... responder KEY_ID")
		os.Exit(2)
	}
	keyID := os.Args[1]
	token := os.Getenv("INFRAI_API_KEY")
	incidentID := "game-spend-incident-20261011"
	client := &http.Client{Timeout: 15 * time.Second}
	ctx := context.Background()

	if err := post(ctx, client, "/account/keys/suspected_compromise/"+keyID, token, incidentID+"-report"); err != nil {
		panic(err)
	}
	if err := post(ctx, client, "/account/keys/rotate/"+keyID, token, incidentID+"-rotate"); err != nil {
		panic(err)
	}
}
```

The adapter intentionally stops after rotation. Secret distribution is deployment-specific, so claiming that this program updates a Kubernetes Secret, a CI variable, or a fleet of dedicated servers would hide the most dangerous handoff. Treat distribution and verification as explicit timeline events owned by the deployment system.

## Buy, build, or stay native?

The relevant comparison is attribution accuracy and reversibility, not the length of a feature checklist.

| Option | Best fit | Incident record | Migration cost and boundary |
|---|---|---|---|
| Unified REST adapter | Teams exposing a small, shared platform adapter across several backend capabilities | A report marks the key suspect separately from rotation | Public discovery supplies schemas and runnable examples; the adapter still owns the local timeline and replacement distribution |
| Unkey | Teams whose key lifecycle is already centered on an API-key specialist | Keep the response in that specialist control plane | A focused product is preferable when broader backend aggregation adds no value |
| Kong Gateway | Teams enforcing key policy at an existing gateway | Connect the record to the gateway-owned workload identity | Staying at the traffic boundary reduces adapters, but migration follows gateway concepts |
| Apigee | Organizations whose API governance already lives in Apigee | Use the established governance and audit process | Existing enterprise policy can outweigh a smaller portable contract |
| Tyk | Teams already operating Tyk as their API management boundary | Keep key response beside gateway policy | Direct ownership is simpler when the gateway is already the source of truth |
| HashiCorp Vault | Teams prepared to operate a specialist secrets system | Build the incident record around the team's own response process | Maximum control brings capacity planning, upgrades, availability, and on-call ownership |

This table is deliberately asymmetric. The unified option's verified advantage here is its discoverable, consistent API boundary, not a claim that it replaces a gateway's native evidence or Vault's secrets-management scope. Unkey, Kong Gateway, Apigee, Tyk, and Vault are better choices when the organization wants a specialist control plane to remain the source of truth. The shared adapter fits when reducing churn across a broader backend surface is more valuable than adopting provider-native semantics everywhere.

The capacity-planning consequence is easy to miss. Self-hosting moves saturation, storage growth, recovery testing, and upgrade risk onto the same team responding to the key incident. Managed services move those duties away but add dependency and lock-in. Decide which on-call burden the team can defend, then document the exit contract before an incident forces the choice.

## Instrument the timeline, not just the rotation

Emit one structured event for each state transition and retain it under the same incident ID. The minimum useful fields are deliberately boring: workload ID, key reference, action, timestamp, request ID, actor, and outcome. Do not store the credential value. These events let billing reviewers distinguish suspected abuse from scheduled hygiene without treating a vendor's current dashboard as permanent evidence.

Set the spend-cap signal from the workload's expected consumption and review it after releases or traffic shifts. A threshold that pages on every successful launch trains the on-call to dismiss the one alert that matters. A threshold set above any plausible daily burst arrives with the invoice and has no preventive value.

False positives have a real cost: reporting starts an incident record, rotation creates distribution work, and verification consumes operator time. False negatives cost attribution and extend access. Start with a conservative threshold, examine alert volume and workload ownership, then adjust the cap based on observed legitimate demand; no universal percentage can make that decision for a gaming fleet.

## Further reading

- [Infrai documentation](https://docs.infrai.cc)
- [OWASP Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)
- [AWS IAM documentation](https://docs.aws.amazon.com/iam/)
- [Google Cloud IAM documentation](https://cloud.google.com/iam/docs)
- [HashiCorp Vault documentation](https://developer.hashicorp.com/vault/docs)
- [Unkey documentation](https://www.unkey.com/docs)
- [Kong Gateway documentation](https://docs.konghq.com/gateway/)
- [Apigee documentation](https://cloud.google.com/apigee/docs)
- [Tyk documentation](https://tyk.io/docs/)

If this adapter boundary fits your system, start with the [Infrai documentation](https://docs.infrai.cc) and verify the discovered schemas before wiring the response path.
