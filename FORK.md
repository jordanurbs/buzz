# Fork Notes (jordanurbs/buzz)

This fork carries the homelab patch set for the self-hosted Buzz relay.
Upstream: `block/buzz` (https://github.com/block/buzz).

## Pinned Upstream

- Upstream base SHA: `745501962e68041b3aee5c83fd3ea3bf5a496dbc`
  (`main` as of 2026-10-08).
- Patch branch: `homelab/member-joined` (one commit on top of the pin).

## Patches

1. `homelab/member-joined` — `member_joined` workflow trigger for
   kind:40099 system messages (schema variant, engine kind match +
   content-type gate, relay-side `was_inserted`-gated workflow feed in
   `emit_system_message`, kind:40099 excluded from the client-event
   workflow feed). Intended for an upstream PR; if merged, this patch
   retires.

## Update Path

```bash
git fetch upstream
git checkout homelab/member-joined
git rebase upstream/main
# bump the pinned SHA above, rebuild the musl image, redeploy the relay
```

Keep the patch small and confined to the files listed in the commit.
Everything else tracks upstream `main`.
