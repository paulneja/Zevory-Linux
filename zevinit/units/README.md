# Service units

One file per unit, named `<name>.toml`. The unit's name is the file name without the suffix,
never a field inside: a file and the unit it declares can then never disagree.

Unknown keys are refused rather than ignored, so a typo fails at parse time instead of quietly
doing nothing.

## `[unit]`

| key | type | default | meaning |
|---|---|---|---|
| `description` | string | required | what `zevctl status` shows |
| `requires` | list | empty | hard dependency: if it fails, this unit does not start |
| `wants` | list | empty | soft dependency: tried first, failure does not stop us |
| `after` | list | empty | ordering only, no dependency implied |
| `before` | list | empty | the mirror of `after` |
| `conflicts` | list | empty | starting this one stops those |

`requires` and `wants` say *whether*; `after` and `before` say *when*. They are separate on
purpose: requiring a unit does not order against it, and ordering against a unit does not pull
it in.

## `[service]`

| key | type | default | meaning |
|---|---|---|---|
| `start` | string | required | the command that starts it |
| `stop` | string | none | how to ask it to stop; without one it gets a signal |
| `kind` | `simple` \| `oneshot` | `simple` | `oneshot` runs to completion, `simple` keeps running |
| `restart` | `never` \| `on-failure` \| `always` | `never` | when to bring it back |
| `restart_delay` | seconds | `1` | base delay, grows with capped exponential backoff |
| `restart_limit` | count | `5` | give up after this many tries in a row |
| `start_timeout` | seconds | `30` | how long to wait for it to come up |
| `stop_timeout` | seconds | `10` | how long before it gets killed |
| `directory` | path | none | working directory |
| `environment` | list of `NAME=value` | empty | environment for the process |

## Refused on purpose

A unit that lists itself in any relation. The same name twice in one list. A name in both
`conflicts` and `requires`. `restart_limit = 0` together with a restart policy that asks for
restarts. A timeout of zero. An `environment` entry with no `=`. A `oneshot` service with
`restart = "always"`, which would ask a program that exits to run forever.

Every refusal points at the file, the line and the column, and prints the offending line.

## Not here yet

Timers, socket and device activation, cgroup limits, user services, and targets. They belong to
later stages, and the schema will grow to hold them; the keys above are meant to keep meaning
what they mean today when that happens.
