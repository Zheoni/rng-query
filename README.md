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

In a single line, inside balanced `[]`, `{}` or `()`, the `,` separating entries
is ignored. Also, `[]` and `()` balance together, so `[)` and `(]` are
balanced.[^1] If it's not balanced, it's an error.[^2] If you want to use some
of those chars as text, put them between `"` and it will be a string.

[^1]: This may be triggering to some people, I'm sorry, but I like the interval
    syntax.
[^2]: For input from a file, this can be bypassed.

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
- `/ shuffle` same as `/ all`[^3]
- `/ list` same as `/ all Eo`
- `/ eval` same as `/ all eo`

[^3]: Ok, this is actually longer, but it expresses the intention better.

### Choose number from an interval
```
[1..4]   # 1 to 4 including both 1 and 4
[1..4)   # 1 to 4 NOT including 4
(1..4]   # 1 to 4 NOT including 1
(1..4)   # 1 to 4 NOT including 1 nor 4
(0, 1)   # 0 to 1 float
(0, 1)   # 0 to 1 float
(0..1.0) # 0 to 1 float
(.5, 1)  # 0.5 to 1 float
```
`[` and `]` to include endpoint, `(` and `)` to exclude endpoint. If the middle
is `,`, values are floats. If the middle is `..` the values are int by default
except if any of them is a float. A float can be of the form `1.5`, `1.` or
`.5`.

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