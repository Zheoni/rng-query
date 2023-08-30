# RNG Query
```
"a, b, c"                 => choose a, b or c at random
"a, b, c / 2"             => chose 2 values
"a, b, c / shuffle"       => shuffle the list
"2d20"                    => roll 2 20 sided dice and sum the result
"[1..10]" or "1..=10"     => random number between 1 and 10, including both
"[1..10)" or "1..10"      => number between 1 and 10 NOT including 1
"(1..5.5]"                => decimal number between 1 and 5.5 not including 5.5
"coin"                    => toss a coin
"a, coin, d6, -5..5 / 2e" => choose 2 values from the list and evaluate the result
```

## CLI
```
# Install it
cargo install rng-query

# Basic usage
rq "your query"   # run a quick query
rq                # will get you into a REPL
rq --help         # see all options
```

With the CLI you can read from `stdin`, files, treat files as only
data and combine it with a command.

For example, choose 2 lines from a file keeping the input order:
```
rq -F input.txt "/ 2o"
```

## Syntax
Each query works like a stack where you push entries separated by `\n` or `,`.
Then, everything AFTER `/`, UNTIL `\n` or `;` are options. If no `/` is given,
default options are used, which will just select one random entry.

Entries can be expressions, however, by default, if there are more than 1 entry,
then entries are treated as text, not expressions.

When the query get's executed, it consumes the stack and leaves it empty for
future queries.

### Options
If the input ends without options, `/ 1` is the default.

The options have the format `/ <n> [flags]`, where flags are just chars. Spaces
are ignored. `<n>` is a non negative integer or `all`, so you don't have to
count.
- `r`: repeating. Allow options to repeat.
- `e`: **(default if only 1 entry)** each entry is a expression like an
  interval, coin or a dice roll.
- `E`: **(default if more than 1 entry)** each entry is just text.
- `o`: keep the order when choosing multip.le

There are some shorthands:
- `/ shuffle` same as `/ all`[^1]
- `/ list` same as `/ all Eo`
- `/ eval` same as `/ all eo`

[^1]: Ok, this is actually longer, but it expresses the intention better.

### Choose number from an interval
```
[1..4]   # 1 to 4 including both 1 and 4
[1..4)   # 1 to 4 NOT including 4
(1..4]   # 1 to 4 NOT including 1
(1..4)   # 1 to 4 NOT including 1 nor 4
(0..1)f  # 0 to 1 float
(0..1.)  # 0 to 1 float
(0.5..1) # 0.5 to 1 float
```
`[` and `]` to include endpoint, `(` and `)` to exclude endpoint. If any of the
numbers is a float or there's an `f` or `F` after the closing char, the interval
will produce floating point values. A float can be `1.5`, `1.` or `.5`, but no 3
dots can be together in the interval, so `1.` can't be the start and `.5` can't
be the end.

Also `[start]..[end]` or `[start]..=[end]` to include end number is an
alternative notation only for intergers.
```
1..=4  # 1 to 4 including both
1..4   # 1 to 4 NOT including 4
```

Negatives number are supported both in integers and floats.

Open/half-open intervals are not supported because I don't know a good way to
handle max/min values.

### Toss coin
```
coin
```
When evaluated you will get `heads` or `tails`

### Roll dice
```
<amount>d[sides]
```

Sides can also be `%` which equals to `100`.

When evaluated you will get the sum of all the dice rolls.

## Notes on pseudorandomness

Currently, randomness should be statistically valid, but NOT cryptographically
securesecure. If you need this to change, submit an issue.